//! # Task Optimizer Module
//!
//! Worker per l'ottimizzazione di singoli file.
//! Separato dal orchestratore principale per maggiore modularità.

use crate::{
    config::Config,
    error::OptimizeError,
    file_manager::FileManager,
    image_processor::ImageProcessor,
    optimizer::path_resolver::PathResolver,
    state::{ProcessedFile, StateManager},
    video_processor::VideoProcessor,
};
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tracing::debug;

/// Worker ottimizzato per elaborazione singoli file
pub struct TaskOptimizer {
    pub config: Config,
    pub image_processor: ImageProcessor,
    pub video_processor: VideoProcessor,
    pub input_base_dir: PathBuf,
}

impl TaskOptimizer {
    /// Crea nuovo task optimizer
    pub async fn new(config: Config, input_base_dir: PathBuf) -> Result<Self> {
        let image_processor = ImageProcessor::new(config.clone()).await?;
        let video_processor = VideoProcessor::new(config.clone());
        
        Ok(Self {
            config,
            image_processor,
            video_processor,
            input_base_dir,
        })
    }
    
    /// Calcola path di output atteso (delegato a PathResolver)
    pub fn get_expected_output_path(&self, input_path: &Path) -> Result<PathBuf> {
        PathResolver::get_output_path(input_path, &self.input_base_dir, &self.config)
    }

    /// Processa un singolo file
    pub async fn process_single_file(&mut self, file_path: PathBuf) -> Result<Option<ProcessedFile>> {
        // debug!("Starting process_single_file for: {}", file_path.display());
        
        // Canonicalizza path
        let file_path = file_path.canonicalize()
            .map_err(|e| anyhow::anyhow!("Failed to canonicalize path {}: {}", file_path.display(), e))?;
        // debug!("Canonicalized path: {}", file_path.display());
            
        let (original_size, modified_time) = FileManager::get_file_info(&file_path).await
            .map_err(|e| anyhow::anyhow!("Failed to get file info for {}: {}", file_path.display(), e))?;
        // debug!("File info - size: {}, modified: {}", original_size, modified_time);
        
        // Controlla se skippare il file
        if self.should_skip_file(&file_path, modified_time).await? {
            return Ok(None);
        }
        
        
        // Ottimizza basato sul tipo di file
        let optimized_path = self.optimize_file(&file_path).await?;
        // // debug!("Optimized file created at: {}", optimized_path.display());
        
        let optimized_size = FileManager::get_file_info(&optimized_path).await
            .map_err(|e| anyhow::anyhow!("Failed to get optimized file info for {}: {}", optimized_path.display(), e))?.0;
        // // debug!("Optimized file size: {}", optimized_size);
        
        let processed_file = ProcessedFile::new(
            file_path.clone(),
            modified_time,
            original_size,
            optimized_size,
            SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?.as_secs(),
        );
        // // debug!("Created ProcessedFile: {:?}", processed_file);
        
        // Controlla se l'ottimizzazione vale la pena
        self.handle_optimization_result(&file_path, &optimized_path, processed_file).await
    }
    
    /// Controlla se un file deve essere skippato
    async fn should_skip_file(&self, file_path: &Path, modified_time: u64) -> Result<bool> {
        if self.config.output_path.is_none() {
            // Per ottimizzazione in-place, controlla state manager
            let state_manager = StateManager::new(&self.input_base_dir).await?;
            if state_manager.is_processed(file_path, modified_time) {
                // debug!("Skipping already processed file (in-place): {}", file_path.display());
                return Ok(true);
            }
        } else if self.config.keep_processed {
            // Per output directory con --keep-processed, controlla se output esiste
            let expected_output_path = self.get_expected_output_path(file_path)?;
            // // debug!("Checking if output exists: {} -> {}", file_path.display(), expected_output_path.display());
            if expected_output_path.exists() {
                debug!("[OK] Skipping file, output already exists: {} -> {}", 
                       file_path.display(), expected_output_path.display());
                return Ok(true);
            } else {
                debug!("[PROCESS] Output does not exist, will process: {}", expected_output_path.display());
            }
        }
        Ok(false)
    }
    
