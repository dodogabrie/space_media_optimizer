//! # State Management Module
//!
//! Questo modulo gestisce il tracking dei file processati per evitare rielaborazioni.
//! 
//! ## Responsabilità:
//! - Traccia quali file sono già stati processati e quando
//! - Persiste lo stato in file JSON per directory specifiche
//! - Evita rielaborazione di file non modificati
//! - Fornisce statistiche sui file processati storicamente
//! - Cleanup automatico di entry per file che non esistono più
//! 
//! ## Strutture dati:
//! - `ProcessedFile`: Info su un file processato (path, size, reduction, timestamp)
//! - `StateFile`: Container per tutti i file processati di una directory
//! - `StateManager`: Gestisce operazioni di lettura/scrittura stato
//! 
//! ## Strategia di persistence:
//! - Un file JSON per directory media (basato su hash del path)
//! - Salvataggio in `~/.media-optimizer/processed_files_<hash>.json`
//! - Tracking basato su modification time del file
//! - Cleanup automatico di file inesistenti
//! 
//! ## Prevenzione rielaborazione:
//! - Controlla se file è già stato processato
//! - Compara modification time per detect modifiche
//! - Skip intelligente di file non modificati
//! 
//! ## Esempio struttura state file:
//! ```json
//! {
//!   "processed_files": {
//!     "/path/file.jpg": {
//!       "path": "/path/file.jpg",
//!       "modified_time": 1642680000,
//!       "original_size": 1048576,
//!       "optimized_size": 524288,
//!       "reduction_percent": 50.0,
//!       "processed_at": 1642680000
//!     }
//!   }
//! }
//! ```

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;

/// Information about a processed file
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProcessedFile {
    pub path: PathBuf,
    pub modified_time: u64,
    pub original_size: u64,
    pub optimized_size: u64,
    pub reduction_percent: f64,
    pub processed_at: u64,
}

impl ProcessedFile {
    pub fn new(
        path: PathBuf,
        modified_time: u64,
        original_size: u64,
        optimized_size: u64,
        processed_at: u64,
    ) -> Self {
        let reduction_percent = if original_size > 0 {
            (1.0 - (optimized_size as f64 / original_size as f64)) * 100.0
        } else {
            0.0
        };

        Self {
            path,
            modified_time,
            original_size,
            optimized_size,
            reduction_percent,
            processed_at,
        }
    }
}

/// State file to track processed files
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct StateFile {
    pub processed_files: HashMap<String, ProcessedFile>,
}

/// Manages the state of processed files
pub struct StateManager {
    state_file_path: PathBuf,
    state: StateFile,
}

impl StateManager {
    /// Create a new state manager for a specific media directory
    pub async fn new(media_dir: &Path) -> Result<Self> {
        let state_dir = dirs::home_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?
            .join(".media-optimizer");
        
        fs::create_dir_all(&state_dir).await?;
        
        // Create unique state file based on media directory hash
        let mut hasher = Sha256::new();
        hasher.update(media_dir.to_string_lossy().as_bytes());
        let hash = hex::encode(hasher.finalize())[..16].to_string();
        
        let state_file_path = state_dir.join(format!("processed_files_{}.json", hash));
        
        let state = if state_file_path.exists() {
            let content = fs::read_to_string(&state_file_path).await?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            StateFile::default()
        };
        
        Ok(Self {
            state_file_path,
            state,
        })
    }
    
    /// Save current state to file
    pub async fn save(&self) -> Result<()> {
        let content = serde_json::to_string_pretty(&self.state)?;
        fs::write(&self.state_file_path, content).await?;
        Ok(())
    }
    
    /// Check if a file has been processed and is up to date
    pub fn is_processed(&self, file_path: &Path, modified_time: u64) -> bool {
        if let Some(processed) = self.state.processed_files.get(&file_path.to_string_lossy().to_string()) {
            processed.modified_time == modified_time
        } else {
            false
        }
    }
    
    /// Mark a file as processed
    pub async fn mark_processed(&mut self, processed_file: ProcessedFile) -> Result<()> {
        self.state.processed_files.insert(
            processed_file.path.to_string_lossy().to_string(),
            processed_file,
        );
        self.save().await
    }
    
    /// Get statistics about processed files
    pub fn get_stats(&self) -> (usize, u64, f64) {
        let count = self.state.processed_files.len();
        let total_saved: u64 = self.state.processed_files
            .values()
            .map(|f| f.original_size.saturating_sub(f.optimized_size))
            .sum();
        let avg_reduction: f64 = if count > 0 {
            self.state.processed_files
                .values()
                .map(|f| f.reduction_percent)
                .sum::<f64>() / count as f64
        } else {
            0.0
        };
        
        (count, total_saved, avg_reduction)
    }
    
    /// Clean up old entries (files that no longer exist)
    pub async fn cleanup(&mut self) -> Result<()> {
        let mut to_remove = Vec::new();
        
        for (key, processed_file) in &self.state.processed_files {
            if !processed_file.path.exists() {
                to_remove.push(key.clone());
            }
        }
        
        let removed_count = to_remove.len();
        
        for key in to_remove {
            self.state.processed_files.remove(&key);
        }
        
        if removed_count > 0 {
            self.save().await?;
        }
        
        Ok(())
    }
}
