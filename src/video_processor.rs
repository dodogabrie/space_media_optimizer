//! # Video Processing Module
//!
//! Questo modulo gestisce l'ottimizzazione di tutti i formati video supportati.
//! 
//! ## Responsabilità:
//! - Compressione video con FFmpeg
//! - Controllo qualità tramite CRF (Constant Rate Factor)
//! - Ricodifica audio con AAC e bitrate configurabile
//! - Preservazione completa metadata video
//! - Analisi proprietà video con ffprobe
//! - Verifica dipendenze esterne (ffmpeg, ffprobe, exiftool)
//! 
//! ## Formati supportati:
//! - **Input**: MP4, MOV, AVI, MKV, WebM
//! - **Output**: MP4 (H.264 + AAC) per massima compatibilità
//! 
//! ## Pipeline di compressione:
//! 1. Analizza video originale con ffprobe (opzionale)
//! 2. Comprime con FFmpeg usando parametri ottimizzati:
//!    - Codec video: libx264
//!    - Preset: veryslow (migliore compressione)
//!    - CRF: configurabile (default 26)
//!    - Codec audio: AAC
//!    - Bitrate audio: configurabile (default 128k)
//! 3. Preserva metadata con exiftool
//! 4. Ritorna file temporaneo ottimizzato
//! 
//! ## Controllo qualità (CRF):
//! - 0-17: Visualmente lossless (file grandi)
//! - 18-23: Alta qualità (raccomandato per archivio)
//! - 24-28: Buona qualità (default, bilanciato)
//! - 29-35: Qualità accettabile (file piccoli)
//! - 36+: Bassa qualità (non raccomandato)
//! 
//! ## Preservazione metadata:
//! - Copia tutti i metadati originali
//! - Mantiene timestamp, GPS, info camera
//! - Usa flag extractEmbedded per metadata embedded
//! 
//! ## Analisi video (VideoInfo):
//! - Durata, bitrate, risoluzione
//! - Codec originale
//! - Stima dimensione compressa
//! 
//! ## Dipendenze richieste:
//! - `ffmpeg`: Compressione video
//! - `ffprobe`: Analisi proprietà video
//! - `exiftool`: Preservazione metadata
//! 
//! ## Esempio:
//! ```rust
//! let processor = VideoProcessor::new(config);
//! let optimized = processor.optimize(&video_path).await?;
//! let info = processor.get_video_info(&video_path).await?;
//! ```

use crate::config::Config;
use crate::error::OptimizeError;
use crate::platform::PlatformCommands;
use anyhow::Result;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::NamedTempFile;
use tracing::{debug, warn};

/// Handles video optimization
pub struct VideoProcessor {
    config: Config,
}

impl VideoProcessor {
    pub fn new(config: Config) -> Self {
        Self { config }
    }
    
    /// Get the output path for an optimized video file
    fn get_output_path(&self, input_path: &Path, input_base_dir: &Path) -> PathBuf {
        let file_stem = input_path.file_stem().unwrap_or_default().to_string_lossy();
        let filename = format!("{}.mp4", file_stem);
        
        if let Some(ref output_dir) = self.config.output_path {
            // If output directory is specified, preserve directory structure relative to input base
            // Canonicalize both paths to ensure strip_prefix works correctly (same as image processor)
            let canonical_input = input_path.canonicalize().unwrap_or_else(|_| input_path.to_path_buf());
            let canonical_base = input_base_dir.canonicalize().unwrap_or_else(|_| input_base_dir.to_path_buf());
            
            let relative_path = match canonical_input.strip_prefix(&canonical_base) {
                Ok(rel) => {
                    rel.parent().unwrap_or(Path::new(""))
                }
                Err(_) => {
                    input_path.parent().unwrap_or(Path::new(""))
                }
            };
            
            output_dir.join(relative_path).join(filename)
        } else {
            // Replace in place - use same directory with optimized extension
            input_path.with_file_name(format!("optimized.mp4"))
        }
    }
    
