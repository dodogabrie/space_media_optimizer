//! # Configuration Management Module
//!
//! Questo modulo gestisce tutta la configurazione dell'applicazione.
//! 
//! ## Responsabilità:
//! - Definisce la struct `Config` con tutti i parametri di ottimizzazione
//! - Fornisce validazione robusta dei parametri di input
//! - Supporta caricamento/salvataggio configurazione da/verso file JSON
//! - Fornisce valori di default sensati per tutti i parametri
//! 
//! ## Parametri di configurazione:
//! - `jpeg_quality`: Qualità JPEG (1-100, default: 80)
//! - `video_crf`: CRF video (0-51, default: 26, più basso = migliore qualità)
//! - `audio_bitrate`: Bitrate audio video (default: "128k")
//! - `size_threshold`: Soglia per sostituire file (0.0-1.0, default: 0.9)
//! - `dry_run`: Flag per simulazione senza modifiche (default: false)
//! - `workers`: Numero di worker paralleli (default: 4)
//! - `output_path`: Directory di output per file ottimizzati (default: None = replace in place)
//! - `convert_to_webp`: Converte tutti i media a formato WebP (default: false)
//! - `webp_quality`: Qualità WebP (1-100, default: 80)
//! 
//! ## Validazione:
//! - Controlla che jpeg_quality sia 1-100
//! - Controlla che video_crf sia 0-51
//! - Controlla che size_threshold sia 0.0-1.0
//! - Controlla che workers sia > 0
//! 
//! ## Esempio:
//! ```rust
//! let config = Config {
//!     jpeg_quality: 85,
//!     video_crf: 24,
//!     workers: 8,
//!     ..Default::default()
//! };
//! config.validate()?;
//! ```

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuration for media optimization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// JPEG quality (1-100)
    pub jpeg_quality: u8,
    /// Video CRF value (0-51, lower = better quality)
    pub video_crf: u8,
    /// Video audio bitrate
    pub audio_bitrate: String,
    /// Size threshold (keep if new size < original * threshold)
    pub size_threshold: f64,
    /// Dry run - don't actually replace files
    pub dry_run: bool,
    /// Number of parallel workers
    pub workers: usize,
    /// Output directory for optimized files (None = replace in place)
    pub output_path: Option<PathBuf>,
    /// Convert all media to WebP format
    pub convert_to_webp: bool,
    /// WebP quality (1-100)
    pub webp_quality: u8,
    /// Skip files that have already been processed (even when using output directory)
    pub keep_processed: bool,
    /// Skip video compression (just copy videos to output)
    pub skip_video_compression: bool,
    /// Output progress and status as JSON for programmatic use
    pub json_output: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            jpeg_quality: 80,
            video_crf: 26,
            audio_bitrate: "128k".to_string(),
            size_threshold: 0.9,
            dry_run: false,
            workers: 4,
            output_path: None,
            convert_to_webp: false,
            webp_quality: 80,
            keep_processed: false,
            skip_video_compression: false,
            json_output: false,
        }
    }
}

impl Config {
    /// Validate configuration parameters
    pub fn validate(&self) -> Result<()> {
        if self.jpeg_quality == 0 || self.jpeg_quality > 100 {
            return Err(anyhow::anyhow!("JPEG quality must be between 1 and 100"));
        }
        
        if self.webp_quality == 0 || self.webp_quality > 100 {
            return Err(anyhow::anyhow!("WebP quality must be between 1 and 100"));
        }
        
        if self.video_crf > 51 {
            return Err(anyhow::anyhow!("Video CRF must be between 0 and 51"));
        }
        
        if self.size_threshold <= 0.0 || self.size_threshold > 1.0 {
            return Err(anyhow::anyhow!("Size threshold must be between 0.0 and 1.0"));
        }
        
        if self.workers == 0 {
            return Err(anyhow::anyhow!("Number of workers must be greater than 0"));
        }
        
        // Validate output path if specified
        if let Some(ref output_path) = self.output_path {
            if !output_path.exists() {
                return Err(anyhow::anyhow!("Output path does not exist: {}", output_path.display()));
            }
            if !output_path.is_dir() {
                return Err(anyhow::anyhow!("Output path is not a directory: {}", output_path.display()));
            }
        }
        
        Ok(())
    }
    
    /// Load configuration from file
    pub async fn from_file(path: &PathBuf) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }
        
        let content = tokio::fs::read_to_string(path).await?;
        let config: Config = serde_json::from_str(&content)?;
        config.validate()?;
        Ok(config)
    }
    
    /// Save configuration to file
    pub async fn save_to_file(&self, path: &PathBuf) -> Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        tokio::fs::write(path, content).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_config_validation() {
        let mut config = Config::default();
        assert!(config.validate().is_ok());

        config.jpeg_quality = 0;
        assert!(config.validate().is_err());

        config.jpeg_quality = 80;
        config.video_crf = 52;
        assert!(config.validate().is_err());

        config.video_crf = 26;
        config.size_threshold = 1.5;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert_eq!(config.jpeg_quality, 80);
        assert_eq!(config.video_crf, 26);
        assert_eq!(config.audio_bitrate, "128k");
        assert_eq!(config.size_threshold, 0.9);
        assert!(!config.dry_run);
        assert_eq!(config.workers, 4);
    }

    #[tokio::test]
    async fn test_config_save_load() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.json");

        let original_config = Config {
            jpeg_quality: 85,
            video_crf: 24,
            audio_bitrate: "192k".to_string(),
            size_threshold: 0.85,
            dry_run: true,
            workers: 8,
        };

        // Save config
        original_config.save_to_file(&config_path).await.unwrap();

        // Load config
        let loaded_config = Config::from_file(&config_path).await.unwrap();

        assert_eq!(loaded_config.jpeg_quality, 85);
        assert_eq!(loaded_config.video_crf, 24);
        assert_eq!(loaded_config.audio_bitrate, "192k");
        assert_eq!(loaded_config.size_threshold, 0.85);
        assert!(loaded_config.dry_run);
        assert_eq!(loaded_config.workers, 8);
    }
}
