//! # Main Optimizer Orchestrator Module
//!
//! Questo è il modulo principale che orchestra tutto il processo di ottimizzazione.
//! 
//! ## Responsabilità:
//! - Coordinamento di tutti gli altri moduli
//! - Gestione concorrenza e parallelizzazione con worker pools
//! - Orchestrazione del flusso: discovery → processing → tracking
//! - Verifica dipendenze esterne prima dell'avvio
//! - Gestione state globale e statistics
//! - Report finali con statistiche complete
//! 
//! ## Architettura:
//! - `MediaOptimizer`: Orchestratore principale (single instance)
//! - `TaskOptimizer`: Worker per processing parallelo (multiple instances)
//! 
//! ## Flusso di esecuzione:
//! 1. **Inizializzazione**: Verifica config, crea state manager
//! 2. **Dependency check**: Verifica ffmpeg, exiftool disponibili
//! 3. **File discovery**: Trova tutti i file media nella directory
//! 4. **Parallel processing**: Distribuisce lavoro su worker pool
//! 5. **Progress tracking**: Aggiorna progress bar per ogni file
//! 6. **Statistics**: Raccoglie risultati e calcola statistiche
//! 7. **Reporting**: Mostra report finale con byte saved e percentuali
//! 
//! ## Gestione concorrenza:
//! - Semafori per limitare worker concorrenti (default: 4)
//! - TaskOptimizer indipendenti per avoid shared state conflicts
//! - Progress tracking thread-safe con Arc<ProgressBar>
//! 
//! ## Processing pipeline per file:
//! 1. Check se già processato (via StateManager)
//! 2. Determine tipo (image vs video)
//! 3. Optimize con processor appropriato
//! 4. Check se riduzione sufficiente (threshold)
//! 5. Replace file originale (se non dry-run)
//! 6. Update state per evitare reprocessing
//! 
//! ## Threshold logic:
//! - Solo sostituisce se: `new_size < original_size * threshold`
//! - Default threshold: 0.9 (sostituisce se almeno 10% riduzione)
//! - Previene sostituzioni con riduzione minima
//! 
//! ## Error handling:
//! - Errors per singoli file non bloccano l'operazione
//! - Statistics tracciano numero di errori
//! - Logging dettagliato per debugging
//! 
//! ## Dry run mode:
//! - Simula tutte le operazioni senza modificare file
//! - Mostra cosa farebbe senza risk
//! - Utile per testing e preview
//! 
//! ## Esempio:
//! ```rust
//! let mut optimizer = MediaOptimizer::new(&path, config).await?;
//! optimizer.run(&path).await?; // Processa tutta la directory
//! ```

use crate::{
    config::Config,
    error::OptimizeError,
    file_manager::FileManager,
    image_processor::ImageProcessor,
    progress::{OptimizationStats, ProgressManager},
    state::{ProcessedFile, StateManager},
    video_processor::VideoProcessor,
};
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::Semaphore;
use tracing::{debug, error, info};

/// Main media optimizer orchestrator
pub struct MediaOptimizer {
    config: Config,
    state_manager: StateManager,
    input_base_dir: PathBuf,
}

impl MediaOptimizer {
    /// Create a new media optimizer instance
    pub async fn new(media_dir: &Path, config: Config) -> Result<Self> {
        config.validate()?;
        
        let state_manager = StateManager::new(media_dir).await?;
        
        Ok(Self {
            config,
            state_manager,
            input_base_dir: media_dir.to_path_buf(),
        })
    }
    
