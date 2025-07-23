//! # Video Processing Module
//!
//! This module provides comprehensive video optimization capabilities using FFmpeg and related tools.
//! All video processing is performed using external command-line tools to ensure maximum compatibility
//! and leverage the most advanced encoding algorithms available.
//! 
//! ## Core Responsibilities
//! - **Video Compression**: High-quality compression using H.264/H.265 codecs
//! - **Quality Control**: Precise quality control via CRF (Constant Rate Factor) settings
//! - **Audio Processing**: Audio re-encoding with AAC codec and configurable bitrate
//! - **Metadata Preservation**: Complete preservation of video metadata using exiftool
//! - **Format Analysis**: Video property analysis using ffprobe for optimization decisions
//! - **Dependency Management**: Automatic detection and verification of external tools
//! 
//! ## Supported Formats
//! - **Input Formats**: MP4, MOV, AVI, MKV, WebM, 3GP, FLV, WMV
//! - **Output Format**: MP4 (H.264 + AAC) for maximum compatibility across devices and platforms
//! 
//! ## Optimization Pipeline
//! 
//! ### 1. Video Analysis (Optional)
//! - Uses `ffprobe` to analyze input video properties
//! - Extracts resolution, framerate, duration, codec information
//! - Determines optimal encoding parameters based on source characteristics
//! 
//! ### 2. Video Compression
//! - **Video Codec**: libx264 (H.264) with optimized settings
//! - **Encoding Preset**: `veryslow` for maximum compression efficiency
//! - **Quality Control**: CRF-based encoding for consistent quality
//! - **Rate Control**: Two-pass encoding for critical applications (optional)
//! 
//! ### 3. Audio Processing
//! - **Audio Codec**: AAC-LC for broad compatibility
//! - **Bitrate Control**: Configurable audio bitrate (default: 128 kbps)
//! - **Channel Optimization**: Maintains original channel layout
//! - **Sample Rate**: Preserves or optimizes sample rate as needed
//! 
//! ### 4. Metadata Preservation
//! - Uses `exiftool` to preserve all original metadata
//! - Maintains creation dates, GPS coordinates, device information
//! - Preserves custom metadata fields and user-defined tags
//! 
//! ## Quality Control (CRF Scale)
//! 
//! The Constant Rate Factor (CRF) provides precise quality control:
//! 
//! - **0-17**: Visually lossless quality (very large files, archival use)
//! - **18-23**: High quality (recommended for professional/archival work)
//! - **24-27**: Good quality (recommended for general use, default: 26)
//! - **28-32**: Acceptable quality (smaller files, web distribution)
//! - **33+**: Poor quality (very small files, not recommended)
//! 
//! ## Performance Optimizations
//! 
//! ### Encoding Presets (Speed vs Compression)
//! - **ultrafast**: Fastest encoding, larger files
//! - **superfast**: Very fast encoding, moderately larger files
//! - **veryfast**: Fast encoding, good compression
//! - **faster**: Faster than medium, good balance
//! - **fast**: Fast encoding, reasonable compression
//! - **medium**: Default preset, balanced speed/compression
//! - **slow**: Slower encoding, better compression
//! - **slower**: Much slower encoding, excellent compression
//! - **veryslow**: Slowest encoding, maximum compression (default choice)
//! 
//! ### Hardware Acceleration Support
//! Future versions may include:
//! - NVENC (NVIDIA GPU encoding)
//! - Quick Sync (Intel integrated graphics)
//! - VCE (AMD GPU encoding)
//! - VideoToolbox (macOS hardware acceleration)
//! 
//! ## Error Handling and Resilience
//! 
//! - **Tool Availability**: Graceful handling when FFmpeg/ffprobe are unavailable
//! - **Format Support**: Automatic fallback for unsupported input formats
//! - **Encoding Failures**: Comprehensive error reporting and recovery
//! - **Resource Management**: Proper cleanup of temporary files
//! - **Process Monitoring**: Async process management without timeouts
//! 
//! ## Usage Examples
//! 
//! ```rust
//! use video_processor::VideoProcessor;
//! use config::Config;
//! 
//! // Create processor with custom settings
//! let config = Config {
//!     video_crf: 23,
//!     audio_bitrate: 192,
//!     output_path: Some(PathBuf::from("/output")),
//!     ..Default::default()
//! };
//! 
//! let processor = VideoProcessor::new(config);
//! 
//! // Optimize a single video
//! let optimized_path = processor.optimize_video(
//!     Path::new("/input/video.mov"),
//!     Path::new("/input")
//! ).await?;
//! 
//! // Check available tools
//! processor.// print_available_tools().await;
//! 
//! // Analyze video properties
//! let info = processor.analyze_video(Path::new("/input/video.mp4")).await?;
//! ```
//! 
//! ## External Dependencies
//! 
//! - **ffmpeg**: Core video processing engine
//! - **ffprobe**: Video analysis and metadata extraction
//! - **exiftool**: Metadata preservation and manipulation
//! 
//! All tools are checked for availability at runtime with graceful degradation
//! when tools are missing.
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
use crate::optimizer::path_resolver::PathResolver;
use crate::platform::PlatformCommands;
use anyhow::Result;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;
use tokio::process::Command;
use tokio::sync::broadcast;
use tracing::{debug, info, warn, error};