    /// Optimize a video file
    pub async fn optimize(&self, input_path: &Path, input_base_dir: &Path) -> Result<PathBuf> {
        debug!("Starting video optimization for: {}", input_path.display());
        
        // Add timeout to prevent hanging on problematic videos
        tokio::time::timeout(
            std::time::Duration::from_secs(600), // 10 minutes per video
            self.optimize_internal(input_path, input_base_dir)
        ).await.map_err(|_| anyhow::anyhow!("Video optimization timed out for: {}", input_path.display()))?
    }
    
    async fn optimize_internal(&self, input_path: &Path, input_base_dir: &Path) -> Result<PathBuf> {
        eprintln!("🎬 Starting video optimization: {}", input_path.file_name().unwrap_or_default().to_string_lossy());
        
        // Get the final output path
        let final_output_path = self.get_output_path(input_path, input_base_dir);
        
        // Create output directory if it doesn't exist
        if let Some(parent) = final_output_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        
        if self.config.skip_video_compression {
            // Just copy the original video without compression
            eprintln!("⏩ Skipping video compression, copying original: {}", input_path.file_name().unwrap_or_default().to_string_lossy());
            std::fs::copy(input_path, &final_output_path)?;
            eprintln!("✅ Video copied without compression: {}", input_path.file_name().unwrap_or_default().to_string_lossy());
            return Ok(final_output_path);
        }
        
        let temp_file = NamedTempFile::with_suffix(".mp4")?;
        let temp_path = temp_file.path().to_path_buf();
        
        // Use FFmpeg to compress the video
        self.compress_video(input_path, &temp_path).await?;
        
        // Preserve metadata
        debug!("📝 Preserving video metadata...");
        self.preserve_metadata(input_path, &temp_path).await?;
        
        // Copy optimized file to final location
        eprintln!("💾 Saving optimized video to: {}", final_output_path.display());
        std::fs::copy(&temp_path, &final_output_path)?;
        
        eprintln!("✅ Video optimization completed: {}", input_path.file_name().unwrap_or_default().to_string_lossy());
        
        // The NamedTempFile will be deleted automatically when temp_file goes out of scope
        Ok(final_output_path)
    }
    
    async fn compress_video(&self, input_path: &Path, output_path: &Path) -> Result<()> {
        debug!(
            "🎬 Compressing video: {} (CRF: {}, audio: {})",
            input_path.file_name().unwrap_or_default().to_string_lossy(),
            self.config.video_crf,
            self.config.audio_bitrate
        );
        
        let platform = PlatformCommands::instance();
        let ffmpeg_cmd = platform.get_command("ffmpeg");
        
        let mut cmd = Command::new(ffmpeg_cmd);
        cmd.args([
            "-i", input_path.to_str().unwrap(),
            "-c:v", "libx264",
            "-preset", "veryslow",
            "-crf", &self.config.video_crf.to_string(),
            "-c:a", "aac",
            "-b:a", &self.config.audio_bitrate,
            "-map_metadata", "0",
            "-movflags", "use_metadata_tags",
            "-y", output_path.to_str().unwrap(),
        ]);
        
        // Suppress FFmpeg output unless in debug mode
        if !tracing::enabled!(tracing::Level::DEBUG) {
            cmd.args(["-loglevel", "warning"]);
        } else {
            cmd.args(["-progress", "pipe:2"]);
        }
        
        eprintln!("🔄 Starting FFmpeg compression...");
        let start_time = std::time::Instant::now();
        
        let output = cmd.output()
            .map_err(|e| anyhow::anyhow!("Failed to execute {}: {}", ffmpeg_cmd, e))?;
        
        let duration = start_time.elapsed();
        
        if !output.status.success() {
            eprintln!("❌ FFmpeg failed after {:.1}s: {}", duration.as_secs_f64(), String::from_utf8_lossy(&output.stderr));
            return Err(OptimizeError::FFmpeg(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ).into());
        }
        
        eprintln!("✅ Video compression completed in {:.1}s", duration.as_secs_f64());
        
        Ok(())
    }
    