    /// Run the optimization process
    pub async fn run(&mut self, media_dir: &Path) -> Result<()> {
        info!("Starting media optimization in: {}", media_dir.display());
        
        // Log configuration details
        if self.config.convert_to_webp {
            info!("🎯 Mode: Convert all media to WebP (quality: {})", self.config.webp_quality);
        } else {
            info!("🎯 Mode: Optimize in original formats (JPEG quality: {})", self.config.jpeg_quality);
        }
        
        // Log the keep_processed flag status if output directory is used
        if let Some(ref output_path) = self.config.output_path {
            info!("📁 Output directory: {}", output_path.display());
            if self.config.keep_processed {
                info!("⏩ Skip mode: Will skip files where output already exists");
            } else {
                info!("🔄 Overwrite mode: Will overwrite existing output files");
            }
        } else {
            info!("📁 Mode: Replace files in place");
        }
        
        if self.config.dry_run {
            info!("🧪 Dry run mode: No files will be modified");
        }
        
        if self.config.skip_video_compression {
            info!("🎬 Video mode: Skip compression (copy only)");
        } else {
            info!("🎬 Video mode: Compress videos (CRF: {})", self.config.video_crf);
        }
        
        // Check dependencies
        self.check_dependencies().await?;
        
        // Clean up old state entries
        self.state_manager.cleanup().await?;
        
        // Find all media files
        let files = FileManager::find_media_files(media_dir)?;
        info!("Found {} media files to process", files.len());
        
        if files.is_empty() {
            info!("No media files found to process");
            return Ok(());
        }
        
        // Initialize progress tracking
        let progress = ProgressManager::new(files.len() as u64);
        let mut stats = OptimizationStats::new();
        
        // Process files with controlled concurrency
        let semaphore = Arc::new(Semaphore::new(self.config.workers));
        let mut tasks = Vec::new();
        
        for file_path in files {
            let permit = semaphore.clone().acquire_owned().await?;
            let task_optimizer = TaskOptimizer {
                config: self.config.clone(),
                image_processor: ImageProcessor::new(self.config.clone()),
                video_processor: VideoProcessor::new(self.config.clone()),
                input_base_dir: self.input_base_dir.clone(),
            };
            let progress_clone = progress.clone();
            
            let task = tokio::spawn(async move {
                let _permit = permit; // Keep permit alive
                
                // Add timeout to prevent individual files from hanging
                // Use different timeouts based on file type
                let timeout_duration = if FileManager::is_video(&file_path) {
                    std::time::Duration::from_secs(900) // 15 minutes for videos
                } else {
                    std::time::Duration::from_secs(180) // 3 minutes for images
                };
                
                let result = tokio::time::timeout(
                    timeout_duration,
                    task_optimizer.process_single_file(file_path.clone())
                ).await;
                
                let result = match result {
                    Ok(r) => r,
                    Err(_) => {
                        error!("File processing timed out: {}", file_path.display());
                        
                        // If we have an output directory, copy the original file
                        if let Some(ref _output_dir) = task_optimizer.config.output_path {
                            if let Ok(expected_output) = task_optimizer.get_expected_output_path(&file_path) {
                                if let Some(parent) = expected_output.parent() {
                                    let _ = tokio::fs::create_dir_all(parent).await;
                                }
                                if let Err(e) = std::fs::copy(&file_path, &expected_output) {
                                    error!("Failed to copy original file after timeout: {}", e);
                                } else {
                                    debug!("Copied original file to output after timeout: {}", expected_output.display());
                                }
                            }
                        }
                        
                        Err(anyhow::anyhow!("Processing timeout"))
                    }
                };
                
                let message = match &result {
                    Ok(Some(processed)) => {
                        format!("✅ {}: {:.1}% saved", 
                               file_path.file_name().unwrap_or_default().to_string_lossy(),
                               processed.reduction_percent)
                    }
                    Ok(None) => {
                        format!("⏩ {}: skipped", 
                               file_path.file_name().unwrap_or_default().to_string_lossy())
                    }
                    Err(_) => {
                        format!("❌ {}: error", 
                               file_path.file_name().unwrap_or_default().to_string_lossy())
                    }
                };
                
                progress_clone.update(&message);
                result
            });
            
            tasks.push(task);
        }
        
        // Wait for all tasks and collect results
        for task in tasks {
            match task.await? {
                Ok(Some(processed)) => {
                    stats.add_optimized(processed.original_size, processed.optimized_size);
                }
                Ok(None) => {
                    // File was skipped or already processed - we don't have size info here
                    stats.add_skipped(0);
                }
                Err(e) => {
                    stats.add_error();
                    error!("Failed to process file: {}", e);
                }
            }
        }
        
        progress.finish(&stats.format_summary());
        
        // Print final statistics
        self.print_final_stats(&stats).await?;
        
        Ok(())
    }
    
    async fn check_dependencies(&self) -> Result<()> {
        ImageProcessor::check_dependencies().await?;
        VideoProcessor::check_dependencies().await?;
        
        // Check for WebP support if needed
        if self.config.convert_to_webp {
            if !ImageProcessor::check_webp_support().await {
                return Err(anyhow::anyhow!(
                    "cwebp is required for WebP conversion. Please install webp tools."
                ));
            }
        }
        
        Ok(())
    }
    