    /// Ottimizza file basato sul tipo
    async fn optimize_file(&mut self, file_path: &Path) -> Result<PathBuf> {
        if FileManager::is_image(file_path) {
            // debug!("Processing as image: {}", file_path.display());
            self.image_processor.optimize(file_path, &self.input_base_dir).await
                .map_err(|e| anyhow::anyhow!("Image optimization failed for {}: {}", file_path.display(), e))
        } else if FileManager::is_video(file_path) {
            // debug!("Processing as video: {}", file_path.display());
            self.video_processor.optimize(file_path, &self.input_base_dir).await
                .map_err(|e| anyhow::anyhow!("Video optimization failed for {}: {}", file_path.display(), e))
        } else {
            Err(OptimizeError::UnsupportedFormat(
                format!("Unsupported file type: {}", file_path.display())
            ).into())
        }
    }
    
    /// Gestisce il risultato dell'ottimizzazione
    async fn handle_optimization_result(
        &self,
        file_path: &Path,
        optimized_path: &Path,
        processed_file: ProcessedFile
    ) -> Result<Option<ProcessedFile>> {
        let should_replace = (processed_file.optimized_size as f64) < 
                           (processed_file.original_size as f64 * self.config.size_threshold);
        
        debug!("Should replace? {} (optimized: {}, original: {}, threshold: {})", 
               should_replace, processed_file.optimized_size, processed_file.original_size, self.config.size_threshold);
        
        if should_replace {
            self.handle_successful_optimization(file_path, optimized_path, processed_file).await
        } else {
            self.handle_insufficient_optimization(file_path, optimized_path).await
        }
    }
    
    /// Gestisce ottimizzazione riuscita
    async fn handle_successful_optimization(
        &self,
        file_path: &Path,
        optimized_path: &Path,
        processed_file: ProcessedFile
    ) -> Result<Option<ProcessedFile>> {
        if !self.config.dry_run {
            if self.config.output_path.is_some() {
                // debug!("File saved to output directory: {}", optimized_path.display());
            } else {
                // debug!("Replacing file: {} with {}", file_path.display(), optimized_path.display());
                FileManager::replace_file(file_path, optimized_path).await
                    .map_err(|e| anyhow::anyhow!("Failed to replace file {}: {}", file_path.display(), e))?;
                
                // debug!("Cleaning up temporary file: {}", optimized_path.display());
                let _ = std::fs::remove_file(optimized_path);
            }
        } else {
            if self.config.output_path.is_some() {
                // debug!("Dry run: would save {} to {}", file_path.display(), optimized_path.display());
            } else {
                // debug!("Dry run: would replace {} with {}", file_path.display(), optimized_path.display());
            }
            // debug!("Cleaning up temporary file: {}", optimized_path.display());
            let _ = std::fs::remove_file(optimized_path);
        }
        
        // Marca come processato solo per ottimizzazione in-place
        if self.config.output_path.is_none() {
            let mut state_manager = StateManager::new(&self.input_base_dir).await?;
            state_manager.mark_processed(processed_file.clone()).await?;
        }
        
        Ok(Some(processed_file))
    }
    
    /// Gestisce ottimizzazione insufficiente
    async fn handle_insufficient_optimization(
        &self,
        file_path: &Path,
        optimized_path: &Path
    ) -> Result<Option<ProcessedFile>> {
        let metadata = tokio::fs::metadata(file_path).await?;
        let skipped_file = ProcessedFile::new(
            file_path.to_path_buf(),
            metadata.modified()
                .unwrap_or(SystemTime::UNIX_EPOCH)
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            metadata.len(),
            metadata.len(), // Stessa dimensione dell'originale
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        );
        
        // Per modalità output directory, copia file originale
        if self.config.output_path.is_some() && !self.config.dry_run {
            let original_output_path = self.get_expected_output_path(file_path)?;
            PathResolver::ensure_parent_dirs(&original_output_path).await?;
            std::fs::copy(file_path, &original_output_path)?;
            // debug!("Copied original file to output directory (insufficient reduction): {}", original_output_path.display());
        } else {
            // debug!("Cleaning up temporary file: {}", optimized_path.display());
            let _ = std::fs::remove_file(optimized_path);
        }
        
        // Marca come processato per evitare riprocessing (solo per ottimizzazione in-place)
        if self.config.output_path.is_none() {
            let mut state_manager = StateManager::new(&self.input_base_dir).await?;
            state_manager.mark_processed(skipped_file.clone()).await?;
            // debug!("Marked file as processed (skipped)");
        }
        
        Ok(Some(skipped_file))
    }
}