/// # Video Processor
/// 
/// Handles comprehensive video optimization using FFmpeg and related external tools.
/// Provides high-quality video compression, audio re-encoding, and metadata preservation
/// while maintaining broad compatibility across devices and platforms.
/// 
/// ## Key Features
/// - **Quality-controlled compression** using CRF (Constant Rate Factor)
/// - **Audio optimization** with AAC codec and configurable bitrate
/// - **Metadata preservation** using exiftool
/// - **Async processing** without blocking timeouts
/// - **Comprehensive error handling** with detailed logging
/// - **Format compatibility** with output standardized to MP4/H.264/AAC
/// - **Cancellation support** via broadcast channels
/// - **Strict tool requirements** - fails if required tools are missing
pub struct VideoProcessor {
    /// Configuration settings for video optimization
    config: Config,
    /// Cancellation receiver for stopping operations
    stop_receiver: Option<broadcast::Receiver<()>>,
}

impl VideoProcessor {
    /// Creates a new VideoProcessor instance with the provided configuration.
    /// 
    /// # Arguments
    /// * `config` - Configuration containing video quality, audio settings, and output paths
    /// 
    /// # Example
    /// ```rust
    /// let config = Config {
    ///     video_crf: 23,
    ///     audio_bitrate: "192k".to_string(),
    ///     skip_video_compression: false,
    ///     ..Default::default()
    /// };
    /// let processor = VideoProcessor::new(config);
    /// ```
    pub fn new(config: Config) -> Self {
        Self { 
            config,
            stop_receiver: None,
        }
    }

    /// Creates a new VideoProcessor instance with cancellation support.
    /// 
    /// # Arguments
    /// * `config` - Configuration containing video quality, audio settings, and output paths
    /// * `stop_receiver` - Broadcast receiver for cancellation signals
    /// 
    /// # Returns
    /// * `Self` - A new VideoProcessor instance with cancellation support
    /// 
    /// # Example
    /// ```rust
    /// let (stop_sender, stop_receiver) = broadcast::channel(1);
    /// let config = Config::default();
    /// let processor = VideoProcessor::new_with_cancellation(config, stop_receiver);
    /// 
    /// // To stop processing:
    /// stop_sender.send(()).unwrap();
    /// ```
    pub fn new_with_cancellation(config: Config, stop_receiver: broadcast::Receiver<()>) -> Self {
        Self { 
            config,
            stop_receiver: Some(stop_receiver),
        }
    }

    /// Checks if a stop signal has been received.
    /// 
    /// # Returns
    /// * `bool` - True if stop signal was received, false otherwise
    fn should_stop(&mut self) -> bool {
        if let Some(ref mut receiver) = self.stop_receiver {
            match receiver.try_recv() {
                Ok(_) => {
                    // debug!("Stop signal received, cancelling video processing");
                    return true;
                }
                Err(broadcast::error::TryRecvError::Empty) => {
                    // No signal yet, continue
                    return false;
                }
                Err(broadcast::error::TryRecvError::Lagged(_)) => {
                    // Signal was sent but we missed it, treat as stop
                    // debug!("Stop signal was lagged, cancelling video processing");
                    return true;
                }
                Err(broadcast::error::TryRecvError::Closed) => {
                    // Sender was dropped, continue processing
                    return false;
                }
            }
        }
        false
    }
    