    async fn print_final_stats(&self, stats: &OptimizationStats) -> Result<()> {
        let (total_files, total_saved, avg_reduction) = self.state_manager.get_stats();
        
        info!("=== Optimization Complete ===");
        info!("Files processed this run: {}", stats.files_processed);
        info!("Files optimized this run: {}", stats.files_optimized);
        info!("Files skipped this run: {}", stats.files_skipped);
        info!("Errors this run: {}", stats.errors);
        info!("Bytes saved this run: {}", FileManager::format_size(stats.total_bytes_saved));
        info!("Average reduction this run: {:.2}%", stats.overall_reduction_percent());
        info!("--- Historical Stats ---");
        info!("Total files ever processed: {}", total_files);
        info!("Total bytes saved historically: {}", FileManager::format_size(total_saved));
        info!("Average historical reduction: {:.2}%", avg_reduction);
        
        Ok(())
    }
}

/// Simplified optimizer for individual tasks
struct TaskOptimizer {
    config: Config,
    image_processor: ImageProcessor,
    video_processor: VideoProcessor,
    input_base_dir: PathBuf,
}

impl TaskOptimizer {
    /// Calculate the expected output path for a given input file
    fn get_expected_output_path(&self, input_path: &Path) -> Result<PathBuf> {
        let file_stem = input_path.file_stem()
            .ok_or_else(|| anyhow::anyhow!("Invalid file name: {}", input_path.display()))?
            .to_string_lossy();
        
        // Determine output extension based on file type
        let (filename, extension) = if FileManager::is_video(input_path) {
            (format!("{}.mp4", file_stem), "mp4")
        } else if self.config.convert_to_webp {
            (format!("{}.webp", file_stem), "webp")
        } else {
            let ext = input_path.extension().unwrap_or_default().to_str().unwrap_or("jpg");
            (format!("{}.{}", file_stem, ext), ext)
        };
        
        if let Some(ref output_dir) = self.config.output_path {
            // Canonicalize input_base_dir to match canonicalized file_path for strip_prefix
            let canonical_base = self.input_base_dir.canonicalize()
                .map_err(|e| anyhow::anyhow!("Failed to canonicalize base dir {}: {}", self.input_base_dir.display(), e))?;
            
            // Canonicalize output_dir to get the correct absolute path
            let canonical_output = output_dir.canonicalize()
                .map_err(|e| anyhow::anyhow!("Failed to canonicalize output dir {}: {}", output_dir.display(), e))?;
            
            debug!("Calculating path for: {}", input_path.display());
            debug!("Canonical base: {}", canonical_base.display());
            debug!("Canonical output: {}", canonical_output.display());
            
            // Use EXACTLY the same logic as ImageProcessor::get_output_path
            let relative_path = match input_path.strip_prefix(&canonical_base) {
                Ok(rel) => {
                    debug!("✅ Strip prefix successful: {}", rel.display());
                    rel.parent().unwrap_or(Path::new(""))
                }
                Err(e) => {
                    debug!("❌ Strip prefix failed: {} - fallback to parent", e);
                    input_path.parent().unwrap_or(Path::new(""))
                }
            };
            
            let result = canonical_output.join(relative_path).join(filename);
            debug!("Expected output path: {} -> {}", input_path.display(), result.display());
            debug!("Relative path used: {}", relative_path.display());
            
            Ok(result)
        } else {
            // This shouldn't be called for in-place mode, but handle it anyway
            Ok(input_path.with_file_name(format!("optimized.{}", extension)))
        }
    }

