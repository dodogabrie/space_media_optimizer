//! # Path Resolution Module
//!
//! Centralizza tutta la logica di calcolo dei path di output.
//! Evita duplicazione tra ImageProcessor e TaskOptimizer.

use crate::{config::Config, file_manager::FileManager};
use anyhow::Result;
use std::path::{Path, PathBuf};
use tracing::debug;

/// Utility per calcolare i path di output in modo centralizzato
pub struct PathResolver;

impl PathResolver {
    /// Calcola il path di output per un file dato
    pub fn get_output_path(
        input_path: &Path, 
        input_base_dir: &Path, 
        config: &Config
    ) -> Result<PathBuf> {
        let file_stem = input_path.file_stem()
            .ok_or_else(|| anyhow::anyhow!("Invalid file name: {}", input_path.display()))?
            .to_string_lossy();
        
        // Determina l'estensione di output
        let extension = Self::get_output_extension(input_path, config);
        let filename = format!("{}.{}", file_stem, extension);
        
        if let Some(ref output_dir) = config.output_path {
            // Modalità output directory
            Self::resolve_output_directory_path(input_path, input_base_dir, output_dir, filename)
        } else {
            // Modalità in-place
            Ok(input_path.with_file_name(filename))
        }
    }
    
    /// Determina l'estensione di output basata sul tipo file e config
    fn get_output_extension(input_path: &Path, config: &Config) -> String {
        if FileManager::is_video(input_path) {
            "mp4".to_string()
        } else if config.convert_to_webp {
            "webp".to_string()
        } else {
            input_path.extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("jpg")
                .to_string()
        }
    }
    
    /// Risolve il path per modalità output directory
    fn resolve_output_directory_path(
        input_path: &Path,
        input_base_dir: &Path, 
        output_dir: &Path,
        filename: String
    ) -> Result<PathBuf> {
        // Canonicalizza i path per evitare problemi
        let canonical_base = input_base_dir.canonicalize()
            .map_err(|e| anyhow::anyhow!("Failed to canonicalize base dir {}: {}", input_base_dir.display(), e))?;
        
        let canonical_output = output_dir.canonicalize()
            .map_err(|e| anyhow::anyhow!("Failed to canonicalize output dir {}: {}", output_dir.display(), e))?;
        
        debug!("Calculating path for: {}", input_path.display());
        debug!("Canonical base: {}", canonical_base.display());
        debug!("Canonical output: {}", canonical_output.display());
        
        // Calcola path relativo
        let relative_path = match input_path.strip_prefix(&canonical_base) {
            Ok(rel) => {
                debug!("[OK] Strip prefix successful: {}", rel.display());
                rel.parent().unwrap_or(Path::new(""))
            }
            Err(e) => {
                debug!("[ERROR] Strip prefix failed: {} - fallback to parent", e);
                input_path.parent().unwrap_or(Path::new(""))
            }
        };
        
        let result = canonical_output.join(relative_path).join(filename);
        debug!("Resolved output path: {} -> {}", input_path.display(), result.display());
        
        Ok(result)
    }
    
    /// Crea le directory parent se necessario
    pub async fn ensure_parent_dirs(path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await
                .map_err(|e| anyhow::anyhow!("Failed to create parent directories for {}: {}", path.display(), e))?;
        }
        Ok(())
    }
}
