//! # File Management Module
//!
//! Questo modulo gestisce tutte le operazioni sui file e la discovery di media.
//! 
//! ## Responsabilità:
//! - Discovery ricorsiva di file media in directory
//! - Determinazione formato file (immagine vs video)
//! - Operazioni sicure sui file con backup automatici
//! - Utilità per calcoli dimensioni e percentuali
//! - Formattazione human-readable delle dimensioni
//! 
//! ## Formati supportati:
//! - **Immagini**: JPG, JPEG, PNG, WebP
//! - **Video**: MP4, MOV, AVI, MKV, WebM
//! 
//! ## Operazioni sui file:
//! - `find_media_files()`: Trova tutti i file media in una directory
//! - `is_image()` / `is_video()`: Determina tipo di file
//! - `get_file_info()`: Ottiene dimensione e modification time
//! - `replace_file()`: Sostituzione sicura con backup
//! 
//! ## Sicurezza operazioni:
//! - Backup automatico prima della sostituzione
//! - Rollback in caso di errore durante la sostituzione
//! - Validazione esistenza file prima delle operazioni
//! 
//! ## Utilità:
//! - `format_size()`: Converte bytes in formato leggibile (KB, MB, GB)
//! - `calculate_reduction()`: Calcola percentuale di riduzione
//! 
//! ## Esempio:
//! ```rust
//! let files = FileManager::find_media_files("/path/to/media")?;
//! for file in files {
//!     if FileManager::is_image(&file) {
//!         // process image
//!     }
//! }
//! ```

use anyhow::Result;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tokio::fs;
use walkdir::WalkDir;

/// Manages file operations and discovery
pub struct FileManager;

impl FileManager {
    /// Get information about a file (size and modification time)
    pub async fn get_file_info(path: &Path) -> Result<(u64, u64)> {
        let metadata = fs::metadata(path).await?;
        let size = metadata.len();
        let modified = metadata
            .modified()?
            .duration_since(SystemTime::UNIX_EPOCH)?
            .as_secs();
        Ok((size, modified))
    }
    
    /// Find all supported media files in a directory
    pub fn find_media_files(media_dir: &Path) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        
        for entry in WalkDir::new(media_dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
        {
            let path = entry.path();
            if Self::is_supported_format(path) {
                files.push(path.to_path_buf());
            }
        }
        
        Ok(files)
    }
    
    /// Check if a file format is supported
    pub fn is_supported_format(path: &Path) -> bool {
        if let Some(ext) = path.extension() {
            let ext_lower = ext.to_string_lossy().to_lowercase();
            matches!(
                ext_lower.as_str(),
                "jpg" | "jpeg" | "png" | "webp" | "mp4" | "mov" | "avi" | "mkv" | "webm"
            )
        } else {
            false
        }
    }
    
    /// Check if a file is an image
    pub fn is_image(path: &Path) -> bool {
        if let Some(ext) = path.extension() {
            let ext_lower = ext.to_string_lossy().to_lowercase();
            matches!(ext_lower.as_str(), "jpg" | "jpeg" | "png" | "webp")
        } else {
            false
        }
    }
    
    /// Check if a file is a video
    pub fn is_video(path: &Path) -> bool {
        if let Some(ext) = path.extension() {
            let ext_lower = ext.to_string_lossy().to_lowercase();
            matches!(ext_lower.as_str(), "mp4" | "mov" | "avi" | "mkv" | "webm")
        } else {
            false
        }
    }
    
    /// Safely replace a file with its optimized version
    pub async fn replace_file(original: &Path, optimized: &Path) -> Result<()> {
        // Create backup first
        let backup_path = original.with_extension(
            format!("{}.backup", original.extension().unwrap_or_default().to_string_lossy())
        );
        
        // Copy original to backup
        fs::copy(original, &backup_path).await?;
        
        // Replace original with optimized
        match fs::copy(optimized, original).await {
            Ok(_) => {
                // Success - remove backup
                let _ = fs::remove_file(&backup_path).await;
                Ok(())
            }
            Err(e) => {
                // Failure - restore from backup
                let _ = fs::copy(&backup_path, original).await;
                let _ = fs::remove_file(&backup_path).await;
                Err(e.into())
            }
        }
    }
    
    /// Get human-readable file size
    pub fn format_size(size: u64) -> String {
        const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
        let mut size = size as f64;
        let mut unit_index = 0;
        
        while size >= 1024.0 && unit_index < UNITS.len() - 1 {
            size /= 1024.0;
            unit_index += 1;
        }
        
        if unit_index == 0 {
            format!("{} {}", size as u64, UNITS[unit_index])
        } else {
            format!("{:.2} {}", size, UNITS[unit_index])
        }
    }
    
    /// Calculate percentage reduction
    pub fn calculate_reduction(original_size: u64, new_size: u64) -> f64 {
        if original_size == 0 {
            0.0
        } else {
            ((original_size as f64 - new_size as f64) / original_size as f64) * 100.0
        }
    }
}
