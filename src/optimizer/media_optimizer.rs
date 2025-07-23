//! # Media Optimizer Main Orchestrator
//!
//! Orchestratore principale semplificato che delega responsabilità
//! ai moduli specializzati.

use crate::{
    config::Config,
    file_manager::FileManager,
    image_processor::ImageProcessor,
    json_output::{JsonConfig, JsonMessage, HistoricalStats},
    optimizer::{progress_tracker::ProgressTracker, task_optimizer::TaskOptimizer},
    progress::OptimizationStats,
    resize::{ImageResizer, ResizeAlgorithm, ResizeMode},
    state::{StateManager, ProcessedFile},
    video_processor::VideoProcessor,
};
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing::{debug, error, info, warn};

/// Orchestratore principale ottimizzato
pub struct MediaOptimizer {
    config: Config,
    state_manager: StateManager,
    input_base_dir: PathBuf,
}

impl MediaOptimizer {
    /// Crea nuova istanza dell'ottimizzatore
    pub async fn new(media_dir: &Path, config: Config) -> Result<Self> {
        config.validate()?;
        let state_manager = StateManager::new(media_dir).await?;
        
        Ok(Self {
            config,
            state_manager,
            input_base_dir: media_dir.to_path_buf(),
        })
    }
    
    /// Esegue il processo di ottimizzazione
    pub async fn run(&mut self, media_dir: &Path) -> Result<()> {
        let start_time = std::time::Instant::now();
        
        // Trova tutti i file media
        let files = FileManager::find_media_files(media_dir)?;
        
        self.emit_start_message(media_dir, &files).await;
        self.log_configuration(&files);
        
        // Controlla dipendenze
        self.check_dependencies().await?;
        self.state_manager.cleanup().await?;
        
        if files.is_empty() {
            self.handle_empty_directory(start_time).await;
            return Ok(());
        }
        
        // Processa file con concorrenza controllata
        let progress_tracker = ProgressTracker::new(files.len());
        
        // STRATEGIA MIGLIORATA: Crea thumbnails dalle originali PRIMA dell'ottimizzazione
        if !self.config.thumbnails.is_empty() && self.config.output_path.is_some() {
            info!("🖼️ Creating thumbnails from original images (before optimization)...");
            self.create_thumbnails_from_originals(&files).await?;
        }
        
        let stats = self.process_files_concurrently(files, progress_tracker.clone()).await?;
        
        // Finalizza e stampa statistiche
        progress_tracker.finish(&stats.format_summary());
        self.print_final_stats(&stats, start_time.elapsed().as_secs_f64()).await?;
        
        Ok(())
    }
    
    /// Invia messaggio di inizio
    async fn emit_start_message(&self, media_dir: &Path, files: &[PathBuf]) {
        if self.config.json_output {
            JsonMessage::start(
                media_dir.to_path_buf(),
                self.config.output_path.clone(),
                files.len(),
                JsonConfig::from(&self.config),
            ).emit();
        } else {
            info!("Starting media optimization in: {}", media_dir.display());
        }
    }
    
    /// Logga configurazione (solo se non JSON mode)
    fn log_configuration(&self, files: &[PathBuf]) {
        if self.config.json_output {
            return;
        }
        
        if self.config.convert_to_webp {
            info!("Mode: Convert all media to WebP (quality: {})", self.config.webp_quality);
        } else {
            info!("Mode: Optimize in original formats (JPEG quality: {})", self.config.jpeg_quality);
        }
        
        if let Some(ref output_path) = self.config.output_path {
            info!("Output directory: {}", output_path.display());
            if self.config.keep_processed {
                info!("Skip mode: Will skip files where output already exists");
            } else {
                info!("Overwrite mode: Will overwrite existing output files");
            }
        } else {
            info!("Mode: Replace files in place");
        }
        
        if self.config.dry_run {
            info!("Dry run mode: No files will be modified");
        }
        
        if self.config.skip_video_compression {
            info!("Video mode: Skip compression (copy only)");
        } else {
            info!("Video mode: Compress videos (CRF: {})", self.config.video_crf);
        }
        
        // Log thumbnail configuration if enabled
        if !self.config.thumbnails.is_empty() {
            info!("Thumbnails: {} sizes configured", self.config.thumbnails.len());
            for (name, size) in &self.config.thumbnails {
                info!("  • {} - {}x{}", name, size.width, size.height);
            }
        }
        
        info!("Found {} media files to process", files.len());
    }
    