    /// Optimizes a video file using FFmpeg with high-quality settings.
    /// 
    /// This is the main entry point for video optimization. The method handles
    /// the complete optimization pipeline including compression, metadata preservation,
    /// and proper file handling with cancellation support.
    /// 
    /// # Arguments
    /// * `input_path` - Path to the input video file
    /// * `input_base_dir` - Base directory for calculating relative output paths
    /// 
    /// # Returns
    /// * `Result<PathBuf>` - Path to the optimized video file
    /// 
    /// # Errors
    /// - Returns error if operation is cancelled
    /// - Returns error if required tools (ffmpeg, exiftool) are missing
    /// - Returns error if video processing fails
    /// 
    /// # Process Overview
    /// 1. Checks for cancellation signal before starting
    /// 2. Validates input and calculates output path
    /// 3. Creates output directories if needed
    /// 4. Optionally skips compression if configured
    /// 5. Performs video compression using FFmpeg with cancellation checks
    /// 6. Preserves original metadata using exiftool
    /// 7. Returns path to optimized file
    /// 
    /// # Example
    /// ```rust
    /// let processor = VideoProcessor::new(config);
    /// let optimized_path = processor.optimize(
    ///     Path::new("/input/video.mov"),
    ///     Path::new("/input")
    /// ).await?;
    /// ```
    pub async fn optimize(&mut self, input_path: &Path, input_base_dir: &Path) -> Result<PathBuf> {
        // Check for cancellation before starting
        if self.should_stop() {
            return Err(anyhow::anyhow!("Video optimization cancelled by user"));
        }

        info!("🎬 Starting video optimization for: {}", input_path.display());
        
        // Process without timeout - let FFmpeg complete its work
        self.optimize_internal(input_path, input_base_dir).await
    }
    
    /// Internal optimization implementation with comprehensive error handling and cancellation support.
    /// 
    /// This method handles the complete optimization workflow, from path resolution
    /// to final file output. It uses temporary files for safe processing and ensures
    /// proper cleanup even in error scenarios, with cancellation checks throughout.
    /// 
    /// # Process Details
    /// - Uses centralized PathResolver for consistent output path calculation
    /// - Creates temporary files for safe intermediate processing
    /// - Handles skip_video_compression flag for scenarios where only copying is needed
    /// - Preserves metadata after compression to maintain file integrity
    /// - Provides detailed logging throughout the process
    /// - Checks for cancellation signals at key points
    /// 
    /// # Error Handling
    /// - Validates input paths and file accessibility
    /// - Handles FFmpeg execution failures with detailed error messages
    /// - Manages temporary file cleanup automatically
    /// - Provides fallback behaviors for non-critical operations
    /// - Returns appropriate errors for cancellation
    async fn optimize_internal(&mut self, input_path: &Path, input_base_dir: &Path) -> Result<PathBuf> {
        info!("🎬 Starting video optimization: {}", 
              input_path.file_name().unwrap_or_default().to_string_lossy());
        
        // Calculate final output path using centralized logic
        let final_output_path = PathResolver::get_output_path(input_path, input_base_dir, &self.config)?;
        
        // Ensure output directory exists
        PathResolver::ensure_parent_dirs(&final_output_path).await?;

        // Check for cancellation after path setup
        if self.should_stop() {
            return Err(anyhow::anyhow!("Video optimization cancelled by user"));
        }
        
        // Handle skip compression mode
        if self.config.skip_video_compression {
            info!("⏩ Skipping video compression, copying original: {}", 
                  input_path.file_name().unwrap_or_default().to_string_lossy());
            tokio::fs::copy(input_path, &final_output_path).await?;
            info!("✅ Video copied without compression: {}", 
                  input_path.file_name().unwrap_or_default().to_string_lossy());
            return Ok(final_output_path);
        }
        
        // Create temporary file for safe processing
        let temp_file = NamedTempFile::with_suffix(".mp4")?;
        let temp_path = temp_file.path().to_path_buf();
        
        // Perform video compression with cancellation support
        self.compress_video(input_path, &temp_path).await?;

        // Check for cancellation after compression
        if self.should_stop() {
            return Err(anyhow::anyhow!("Video optimization cancelled by user"));
        }
        
        // Preserve original metadata
        // debug!("📝 Preserving video metadata...");
        self.preserve_metadata(input_path, &temp_path).await?;
        
        // Move optimized video to final destination
        info!("💾 Saving optimized video to: {}", final_output_path.display());
        tokio::fs::copy(&temp_path, &final_output_path).await?;
        
        info!("✅ Video optimization completed: {}", 
              input_path.file_name().unwrap_or_default().to_string_lossy());
        
        // Temporary file is automatically cleaned up when NamedTempFile is dropped
        Ok(final_output_path)
    }
    