    async fn preserve_metadata(&self, source: &Path, target: &Path) -> Result<()> {
        debug!("Preserving video metadata");
        
        let platform = PlatformCommands::instance();
        let exiftool_cmd = platform.get_command("exiftool");
        
        let output = Command::new(exiftool_cmd)
            .args([
                "-tagsFromFile",
                source.to_str().unwrap(),
                "-extractEmbedded",
                "-all:all",
                "-FileModifyDate",
                "-overwrite_original",
                target.to_str().unwrap(),
            ])
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to execute exiftool: {}", e))?;
        
        if !output.status.success() {
            warn!(
                "Failed to preserve video metadata for {}: {}",
                source.display(),
                String::from_utf8_lossy(&output.stderr)
            );
            // Don't fail the whole operation for metadata issues
        }
        
        Ok(())
    }
    
    /// Get video information using ffprobe
    pub async fn get_video_info(&self, video_path: &Path) -> Result<VideoInfo> {
        let platform = PlatformCommands::instance();
        let ffprobe_cmd = platform.get_command("ffprobe");
        
        let output = Command::new(ffprobe_cmd)
            .args([
                "-v", "quiet",
                "-print_format", "json",
                "-show_format",
                "-show_streams",
                video_path.to_str().unwrap(),
            ])
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to execute ffprobe: {}", e))?;
        
        if !output.status.success() {
            return Err(OptimizeError::FFmpeg(
                String::from_utf8_lossy(&output.stderr).to_string(),
            ).into());
        }
        
        let info_str = String::from_utf8_lossy(&output.stdout);
        let info: serde_json::Value = serde_json::from_str(&info_str)?;
        
        // Extract basic video information
        let format = &info["format"];
        let duration = format["duration"]
            .as_str()
            .and_then(|d| d.parse::<f64>().ok())
            .unwrap_or(0.0);
        
        let bitrate = format["bit_rate"]
            .as_str()
            .and_then(|b| b.parse::<u64>().ok())
            .unwrap_or(0);
        
        // Find video stream
        let empty_vec = vec![];
        let streams = info["streams"].as_array().unwrap_or(&empty_vec);
        let video_stream = streams.iter()
            .find(|s| s["codec_type"] == "video")
            .unwrap_or(&serde_json::Value::Null);
        
        let width = video_stream["width"].as_u64().unwrap_or(0) as u32;
        let height = video_stream["height"].as_u64().unwrap_or(0) as u32;
        let codec = video_stream["codec_name"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();
        
        Ok(VideoInfo {
            duration,
            bitrate,
            width,
            height,
            codec,
        })
    }
    
    /// Check if required tools are available
    pub async fn check_dependencies() -> Result<()> {
        let platform = PlatformCommands::instance();
        let tools = ["ffmpeg", "ffprobe", "exiftool"];
        
        for tool in &tools {
            if !platform.is_command_available(tool).await {
                return Err(OptimizeError::MissingDependency(
                    format!("{} is required for video processing", tool)
                ).into());
            }
        }
        
        Ok(())
    }
}

/// Video file information
#[derive(Debug, Clone)]
pub struct VideoInfo {
    pub duration: f64,
    pub bitrate: u64,
    pub width: u32,
    pub height: u32,
    pub codec: String,
}

impl VideoInfo {
    /// Check if video needs optimization based on quality/size
    pub fn needs_optimization(&self, target_bitrate: u64) -> bool {
        self.bitrate > target_bitrate
    }
    
    /// Estimate compressed size
    pub fn estimate_compressed_size(&self, target_bitrate: u64) -> u64 {
        ((target_bitrate as f64 * self.duration) / 8.0) as u64
    }
}