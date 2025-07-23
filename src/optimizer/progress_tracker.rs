//! # Progress Tracking Module
//!
//! Unifica GlobalProgress e ProgressManager in un singolo tracker thread-safe.
//! Gestisce sia output JSON che progress bar tradizionale.

use crate::{
    config::Config,
    json_output::JsonMessage,
    progress::{OptimizationStats, ProgressManager},
    state::ProcessedFile,
};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::Mutex;

/// Tracker progress unificato che sostituisce GlobalProgress + ProgressManager
#[derive(Clone)]
pub struct ProgressTracker {
    pub total_files: usize,
    current_file: Arc<Mutex<usize>>,
    files_optimized: Arc<Mutex<usize>>,
    files_skipped: Arc<Mutex<usize>>,
    errors: Arc<Mutex<usize>>,
    bytes_saved: Arc<Mutex<u64>>,
    progress_manager: ProgressManager,
}

impl ProgressTracker {
    /// Crea un nuovo tracker
    pub fn new(total_files: usize) -> Self {
        Self {
            total_files,
            current_file: Arc::new(Mutex::new(0)),
            files_optimized: Arc::new(Mutex::new(0)),
            files_skipped: Arc::new(Mutex::new(0)),
            errors: Arc::new(Mutex::new(0)),
            bytes_saved: Arc::new(Mutex::new(0)),
            progress_manager: ProgressManager::new(total_files as u64),
        }
    }
    
    /// Aggiorna progress e invia eventi JSON se necessario
    pub async fn emit_progress(&self, config: &Config) {
        if config.json_output {
            let current = *self.current_file.lock().await;
            let optimized = *self.files_optimized.lock().await;
            let skipped = *self.files_skipped.lock().await;
            let errors = *self.errors.lock().await;
            let saved = *self.bytes_saved.lock().await;
            
            JsonMessage::progress(
                current,
                self.total_files,
                optimized,
                skipped,
                errors,
                saved,
            ).emit();
        }
    }
    
    /// Aggiorna progress bar con messaggio
    pub fn update_message(&self, message: &str) {
        self.progress_manager.update(message);
    }
    
    /// Finalizza progress bar
    pub fn finish(&self, summary: &str) {
        self.progress_manager.finish(summary);
    }
    
    /// Incrementa file corrente
    pub async fn increment_current(&self) {
        let mut current = self.current_file.lock().await;
        *current += 1;
    }
    
    /// Aggiunge file ottimizzato
    pub async fn add_optimized(&self, bytes_saved: u64) {
        let mut optimized = self.files_optimized.lock().await;
        *optimized += 1;
        let mut saved = self.bytes_saved.lock().await;
        *saved += bytes_saved;
    }
    
    /// Aggiunge file skippato
    pub async fn add_skipped(&self) {
        let mut skipped = self.files_skipped.lock().await;
        *skipped += 1;
    }
    
    /// Aggiunge errore
    pub async fn add_error(&self) {
        let mut errors = self.errors.lock().await;
        *errors += 1;
    }
    
    /// Gestisce completamento file con eventi JSON automatici
    pub async fn handle_file_completion(
        &self,
        config: &Config,
        file_path: &std::path::Path,
        result: &anyhow::Result<Option<ProcessedFile>>
    ) {
        match result {
            Ok(Some(processed)) => {
                self.increment_current().await;
                
                let was_skipped = processed.original_size == processed.optimized_size;
                
                if was_skipped {
                    self.add_skipped().await;
                    
                    if config.json_output {
                        JsonMessage::file_complete(processed, true, None).emit();
                        self.emit_progress(config).await;
                    }
                    
                    let message = format!("[SKIP] {}: No optimization needed", 
                                       file_path.file_name().unwrap_or_default().to_string_lossy());
                    self.update_message(&message);
                } else {
                    let bytes_saved = processed.original_size.saturating_sub(processed.optimized_size);
                    self.add_optimized(bytes_saved).await;
                    
                    if config.json_output {
                        JsonMessage::file_complete(processed, false, None).emit();
                        self.emit_progress(config).await;
                    }
                    
                    let message = format!("[OK] {}: {:.1}% saved", 
                                       file_path.file_name().unwrap_or_default().to_string_lossy(),
                                       processed.reduction_percent);
                    self.update_message(&message);
                }
            }
            Ok(None) => {
                self.increment_current().await;
                self.add_skipped().await;
                
                if config.json_output {
                    // Crea dummy ProcessedFile per file skippato
                    if let Ok(metadata) = tokio::fs::metadata(file_path).await {
                        let dummy_processed = ProcessedFile::new(
                            file_path.to_path_buf(),
                            metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH)
                                .duration_since(SystemTime::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs(),
                            metadata.len(),
                            metadata.len(),
                            SystemTime::now()
                                .duration_since(SystemTime::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs(),
                        );
                        JsonMessage::file_complete(&dummy_processed, true, None).emit();
                    }
                    self.emit_progress(config).await;
                }
                
                let message = format!("[SKIP] {}: skipped", 
                               file_path.file_name().unwrap_or_default().to_string_lossy());
                self.update_message(&message);
            }
            Err(e) => {
                self.increment_current().await;
                self.add_error().await;
                
                if config.json_output {
                    // Crea dummy ProcessedFile per errore
                    if let Ok(metadata) = tokio::fs::metadata(file_path).await {
                        let dummy_processed = ProcessedFile::new(
                            file_path.to_path_buf(),
                            metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH)
                                .duration_since(SystemTime::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs(),
                            metadata.len(),
                            metadata.len(),
                            SystemTime::now()
                                .duration_since(SystemTime::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs(),
                        );
                        JsonMessage::file_complete(&dummy_processed, false, Some(e.to_string())).emit();
                    }
                    self.emit_progress(config).await;
                }
                
                let message = format!("[ERROR] {}: error", 
                               file_path.file_name().unwrap_or_default().to_string_lossy());
                self.update_message(&message);
            }
        }
    }
    
    /// Ottieni statistiche per report finale
    pub async fn get_stats(&self) -> OptimizationStats {
        let mut stats = OptimizationStats::new();
        
        let optimized = *self.files_optimized.lock().await;
        let skipped = *self.files_skipped.lock().await;
        let errors = *self.errors.lock().await;
        let saved = *self.bytes_saved.lock().await;
        
        // Popola le stats con valori reali
        stats.files_processed = optimized + skipped;
        stats.files_optimized = optimized;
        stats.files_skipped = skipped;
        stats.errors = errors;
        stats.total_bytes_saved = saved;
        
        stats
    }
}