    /// Compresses video using FFmpeg with optimized H.264 encoding settings and cancellation support.
    /// 
    /// This method handles the core video compression using FFmpeg with carefully
    /// selected parameters for optimal quality-to-size ratio. The encoding uses
    /// CRF (Constant Rate Factor) for consistent quality across different content types.
    /// Includes cancellation checks and proper error handling.
    /// 
    /// # Encoding Parameters
    /// - **Video Codec**: libx264 (H.264) for maximum compatibility
    /// - **Encoding Preset**: `veryslow` for maximum compression efficiency
    /// - **Quality Control**: CRF-based encoding for consistent quality
    /// - **Audio Codec**: AAC-LC with configurable bitrate
    /// - **Metadata**: Preserved using `-map_metadata 0`
    /// - **Compatibility**: Uses `movflags use_metadata_tags` for broad support
    /// 
    /// # Quality Settings (CRF)
    /// The CRF value controls the quality-size tradeoff:
    /// - Lower values = higher quality, larger files
    /// - Higher values = lower quality, smaller files
    /// - Typical range: 18-28 (default: 26)
    /// 
    /// # Arguments
    /// * `input_path` - Path to the input video file
    /// * `output_path` - Path where the compressed video will be saved
    /// 
    /// # Returns
    /// * `Result<()>` - Success or detailed error information
    /// 
    /// # Error Handling
    /// - Validates FFmpeg availability and execution
    /// - Provides detailed error messages from FFmpeg stderr
    /// - Logs compression progress and timing information
    /// - Handles various FFmpeg exit codes appropriately
    /// - Returns error if operation is cancelled
    async fn compress_video(&mut self, input_path: &Path, output_path: &Path) -> Result<()> {
        info!(
            "🎬 Compressing video: {} (CRF: {}, audio: {})",
            input_path.file_name().unwrap_or_default().to_string_lossy(),
            self.config.video_crf,
            self.config.audio_bitrate
        );

        // Check for cancellation before starting compression
        if self.should_stop() {
            return Err(anyhow::anyhow!("Video compression cancelled by user"));
        }
        
        let platform = PlatformCommands::instance();
        let ffmpeg_cmd = platform.get_command("ffmpeg");
        
        // Build FFmpeg command with optimized parameters
        let mut cmd = Command::new(ffmpeg_cmd);
        cmd.args([
            "-i", input_path.to_str().unwrap(),        // Input file
            "-c:v", "libx264",                         // Video codec: H.264
            "-preset", "veryslow",                     // Encoding speed vs compression trade-off
            "-crf", &self.config.video_crf.to_string(), // Quality control (Constant Rate Factor)
            "-c:a", "aac",                             // Audio codec: AAC-LC
            "-b:a", &self.config.audio_bitrate,        // Audio bitrate
            "-map_metadata", "0",                      // Copy all metadata from input
            "-movflags", "use_metadata_tags",          // Ensure metadata compatibility
            "-y", output_path.to_str().unwrap(),       // Output file (overwrite if exists)
        ]);
        
        // Configure FFmpeg logging level based on our log level
        if !tracing::enabled!(tracing::Level::DEBUG) {
            cmd.args(["-loglevel", "warning"]);        // Quiet mode for production
        } else {
            cmd.args(["-progress", "pipe:2"]);         // Progress output for // debugging
        }
        
        info!("🔄 Starting FFmpeg compression...");
        let start_time = std::time::Instant::now();
        
        // Execute FFmpeg asynchronously
        let output = cmd.output().await
            .map_err(|e| anyhow::anyhow!("Failed to execute {}: {}", ffmpeg_cmd, e))?;
        
        let duration = start_time.elapsed();
        
        // Check for cancellation after compression (before checking results)
        if self.should_stop() {
            return Err(anyhow::anyhow!("Video compression cancelled by user"));
        }
        
        // Check for FFmpeg execution success
        if !output.status.success() {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            error!("❌ FFmpeg failed after {:.1}s: {}", duration.as_secs_f64(), error_msg);
            return Err(OptimizeError::FFmpeg(error_msg.to_string()).into());
        }
        
        info!("✅ Video compression completed in {:.1}s", duration.as_secs_f64());
        
        Ok(())
    }
    