    async fn process_single_file(&self, file_path: PathBuf) -> Result<Option<ProcessedFile>> {
        debug!("Starting process_single_file for: {}", file_path.display());
        
        // Convert to absolute path to avoid working directory issues
        let file_path = file_path.canonicalize()
            .map_err(|e| anyhow::anyhow!("Failed to canonicalize path {}: {}", file_path.display(), e))?;
        debug!("Canonicalized path: {}", file_path.display());
            
        let (original_size, modified_time) = FileManager::get_file_info(&file_path).await
            .map_err(|e| anyhow::anyhow!("Failed to get file info for {}: {}", file_path.display(), e))?;
        debug!("File info - size: {}, modified: {}", original_size, modified_time);
        
        // Check if we should skip this file
        if self.config.output_path.is_none() {
            // For in-place optimization, check state manager
            let state_manager = StateManager::new(&self.input_base_dir).await?;
            if state_manager.is_processed(&file_path, modified_time) {
                debug!("Skipping already processed file (in-place): {}", file_path.display());
                return Ok(None);
            }
        } else if self.config.keep_processed {
            // For output directory mode with --keep-processed, check if output file already exists
            let expected_output_path = self.get_expected_output_path(&file_path)?;
            debug!("Checking if output exists: {} -> {}", file_path.display(), expected_output_path.display());
            if expected_output_path.exists() {
                debug!("✅ Skipping file, output already exists: {} -> {}", 
                       file_path.display(), expected_output_path.display());
                return Ok(None);
            } else {
                debug!("❌ Output does not exist, will process: {}", expected_output_path.display());
            }
        }
        
        debug!("Processing: {}", file_path.display());
        
        // Optimize based on file type
        let optimized_path = if FileManager::is_image(&file_path) {
            debug!("Processing as image: {}", file_path.display());
            self.image_processor.optimize(&file_path, &self.input_base_dir).await
                .map_err(|e| anyhow::anyhow!("Image optimization failed for {}: {}", file_path.display(), e))?
        } else if FileManager::is_video(&file_path) {
            debug!("Processing as video: {}", file_path.display());
            self.video_processor.optimize(&file_path, &self.input_base_dir).await
                .map_err(|e| anyhow::anyhow!("Video optimization failed for {}: {}", file_path.display(), e))?
        } else {
            return Err(OptimizeError::UnsupportedFormat(
                format!("Unsupported file type: {}", file_path.display())
            ).into());
        };
        debug!("Optimized file created at: {}", optimized_path.display());
        
        let optimized_size = FileManager::get_file_info(&optimized_path).await
            .map_err(|e| anyhow::anyhow!("Failed to get optimized file info for {}: {}", optimized_path.display(), e))?.0;
        debug!("Optimized file size: {}", optimized_size);
        
        let processed_file = ProcessedFile::new(
            file_path.clone(),
            modified_time,
            original_size,
            optimized_size,
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)?
                .as_secs(),
        );
        debug!("Created ProcessedFile: {:?}", processed_file);
        
        // Check if optimization was worthwhile
        let should_replace = if self.config.output_path.is_some() {
            // For output directory mode, always keep the optimized file
            true
        } else {
            // For in-place replacement, check size threshold
            (optimized_size as f64) < (original_size as f64 * self.config.size_threshold)
        };
        debug!("Should replace? {} (optimized: {}, original: {}, threshold: {})", 
               should_replace, optimized_size, original_size, self.config.size_threshold);
        
        if should_replace {
            if !self.config.dry_run {
                if self.config.output_path.is_some() {
                    // Output to different directory - file is already in place
                    debug!("File saved to output directory: {}", optimized_path.display());
                } else {
                    // Replace original file
                    debug!("Replacing file: {} with {}", file_path.display(), optimized_path.display());
                    FileManager::replace_file(&file_path, &optimized_path).await
                        .map_err(|e| anyhow::anyhow!("Failed to replace file {}: {}", file_path.display(), e))?;
                    
                    // Clean up the temporary optimized file only if we copied it over
                    debug!("Cleaning up temporary file: {}", optimized_path.display());
                    let _ = std::fs::remove_file(&optimized_path);
                }
            } else {
                if self.config.output_path.is_some() {
                    debug!("Dry run: would save {} to {}", file_path.display(), optimized_path.display());
                } else {
                    debug!("Dry run: would replace {} with {}", file_path.display(), optimized_path.display());
                }
                
                // Clean up temp file in dry run mode
                debug!("Cleaning up temporary file: {}", optimized_path.display());
                let _ = std::fs::remove_file(&optimized_path);
            }
            
            // Mark as processed only for in-place optimization
            if self.config.output_path.is_none() {
                let mut state_manager = StateManager::new(&self.input_base_dir).await?;
                state_manager.mark_processed(processed_file.clone()).await?;
            }
            
            Ok(Some(processed_file))
        } else {
            // For output directory mode, copy original file even if optimization wasn't worthwhile
            if self.config.output_path.is_some() && !self.config.dry_run {
                // Copy the original file to output directory
                let original_output_path = self.get_expected_output_path(&file_path)?;
                if let Some(parent) = original_output_path.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }
                std::fs::copy(&file_path, &original_output_path)?;
                debug!("Copied original file to output directory (insufficient reduction): {}", original_output_path.display());
            }
            
            // Clean up the temporary optimized file
            debug!("Cleaning up temporary file: {}", optimized_path.display());
            let _ = std::fs::remove_file(&optimized_path);
            
            // Mark as processed to avoid reprocessing (only for in-place optimization)
            if self.config.output_path.is_none() {
                let mut state_manager = StateManager::new(&self.input_base_dir).await?;
                state_manager.mark_processed(processed_file.clone()).await?;
                debug!("Marked file as processed (skipped)");
            }
            
            Ok(None)
        }
    }
}