    /// Gestisce directory vuota
    async fn handle_empty_directory(&self, start_time: std::time::Instant) {
        if self.config.json_output {
            JsonMessage::complete(
                0, 0, 0, 0, 0, 0.0, start_time.elapsed().as_secs_f64(),
                HistoricalStats {
                    total_files_ever_processed: 0,
                    total_bytes_saved_historically: 0,
                    average_historical_reduction: 0.0,
                }
            ).emit();
        } else {
            info!("No media files found to process");
        }
    }
    
    /// Processa file con concorrenza controllata
    async fn process_files_concurrently(
        &self,
        files: Vec<PathBuf>,
        progress_tracker: ProgressTracker
    ) -> Result<OptimizationStats> {
        let semaphore = Arc::new(Semaphore::new(self.config.workers));
        let video_semaphore = Arc::new(Semaphore::new(1)); // Un video alla volta
        let mut tasks: Vec<tokio::task::JoinHandle<Result<Option<ProcessedFile>, anyhow::Error>>> = Vec::new();
        let mut stats = OptimizationStats::new();

        for (index, file_path) in files.iter().enumerate() {
            let permit = semaphore.clone().acquire_owned().await?;
            let is_video = FileManager::is_video(file_path);
            let video_permit = if is_video {
                Some(video_semaphore.clone().acquire_owned().await?)
            } else {
                None
            };

            let mut task_optimizer = TaskOptimizer::new(self.config.clone(), self.input_base_dir.clone()).await?;
            let progress_clone = progress_tracker.clone();
            let file_path_clone = file_path.clone();

            let task = tokio::spawn(async move {
                let _permit = permit;
                let _video_permit = video_permit;

                // Emetti evento inizio file
                if task_optimizer.config.json_output {
                    if let Ok(metadata) = tokio::fs::metadata(&file_path_clone).await {
                        JsonMessage::file_start(
                            file_path_clone.clone(),
                            metadata.len(),
                            index,
                            progress_clone.total_files,
                        ).emit();
                    }
                }

                // Timeout basato sul tipo di file
                let timeout_duration = if is_video {
                    std::time::Duration::from_secs(900) // 15 minuti per video
                } else {
                    std::time::Duration::from_secs(180) // 3 minuti per immagini
                };
                
                let result = tokio::time::timeout(
                    timeout_duration,
                    task_optimizer.process_single_file(file_path_clone.clone())
                ).await;
                
                let result = match result {
                    Ok(r) => r,
                    Err(_) => {
                        error!("File processing timed out: {}", file_path_clone.display());
                        Self::handle_timeout(&task_optimizer, &file_path_clone).await;
                        Err(anyhow::anyhow!("Processing timeout"))
                    }
                };

                // Gestisci risultati e eventi JSON
                progress_clone.handle_file_completion(&task_optimizer.config, &file_path_clone, &result).await;
                result
            });

            tasks.push(task);
        }
        
        // Aspetta tutti i task e raccoglie risultati
        for task in tasks {
            match task.await? {
                Ok(Some(processed)) => {
                    stats.add_optimized(processed.original_size, processed.optimized_size);
                }
                Ok(None) => {
                    stats.add_skipped(0);
                }
                Err(e) => {
                    stats.add_error();
                    error!("Failed to process file: {}", e);
                }
            }
        }
        
        Ok(stats)
    }
    
    /// Gestisce timeout di processing
    async fn handle_timeout(task_optimizer: &TaskOptimizer, file_path: &Path) {
        // Se abbiamo output directory, copia file originale
        if let Some(ref _output_dir) = task_optimizer.config.output_path {
            if let Ok(expected_output) = task_optimizer.get_expected_output_path(file_path) {
                if let Some(parent) = expected_output.parent() {
                    let _ = tokio::fs::create_dir_all(parent).await;
                }
                if let Err(e) = std::fs::copy(file_path, &expected_output) {
                    error!("Failed to copy original file after timeout: {}", e);
                } else {
                    debug!("Copied original file to output after timeout: {}", expected_output.display());
                }
            }
        }
    }
    
    /// Controlla dipendenze
    async fn check_dependencies(&self) -> Result<()> {
        ImageProcessor::check_dependencies().await?;
        VideoProcessor::check_dependencies().await?;
        
        if self.config.convert_to_webp {
            if !ImageProcessor::check_webp_support().await {
                return Err(anyhow::anyhow!(
                    "cwebp is required for WebP conversion. Please install webp tools."
                ));
            }
        }
        
        Ok(())
    }
    