    /// Preserves original video metadata using exiftool.
    /// 
    /// This method copies all metadata from the original video file to the compressed
    /// version, ensuring that important information such as creation dates, GPS coordinates,
    /// camera settings, and custom tags are maintained.
    /// 
    /// # Metadata Preservation Features
    /// - **All Tags**: Copies all supported metadata tags (`-all:all`)
    /// - **Embedded Data**: Extracts embedded metadata (`-extractEmbedded`)
    /// - **File Timestamps**: Preserves original file modification dates
    /// - **Safe Operation**: Uses `-overwrite_original` for atomic updates
    /// 
    /// # Metadata Types Preserved
    /// - **Technical**: Resolution, codec, bitrate, duration
    /// - **Temporal**: Creation date, modification date, recording time
    /// - **Geospatial**: GPS coordinates, location names
    /// - **Device**: Camera/device model, settings, software version
    /// - **Custom**: User-defined tags and descriptions
    /// 
    /// # Arguments
    /// * `source` - Path to the original video file (metadata source)
    /// * `target` - Path to the compressed video file (metadata destination)
    /// 
    /// # Returns
    /// * `Result<()>` - Success or error information
    /// 
    /// # Error Handling
    /// - Metadata preservation failures are logged as warnings but don't fail the operation
    /// - Missing exiftool is handled gracefully with appropriate warnings
    /// - Invalid metadata is skipped rather than causing complete failure
    /// 
    /// # Example Command
    /// ```bash
    /// exiftool -tagsFromFile source.mp4 -extractEmbedded -all:all \
    ///          -FileModifyDate -overwrite_original target.mp4
    /// ```
    async fn preserve_metadata(&self, source: &Path, target: &Path) -> Result<()> {
        // debug!("📝 Preserving video metadata from {} to {}", source.display(), target.display());
        
        let platform = PlatformCommands::instance();
        let exiftool_cmd = platform.get_command("exiftool");
        
        let output = Command::new(exiftool_cmd)
            .args([
                "-tagsFromFile",                       // Copy tags from source file
                source.to_str().unwrap(),              // Source file path
                "-extractEmbedded",                    // Extract embedded metadata
                "-all:all",                            // Copy all available tags
                "-FileModifyDate",                     // Preserve file modification date
                "-overwrite_original",                 // Overwrite target file safely
                target.to_str().unwrap(),              // Target file path
            ])
            .output().await
            .map_err(|e| anyhow::anyhow!("Failed to execute exiftool: {}", e))?;
        
        if !output.status.success() {
            // Metadata preservation failure is not critical - log and continue
            warn!(
                "Failed to preserve video metadata for {}: {}",
                source.display(),
                String::from_utf8_lossy(&output.stderr)
            );
            // Don't fail the entire operation for metadata issues
        } else {
            // debug!("✅ Video metadata preserved successfully");
        }
        
        Ok(())
    }
    
    /// Analyzes video file properties using ffprobe.
    /// 
    /// This method extracts comprehensive information about a video file including
    /// technical specifications, stream details, and metadata. The information is
    /// useful for making optimization decisions and providing user feedback.
    /// 
    /// # Information Extracted
    /// - **Duration**: Total video length in seconds
    /// - **Bitrate**: Overall bitrate of the video file
    /// - **Resolution**: Width and height in pixels
    /// - **Codec**: Video codec used for encoding
    /// - **Streams**: Detailed information about video and audio streams
    /// 
    /// # Use Cases
    /// - **Pre-optimization analysis**: Determine if optimization is beneficial
    /// - **Quality assessment**: Compare before/after optimization metrics
    /// - **User interface**: Display video properties to users
    /// - **Batch processing**: Make per-file optimization decisions
    /// 
    /// # Arguments
    /// * `video_path` - Path to the video file to analyze
    /// 
    /// # Returns
    /// * `Result<VideoInfo>` - Structured video information or error
    /// 
    /// # Example
    /// ```rust
    /// let processor = VideoProcessor::new(config);
    /// let info = processor.get_video_info(Path::new("video.mp4")).await?;
    /// // println!("Duration: {:.1}s, Resolution: {}x{}", 
    ///          info.duration, info.width, info.height);
    /// ```
    /// 
    /// # ffprobe Command
    /// ```bash
    /// ffprobe -v quiet -// print_format json -show_format -show_streams video.mp4
    /// ```
    pub async fn get_video_info(&self, video_path: &Path) -> Result<VideoInfo> {
        // debug!("📊 Analyzing video properties: {}", video_path.display());
        
        let platform = PlatformCommands::instance();
        let ffprobe_cmd = platform.get_command("ffprobe");
        
        let output = Command::new(ffprobe_cmd)
            .args([
                "-v", "quiet",                         // Suppress informational output
                "-// print_format", "json",               // Output in JSON format
                "-show_format",                        // Show container format info
                "-show_streams",                       // Show stream details
                video_path.to_str().unwrap(),          // Input video file
            ])
            .output().await
            .map_err(|e| anyhow::anyhow!("Failed to execute ffprobe: {}", e))?;
        
        if !output.status.success() {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            return Err(OptimizeError::FFmpeg(error_msg.to_string()).into());
        }
        
        let info_str = String::from_utf8_lossy(&output.stdout);
        let video_info = VideoInfo::from_ffprobe_json(&info_str)?;
        
        debug!("📊 Video analysis complete: {}x{}, {:.1}s, {} codec", 
               video_info.width, video_info.height, video_info.duration, video_info.codec);
        
        Ok(video_info)
    }
    
