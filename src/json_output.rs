//! # JSON Output Module
//!
//! Questo modulo gestisce l'output strutturato in JSON per comunicazione con Python/Electron.
//! 
//! ## Responsabilità:
//! - Emette messaggi JSON strutturati per eventi di progresso
//! - Utilizza le strutture esistenti di ProcessedFile e StateManager
//! - Fornisce interfaccia standardizzata per comunicazione inter-processo
//! 
//! ## Tipi di messaggi:
//! - `start`: Inizio processo di ottimizzazione
//! - `progress`: Progresso corrente (file processato, stats)
//! - `file_start`: Inizio elaborazione di un file
//! - `file_complete`: Fine elaborazione di un file
//! - `complete`: Fine processo completo con statistiche finali
//! - `error`: Errore durante elaborazione

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use crate::state::ProcessedFile;

/// Tipo di messaggio JSON
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum JsonMessage {
    /// Inizio del processo di ottimizzazione
    #[serde(rename = "start")]
    Start {
        input_dir: PathBuf,
        output_dir: Option<PathBuf>,
        total_files: usize,
        config: JsonConfig,
    },
    
    /// Progresso corrente
    #[serde(rename = "progress")]
    Progress {
        current: usize,
        total: usize,
        percentage: f64,
        files_optimized: usize,
        files_skipped: usize,
        errors: usize,
        bytes_saved: u64,
    },
    
    /// Inizio elaborazione di un file specifico
    #[serde(rename = "file_start")]
    FileStart {
        path: PathBuf,
        size: u64,
        index: usize,
        total: usize,
    },
    
    /// Fine elaborazione di un file specifico
    #[serde(rename = "file_complete")]
    FileComplete {
        path: PathBuf,
        original_size: u64,
        optimized_size: u64,
        reduction_percent: f64,
        skipped: bool,
        error: Option<String>,
    },
    
    /// Processo completato
    #[serde(rename = "complete")]
    Complete {
        files_processed: usize,
        files_optimized: usize,
        files_skipped: usize,
        errors: usize,
        total_bytes_saved: u64,
        average_reduction: f64,
        duration_seconds: f64,
        historical_stats: HistoricalStats,
    },
    
    /// Errore generale
    #[serde(rename = "error")]
    Error {
        message: String,
        details: Option<String>,
    },
}

/// Configurazione per output JSON
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonConfig {
    pub jpeg_quality: u8,
    pub video_crf: u8,
    pub workers: usize,
    pub convert_to_webp: bool,
    pub webp_quality: u8,
    pub dry_run: bool,
}

/// Statistiche storiche
#[derive(Debug, Serialize, Deserialize)]
pub struct HistoricalStats {
    pub total_files_ever_processed: usize,
    pub total_bytes_saved_historically: u64,
    pub average_historical_reduction: f64,
}

impl JsonMessage {
    /// Emette il messaggio JSON su stdout
    pub fn emit(&self) {
        if let Ok(json) = serde_json::to_string(self) {
            println!("{}", json);
        }
    }
    
    /// Crea un messaggio di inizio
    pub fn start(
        input_dir: PathBuf,
        output_dir: Option<PathBuf>,
        total_files: usize,
        config: JsonConfig,
    ) -> Self {
        Self::Start {
            input_dir,
            output_dir,
            total_files,
            config,
        }
    }
    
    /// Crea un messaggio di progresso
    pub fn progress(
        current: usize,
        total: usize,
        files_optimized: usize,
        files_skipped: usize,
        errors: usize,
        bytes_saved: u64,
    ) -> Self {
        let percentage = if total > 0 {
            (current as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        
        Self::Progress {
            current,
            total,
            percentage,
            files_optimized,
            files_skipped,
            errors,
            bytes_saved,
        }
    }
    
    /// Crea un messaggio di inizio file
    pub fn file_start(path: PathBuf, size: u64, index: usize, total: usize) -> Self {
        Self::FileStart {
            path,
            size,
            index,
            total,
        }
    }
    
    /// Crea un messaggio di completamento file
    pub fn file_complete(processed_file: &ProcessedFile, skipped: bool, error: Option<String>) -> Self {
        Self::FileComplete {
            path: processed_file.path.clone(),
            original_size: processed_file.original_size,
            optimized_size: processed_file.optimized_size,
            reduction_percent: processed_file.reduction_percent,
            skipped,
            error,
        }
    }
    
    /// Crea un messaggio di completamento generale
    pub fn complete(
        files_processed: usize,
        files_optimized: usize,
        files_skipped: usize,
        errors: usize,
        total_bytes_saved: u64,
        average_reduction: f64,
        duration_seconds: f64,
        historical_stats: HistoricalStats,
    ) -> Self {
        Self::Complete {
            files_processed,
            files_optimized,
            files_skipped,
            errors,
            total_bytes_saved,
            average_reduction,
            duration_seconds,
            historical_stats,
        }
    }
    
    /// Crea un messaggio di errore
    pub fn error(message: String, details: Option<String>) -> Self {
        Self::Error { message, details }
    }
}

/// Converti Config esistente in JsonConfig
impl From<&crate::Config> for JsonConfig {
    fn from(config: &crate::Config) -> Self {
        Self {
            jpeg_quality: config.jpeg_quality,
            video_crf: config.video_crf,
            workers: config.workers,
            convert_to_webp: config.convert_to_webp,
            webp_quality: config.webp_quality,
            dry_run: config.dry_run,
        }
    }
}