    /// Stampa statistiche finali
    async fn print_final_stats(&self, stats: &OptimizationStats, duration: f64) -> Result<()> {
        let (total_files, total_saved, avg_reduction) = self.state_manager.get_stats();
        
        if self.config.json_output {
            JsonMessage::complete(
                stats.files_processed,
                stats.files_optimized,
                stats.files_skipped,
                stats.errors,
                stats.total_bytes_saved,
                stats.overall_reduction_percent(),
                duration,
                HistoricalStats {
                    total_files_ever_processed: total_files,
                    total_bytes_saved_historically: total_saved,
                    average_historical_reduction: avg_reduction,
                }
            ).emit();
        } else {
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
        }
        
        Ok(())
    }
    
    /// Crea thumbnails per tutte le immagini elaborate
    async fn create_thumbnails_for_all_images(&self) -> Result<()> {
        if self.config.thumbnails.is_empty() || self.config.output_path.is_none() {
            return Ok(());
        }

        info!("🖼️ Creating thumbnails for processed images...");
        
        let output_dir = self.config.output_path.as_ref().unwrap();
        
        // Trova solo le immagini nella directory principale di output (non nelle sottocartelle thumbnail)
        let output_images = FileManager::find_media_files(output_dir)?
            .into_iter()
            .filter(|path| {
                // Verifica che sia supportato per il resize
                if !ImageResizer::is_supported_for_resize(path) {
                    return false;
                }
                
                // Verifica che NON sia in una cartella di thumbnails
                // Controlla se il path contiene "thumbnails" in qualsiasi parte del percorso relativo
                if let Ok(rel_path) = path.strip_prefix(output_dir) {
                    let path_str = rel_path.to_string_lossy();
                    // Esclude qualsiasi file che abbia "thumbnails" nel path
                    if path_str.contains("thumbnails") {
                        debug!("Skipping thumbnail file for input: {}", path.display());
                        return false;
                    }
                }
                
                true
            })
            .collect::<Vec<_>>();

        if output_images.is_empty() {
            info!("No images found for thumbnail creation");
            return Ok(());
        }

        info!("Found {} images for thumbnail creation", output_images.len());

        // Crea il resizer con qualità molto alta per minimizzare degrado da doppia compressione
        let resizer = ImageResizer::new(
            self.config.clone(),
            ResizeAlgorithm::Lanczos,
            ResizeMode::Fit,
            Some(95), // Qualità molto alta per minimizzare degrado da doppia compressione JPEG
            true, // Strip metadata for smaller thumbnails
        )?;

        let estimated_thumbnails = resizer.estimate_thumbnail_count(&output_images);
        info!("Will create {} thumbnails total", estimated_thumbnails);

        // Processo i thumbnails con concorrenza controllata
        let semaphore = Arc::new(Semaphore::new(self.config.workers.min(4))); // Limite per i thumbnails
        let mut tasks: Vec<tokio::task::JoinHandle<Result<usize, anyhow::Error>>> = Vec::new();

        for image_path in output_images {
            let permit = semaphore.clone().acquire_owned().await?;
            let mut resizer_clone = ImageResizer::new(
                self.config.clone(),
                ResizeAlgorithm::Lanczos,
                ResizeMode::Fit,
                Some(95), // Qualità molto alta per minimizzare degrado da doppia compressione JPEG
                true, // Strip metadata for smaller thumbnails
            )?;
            let output_base = output_dir.clone();

            let task = tokio::spawn(async move {
                let _permit = permit;
                
                match resizer_clone.create_thumbnails(&image_path, &output_base).await {
                    Ok(thumbnail_paths) => {
                        if !thumbnail_paths.is_empty() {
                            info!("Created {} thumbnails for {}", 
                                  thumbnail_paths.len(),
                                  image_path.file_name().unwrap_or_default().to_string_lossy());
                        }
                        Ok(thumbnail_paths.len())
                    }
                    Err(e) => {
                        error!("Failed to create thumbnails for {}: {}", image_path.display(), e);
                        Ok(0)
                    }
                }
            });

            tasks.push(task);
        }

        // Aspetta tutti i task
        let mut total_created = 0;
        for task in tasks {
            match task.await? {
                Ok(count) => total_created += count,
                Err(_) => {} // Errore già loggato
            }
        }

        info!("✅ Thumbnail creation complete: {} thumbnails created", total_created);
        Ok(())
    }
    