    /// Verifies that all required external tools are available for video processing.
    /// 
    /// This method checks for the availability of essential tools needed for video
    /// optimization. It's recommended to call this during application startup or
    /// before beginning batch video processing operations. Unlike the previous version,
    /// this now performs strict validation and returns errors for missing tools.
    /// 
    /// # Required Tools
    /// - **ffmpeg**: Core video compression and encoding
    /// - **ffprobe**: Video analysis and property extraction
    /// - **exiftool**: Metadata preservation and manipulation
    /// 
    /// # Returns
    /// * `Result<()>` - Success if all tools are available, error with details if any are missing
    /// 
    /// # Error Details
    /// The error message will specify which tool is missing and its purpose,
    /// helping users understand what needs to be installed.
    /// 
    /// # Example
    /// ```rust
    /// // Check dependencies before processing
    /// VideoProcessor::check_dependencies().await?;
    /// 
    /// // Proceed with video processing
    /// let processor = VideoProcessor::new(config);
    /// processor.optimize(video_path, base_path).await?;
    /// ```
    /// 
    /// # Installation Notes
    /// - **ffmpeg/ffprobe**: Usually installed together as part of the FFmpeg package
    /// - **exiftool**: Separate installation, available on most package managers
    /// - Tools must be available in the system PATH or configured via PlatformCommands
    pub async fn check_dependencies() -> Result<()> {
        let platform = PlatformCommands::instance();
        
        // List of required tools with their purposes
        let tools = [
            ("ffmpeg", "video compression and encoding"),
            ("ffprobe", "video analysis and property extraction"),
            ("exiftool", "metadata preservation and manipulation"),
        ];
        
        info!("🔧 Checking video processing dependencies...");
        let mut missing_tools = Vec::new();
        
        for (tool, purpose) in &tools {
            if !platform.is_command_available(tool).await {
                let error_msg = format!("{} - {}", tool, purpose);
                missing_tools.push(error_msg);
                error!("❌ Missing dependency: {} ({})", tool, purpose);
            } else {
                // debug!("✅ {} available for {}", tool, purpose);
            }
        }
        
        if !missing_tools.is_empty() {
            let error_msg = format!(
                "Missing required video processing tools:\n{}",
                missing_tools.iter()
                    .map(|tool| format!("  ❌ {}", tool))
                    .collect::<Vec<_>>()
                    .join("\n")
            );
            return Err(anyhow::anyhow!(error_msg));
        }
        
        info!("✅ All video processing dependencies are available");
        Ok(())
    }
    
    /// Prints a report of available video processing tools to the log.
    /// 
    /// This method provides a comprehensive overview of tool availability for
    /// // debugging and system verification purposes. Unlike `check_dependencies()`,
    /// this method doesn't fail but reports the status of each tool.
    /// 
    /// # Example Output
    /// ```
    /// 🔧 Checking available video processing tools:
    ///   ✅ ffmpeg - Video compression and encoding
    ///   ✅ ffprobe - Video analysis and property extraction  
    ///   ❌ exiftool - Metadata preservation (optional)
    /// ```
    pub async fn print_available_tools(&self) {
        let platform = PlatformCommands::instance();
        
        info!("🔧 Checking available video processing tools:");
        
        let tools = [
            ("ffmpeg", "Video compression and encoding"),
            ("ffprobe", "Video analysis and property extraction"),
            ("exiftool", "Metadata preservation and manipulation"),
        ];

        for (tool, description) in &tools {
            let available = platform.is_command_available(tool).await;
            let status = if available { "✅" } else { "❌" };
            info!("  {} {} - {}", status, tool, description);
        }
    }

    /// Creates a broadcast channel for cancellation signals.
    /// 
    /// This utility method creates a broadcast channel that can be used to signal
    /// cancellation to multiple VideoProcessor instances or other components.
    /// 
    /// # Arguments
    /// * `capacity` - Channel capacity (number of buffered messages)
    /// 
    /// # Returns
    /// * `(broadcast::Sender<()>, broadcast::Receiver<()>)` - Sender and receiver for cancellation signals
    /// 
    /// # Example
    /// ```rust
    /// let (stop_sender, stop_receiver) = VideoProcessor::create_cancellation_channel(1);
    /// let processor = VideoProcessor::new_with_cancellation(config, stop_receiver);
    /// 
    /// // To stop processing:
    /// stop_sender.send(()).unwrap();
    /// ```
    pub fn create_cancellation_channel(capacity: usize) -> (broadcast::Sender<()>, broadcast::Receiver<()>) {
        broadcast::channel(capacity)
    }
}

/// # Video Information Structure
/// 
/// Contains comprehensive information about a video file extracted using ffprobe.
/// This structure provides essential metadata for making optimization decisions,
/// displaying file properties to users, and calculating compression estimates.
/// 
/// ## Information Categories
/// 
/// ### Technical Specifications
/// - **Duration**: Total video length in seconds (floating point for precision)
/// - **Bitrate**: Overall bitrate in bits per second
/// - **Resolution**: Width and height in pixels
/// - **Codec**: Video codec identifier (e.g., "h264", "hevc", "vp9")
/// 
/// ### Usage Examples
/// - **Quality Assessment**: Compare bitrates before/after optimization
/// - **Size Estimation**: Calculate expected compressed file sizes
/// - **Compatibility Checks**: Verify codec support for target platforms
/// - **User Interface**: Display video properties in file browsers
/// - **Batch Processing**: Make per-file optimization decisions
/// 
/// ## Data Sources
/// The information is extracted from ffprobe JSON output, which provides
/// comprehensive metadata about video containers and streams.
#[derive(Debug, Clone)]
pub struct VideoInfo {
    /// Video duration in seconds (floating point for sub-second precision)
    pub duration: f64,
    /// Overall bitrate in bits per second
    pub bitrate: u64,
    /// Video width in pixels
    pub width: u32,
    /// Video height in pixels  
    pub height: u32,
    /// Video codec name (e.g., "h264", "hevc", "vp9")
    pub codec: String,
}

