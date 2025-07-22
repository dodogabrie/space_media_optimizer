//! # Progress Tracking and Statistics Module
//!
//! Questo modulo gestisce il progress tracking e le statistiche di ottimizzazione.
//! 
//! ## Responsabilità:
//! - Progress bar visual con `indicatif` per feedback real-time
//! - Tracking statistiche di ottimizzazione (file processati, saved, errors)
//! - Calcolo percentuali di riduzione e byte risparmiati
//! - Report finali con statistiche aggregate
//! - Spinner per operazioni indeterminate
//! 
//! ## Componenti principali:
//! - `ProgressManager`: Gestisce progress bar principale
//! - `OptimizationStats`: Traccia statistiche cumulative
//! 
//! ## Progress tracking:
//! - Barra di progresso con percentuale completamento
//! - Tempo elapsed e ETA
//! - Messaggi di stato per ogni file
//! - Spinner animato per operazioni lunghe
//! 
//! ## Statistiche tracciate:
//! - **files_processed**: Totale file elaborati
//! - **files_optimized**: File effettivamente ottimizzati (sostituiti)
//! - **files_skipped**: File saltati (riduzione insufficiente)
//! - **total_bytes_saved**: Byte totali risparmiati
//! - **total_original_size**: Dimensione totale file originali
//! - **errors**: Numero di errori durante processing
//! 
//! ## Report finali:
//! - Riepilogo operazione corrente
//! - Statistiche storiche da state files
//! - Percentuale riduzione media
//! - Byte risparmiati formattati (KB, MB, GB)
//! 
//! ## Visual feedback:
//! ```
//! ⠋ [00:02:15] [████████████████████████████████████████] 150/150 (100%) ✅ photo.jpg: 45.2% saved
//! ```
//! 
//! ## Esempio:
//! ```rust
//! let progress = ProgressManager::new(total_files);
//! let mut stats = OptimizationStats::new();
//! 
//! // Per ogni file processato:
//! stats.add_optimized(original_size, new_size);
//! progress.update("Processing file.jpg");
//! 
//! // Alla fine:
//! progress.finish(&stats.format_summary());
//! ```

use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

/// Manages progress reporting for media optimization
#[derive(Clone)]
pub struct ProgressManager {
    bar: ProgressBar,
}

impl ProgressManager {
    /// Create a new progress manager
    pub fn new(total_files: u64) -> Self {
        let bar = ProgressBar::new(total_files);
        
        bar.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({percent}%) {msg}")
                .unwrap()
                .progress_chars("=>-"),
        );
        
        bar.enable_steady_tick(Duration::from_millis(100));
        
        Self { bar }
    }
    
    /// Update progress with a message
    pub fn update(&self, message: &str) {
        self.bar.inc(1);
        self.bar.set_message(message.to_string());
    }
    
    /// Set a custom message without incrementing
    pub fn set_message(&self, message: &str) {
        self.bar.set_message(message.to_string());
    }
    
    /// Finish with a final message
    pub fn finish(&self, message: &str) {
        self.bar.finish_with_message(message.to_string());
    }
    
    /// Create a spinner for indeterminate progress
    pub fn spinner(message: &str) -> ProgressBar {
        let spinner = ProgressBar::new_spinner();
        
        spinner.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} {msg}")
                .unwrap(),
        );
        
        spinner.set_message(message.to_string());
        spinner.enable_steady_tick(Duration::from_millis(100));
        
        spinner
    }
}

/// Statistics tracker for optimization results
#[derive(Debug, Default)]
pub struct OptimizationStats {
    pub files_processed: usize,
    pub files_optimized: usize,
    pub files_skipped: usize,
    pub total_bytes_saved: u64,
    pub total_original_size: u64,
    pub errors: usize,
}

impl OptimizationStats {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn add_optimized(&mut self, original_size: u64, new_size: u64) {
        self.files_processed += 1;
        self.files_optimized += 1;
        self.total_original_size += original_size;
        self.total_bytes_saved += original_size.saturating_sub(new_size);
    }
    
    pub fn add_skipped(&mut self, original_size: u64) {
        self.files_processed += 1;
        self.files_skipped += 1;
        self.total_original_size += original_size;
    }
    
    pub fn add_error(&mut self) {
        self.files_processed += 1;
        self.errors += 1;
    }
    
    pub fn overall_reduction_percent(&self) -> f64 {
        if self.total_original_size > 0 {
            (self.total_bytes_saved as f64 / self.total_original_size as f64) * 100.0
        } else {
            0.0
        }
    }
    
    pub fn format_summary(&self) -> String {
        format!(
            "Processed: {} files | Optimized: {} | Skipped: {} | Errors: {} | Total saved: {} ({:.2}%)",
            self.files_processed,
            self.files_optimized,
            self.files_skipped,
            self.errors,
            crate::file_manager::FileManager::format_size(self.total_bytes_saved),
            self.overall_reduction_percent()
        )
    }
}