    /// Crea thumbnails dalle immagini originali (prima dell'ottimizzazione)
    /// Questa strategia preserva la massima qualità dei thumbnails
    async fn create_thumbnails_from_originals(&self, files: &[PathBuf]) -> Result<()> {
        if self.config.thumbnails.is_empty() || self.config.output_path.is_none() {
            return Ok(());
        }

        let output_dir = self.config.output_path.as_ref().unwrap();
        
        // Calcola la directory base comune dai file originali
        let base_dir = if let Some(first_file) = files.first() {
            // Usa la directory del primo file come base, oppure trova il prefisso comune
            self.find_common_base_dir(files).unwrap_or_else(|| {
                first_file.parent().unwrap_or(Path::new(".")).to_path_buf()
            })
        } else {
            return Ok(());
        };
        
        // Filtra solo le immagini supportate per il resize
        let image_files: Vec<_> = files
            .iter()
            .filter(|path| ImageResizer::is_supported_for_resize(path))
            .collect();

        if image_files.is_empty() {
            info!("No images found for thumbnail creation from originals");
            return Ok(());
        }

        info!("Found {} original images for thumbnail creation", image_files.len());

        // Crea il resizer con qualità massima (dalle originali)
        let resizer = ImageResizer::new(
            self.config.clone(),
            ResizeAlgorithm::Lanczos, // Migliore qualità per le originali
            ResizeMode::Fit,
            Some(95), // Qualità alta per preservare dettagli dalle originali
            true, // Strip metadata for smaller thumbnails
        )?;

        let estimated_thumbnails = resizer.estimate_thumbnail_count(&image_files.iter().map(|p| (*p).clone()).collect::<Vec<_>>());
        info!("Will create {} thumbnails total from originals", estimated_thumbnails);

        // Processo i thumbnails con concorrenza controllata
        let semaphore = Arc::new(Semaphore::new(self.config.workers.min(4))); // Limite per i thumbnails
        let mut tasks: Vec<tokio::task::JoinHandle<Result<usize, anyhow::Error>>> = Vec::new();

        for image_path in image_files {
            let permit = semaphore.clone().acquire_owned().await?;
            let mut resizer_clone = ImageResizer::new(
                self.config.clone(),
                ResizeAlgorithm::Lanczos, // Migliore qualità per le originali
                ResizeMode::Fit,
                Some(95), // Qualità alta per preservare dettagli dalle originali
                true, // Strip metadata for smaller thumbnails
            )?;
            let output_base = output_dir.clone();
            let media_base = base_dir.clone();
            let image_path = image_path.clone();

            let task = tokio::spawn(async move {
                let _permit = permit;
                
                match resizer_clone.create_thumbnails(&image_path, &media_base).await {
                    Ok(thumbnail_paths) => {
                        if !thumbnail_paths.is_empty() {
                            info!("Created {} thumbnails from original {}", 
                                  thumbnail_paths.len(),
                                  image_path.file_name().unwrap_or_default().to_string_lossy());
                        }
                        Ok(thumbnail_paths.len())
                    }
                    Err(e) => {
                        error!("Failed to create thumbnails from original {}: {}", image_path.display(), e);
                        Ok(0)
                    }
                }
            });

            tasks.push(task);
        }

        // Aspetta tutti i task
        let mut total_created = 0;
        for task in tasks {
            match task.await {
                Ok(Ok(count)) => total_created += count,
                Ok(Err(e)) => error!("Thumbnail creation task failed: {}", e),
                Err(e) => error!("Thumbnail creation task panicked: {}", e),
            }
        }

        if total_created > 0 {
            info!("✅ Created {} thumbnails from original images", total_created);
        } else {
            warn!("No thumbnails were created from original images");
        }
        
        Ok(())
    }

    /// Trova la directory base comune per un set di file
    fn find_common_base_dir(&self, files: &[PathBuf]) -> Option<PathBuf> {
        if files.is_empty() {
            return None;
        }

        // Inizia con la directory del primo file
        let mut common_path = files[0].parent()?.to_path_buf();

        // Per ogni file successivo, trova il prefisso comune
        for file in files.iter().skip(1) {
            if let Some(file_parent) = file.parent() {
                // Trova il prefisso comune tra common_path e file_parent
                let mut new_common = PathBuf::new();
                let common_components: Vec<_> = common_path.components().collect();
                let file_components: Vec<_> = file_parent.components().collect();

                for (c1, c2) in common_components.iter().zip(file_components.iter()) {
                    if c1 == c2 {
                        new_common.push(c1);
                    } else {
                        break;
                    }
                }

                common_path = new_common;
                
                // Se non c'è più un prefisso comune, usa la root
                if common_path.as_os_str().is_empty() {
                    return Some(PathBuf::from("."));
                }
            }
        }

        Some(common_path)
    }
}