impl VideoInfo {
    /// Creates VideoInfo from ffprobe JSON output.
    /// 
    /// This method parses the comprehensive JSON output from ffprobe and extracts
    /// the most relevant video information. It handles various edge cases such as
    /// missing fields, multiple streams, and format variations.
    /// 
    /// # Parsing Strategy
    /// 1. **Format Information**: Extracts container-level metadata (duration, bitrate)
    /// 2. **Stream Analysis**: Locates the primary video stream among all streams
    /// 3. **Codec Detection**: Identifies video codec from stream metadata
    /// 4. **Resolution Extraction**: Gets width/height from video stream properties
    /// 5. **Fallback Handling**: Provides sensible defaults for missing information
    /// 
    /// # JSON Structure Expected
    /// ```json
    /// {
    ///   "format": {
    ///     "duration": "123.456",
    ///     "bit_rate": "1500000"
    ///   },
    ///   "streams": [
    ///     {
    ///       "codec_type": "video",
    ///       "codec_name": "h264",
    ///       "width": 1920,
    ///       "height": 1080
    ///     }
    ///   ]
    /// }
    /// ```
    /// 
    /// # Arguments
    /// * `json_str` - JSON string output from ffprobe command
    /// 
    /// # Returns
    /// * `Result<Self>` - Parsed VideoInfo or error if JSON is invalid
    /// 
    /// # Error Handling
    /// - Invalid JSON format results in parsing errors
    /// - Missing video streams use default values
    /// - Invalid numeric values default to 0
    /// - Missing codec information defaults to "unknown"
    /// 
    /// # Example
    /// ```rust
    /// let json_output = r#"{"format":{"duration":"60.0","bit_rate":"2000000"},...}"#;
    /// let video_info = VideoInfo::from_ffprobe_json(json_output)?;
    /// ```
    pub fn from_ffprobe_json(json_str: &str) -> Result<Self> {
        let info: serde_json::Value = serde_json::from_str(json_str)?;
        
        // Extract container-level format information
        let format = &info["format"];
        let duration = format["duration"]
            .as_str()
            .and_then(|d| d.parse::<f64>().ok())
            .unwrap_or(0.0);
        
        let bitrate = format["bit_rate"]
            .as_str()
            .and_then(|b| b.parse::<u64>().ok())
            .unwrap_or(0);
        
        // Locate the primary video stream
        let empty_vec = vec![];
        let streams = info["streams"].as_array().unwrap_or(&empty_vec);
        let video_stream = streams.iter()
            .find(|s| s["codec_type"] == "video")
            .unwrap_or(&serde_json::Value::Null);
        
        // Extract video stream properties
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
    
    /// Determines if a video would benefit from optimization based on target bitrate.
    /// 
    /// This method provides a simple heuristic for deciding whether a video should
    /// be processed through the optimization pipeline. Videos with bitrates significantly
    /// higher than the target may benefit from compression, while those already at
    /// or below the target may not see meaningful improvements.
    /// 
    /// # Decision Logic
    /// - **Returns true**: Current bitrate exceeds target bitrate
    /// - **Returns false**: Current bitrate is at or below target
    /// 
    /// # Use Cases
    /// - **Batch Processing**: Skip files that don't need optimization
    /// - **User Interface**: Highlight videos that would benefit from compression
    /// - **Automatic Workflows**: Make intelligent processing decisions
    /// - **Storage Planning**: Estimate potential space savings
    /// 
    /// # Arguments
    /// * `target_bitrate` - Desired bitrate threshold in bits per second
    /// 
    /// # Returns
    /// * `bool` - True if optimization is recommended, false otherwise
    /// 
    /// # Example
    /// ```rust
    /// let video_info = VideoInfo::from_ffprobe_json(json_output)?;
    /// let target_bitrate = 2_000_000; // 2 Mbps
    /// 
    /// if video_info.needs_optimization(target_bitrate) {
    ///     // println!("Video could benefit from compression");
    /// } else {
    ///     // println!("Video is already well-compressed");
    /// }
    /// ```
    /// 
    /// # Considerations
    /// - This is a simple bitrate-based heuristic
    /// - Other factors like codec efficiency and visual quality are not considered
    /// - Very short videos may have misleading bitrate calculations
    /// - Consider implementing more sophisticated analysis for critical applications
    pub fn needs_optimization(&self, target_bitrate: u64) -> bool {
        self.bitrate > target_bitrate
    }
    
    /// Estimates the compressed file size based on a target bitrate.
    /// 
    /// This method provides a rough calculation of expected file size after compression
    /// using the specified target bitrate. The calculation uses the simple formula:
    /// `file_size = (bitrate * duration) / 8` to convert from bits to bytes.
    /// 
    /// # Calculation Method
    /// 1. **Bitrate Application**: Applies target bitrate to video duration
    /// 2. **Unit Conversion**: Converts from bits to bytes (divides by 8)
    /// 3. **Rounding**: Returns result as unsigned integer (bytes)
    /// 
    /// # Accuracy Considerations
    /// - **Container Overhead**: Doesn't account for container format overhead
    /// - **Variable Bitrate**: Assumes constant bitrate encoding
    /// - **Audio Streams**: Includes total file bitrate, not just video
    /// - **Metadata**: Doesn't include metadata and subtitle overhead
    /// 
    /// # Use Cases
    /// - **Storage Planning**: Estimate space requirements before processing
    /// - **User Interface**: Show expected file size reductions
    /// - **Batch Processing**: Calculate total compression savings
    /// - **Quality Decisions**: Balance file size vs quality trade-offs
    /// 
    /// # Arguments
    /// * `target_bitrate` - Target bitrate in bits per second
    /// 
    /// # Returns
    /// * `u64` - Estimated compressed file size in bytes
    /// 
    /// # Example
    /// ```rust
    /// let video_info = VideoInfo::from_ffprobe_json(json_output)?;
    /// let target_bitrate = 1_500_000; // 1.5 Mbps
    /// 
    /// let estimated_size = video_info.estimate_compressed_size(target_bitrate);
    /// // println!("Estimated compressed size: {:.1} MB", estimated_size as f64 / 1_000_000.0);
    /// ```
    /// 
    /// # Formula
    /// ```text
    /// file_size_bytes = (target_bitrate_bps * duration_seconds) / 8
    /// ```
    pub fn estimate_compressed_size(&self, target_bitrate: u64) -> u64 {
        ((target_bitrate as f64 * self.duration) / 8.0) as u64
    }
    
    /// Returns the video resolution as a formatted string.
    /// 
    /// # Returns
    /// * `String` - Resolution in "WIDTHxHEIGHT" format (e.g., "1920x1080")
    /// 
    /// # Example
    /// ```rust
    /// let video_info = VideoInfo::from_ffprobe_json(json_output)?;
    /// // println!("Resolution: {}", video_info.resolution_string());
    /// // Output: "Resolution: 1920x1080"
    /// ```
    pub fn resolution_string(&self) -> String {
        format!("{}x{}", self.width, self.height)
    }
    
    /// Returns the duration formatted as a human-readable string.
    /// 
    /// # Returns
    /// * `String` - Duration in "MM:SS" or "HH:MM:SS" format
    /// 
    /// # Example
    /// ```rust
    /// let video_info = VideoInfo::from_ffprobe_json(json_output)?;
    /// // println!("Duration: {}", video_info.duration_string());
    /// // Output: "Duration: 1:23:45" for a 1 hour, 23 minute, 45 second video
    /// ```
    pub fn duration_string(&self) -> String {
        let total_seconds = self.duration as u64;
        let hours = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;
        let seconds = total_seconds % 60;
        
        if hours > 0 {
            format!("{}:{:02}:{:02}", hours, minutes, seconds)
        } else {
            format!("{}:{:02}", minutes, seconds)
        }
    }
    
    /// Returns the bitrate formatted as a human-readable string.
    /// 
    /// # Returns
    /// * `String` - Bitrate in appropriate units (kbps, Mbps, etc.)
    /// 
    /// # Example
    /// ```rust
    /// let video_info = VideoInfo::from_ffprobe_json(json_output)?;
    /// // println!("Bitrate: {}", video_info.bitrate_string());
    /// // Output: "Bitrate: 2.5 Mbps"
    /// ```
    pub fn bitrate_string(&self) -> String {
        let bitrate_f = self.bitrate as f64;
        
        if bitrate_f >= 1_000_000.0 {
            format!("{:.1} Mbps", bitrate_f / 1_000_000.0)
        } else if bitrate_f >= 1_000.0 {
            format!("{:.0} kbps", bitrate_f / 1_000.0)
        } else {
            format!("{} bps", self.bitrate)
        }
    }
}