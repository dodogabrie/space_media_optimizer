//! # Image Processing Module
//!
//! Questo modulo gestisce l'ottimizzazione di tutti i formati immagine supportati
//! utilizzando esclusivamente tool esterni per massimizzare la compatibilità e 
//! le performance.
//! 
//! ## Architettura Zero-Dependency
//! 
//! A differenza di altri processori di immagini che usano librerie Rust come `image`,
//! questo modulo orchestrate solo tool esterni specializzati per ogni formato:
//! 
//! - **Vantaggi**:
//!   - Nessuna dipendenza pesante da librerie di imaging Rust
//!   - Utilizza tool altamente ottimizzati (mozjpeg, oxipng, cwebp)
//!   - Fallback automatico quando tool non disponibili
//!   - Performance native degli strumenti specializzati
//! 
//! - **Svantaggi**:
//!   - Richiede tool esterni installati nel sistema
//!   - Overhead di process spawning per ogni immagine
//! 
//! ## Formati Supportati
//! 
//! | Formato | Input | Output | Tool Utilizzati |
//! |---------|-------|--------|-----------------|
//! | JPEG    | ✅    | ✅     | mozjpeg, jpegoptim, jpegtran |
//! | PNG     | ✅    | ✅     | oxipng, optipng, pngcrush |
//! | WebP    | ✅    | ✅     | cwebp (conversion + optimization) |
//! | Altri   | ✅    | ❌     | Errore (no optimization) |
//! 
//! ## Pipeline di Ottimizzazione
//! 
//! 1. **Rilevamento formato**: Analizza estensione file (case-insensitive)
//! 2. **Decisione conversione**: WebP se `convert_to_webp = true`
//! 3. **Calcolo path output**: Preserva struttura directory o in-place
//! 4. **Creazione directory**: Async creation delle directory parent
//! 5. **Tool selection**: Priorità decrescente per ogni formato
//! 6. **Strict error handling**: Errore se nessun tool disponibile
//! 
//! ## Strategia Tool Selection
//! 
//! ### JPEG (Priorità decrescente):
//! 1. **mozjpeg**: Migliore compressione, controllo qualità preciso
//! 2. **jpegoptim**: Buona compressione, controllo qualità, output su stdout
//! 3. **jpegtran**: Solo ottimizzazione lossless, nessun controllo qualità
//! 4. **Fallback**: Errore se nessun tool disponibile
//! 
//! ### PNG (Priorità decrescente):
//! 1. **oxipng**: Veloce, ottima compressione, strip metadata sicuro
//! 2. **optipng**: Compressione aggressiva, strip completo
//! 3. **pngcrush**: Brute force optimization, più lento ma efficace
//! 4. **Fallback**: Errore se nessun tool disponibile
//! 
//! ### WebP:
//! 1. **cwebp**: Unico tool per conversione e ottimizzazione
//! 2. **Fallback**: Errore se cwebp non disponibile
//! 
//! ## Configurazione Qualità
//! 
//! - **JPEG Quality**: 1-100 (default: 80)
//!   - Utilizzato da mozjpeg (`-quality`) e jpegoptim (`--max=`)
//!   - jpegtran ignora (solo lossless)
//! 
//! - **WebP Quality**: 1-100 (default: 80)
//!   - Utilizzato da cwebp (`-q`) per conversione e ottimizzazione
//!   - Parametri speed: `-m 4` (bilanciato), `-mt` (multithreading)
//! 
//! ## Gestione Path Output
//! 
//! ### Modalità Output Directory (`config.output_path = Some(dir)`):
//! ```text
//! Input:  /src/photos/2023/vacation/IMG_001.jpg
//! Base:   /src/photos
//! Output: /dest/photos/2023/vacation/IMG_001.jpg (o .webp)
//! ```
//! 
//! ### Modalità In-Place (`config.output_path = None`):
//! ```text
//! Input:  /photos/IMG_001.jpg
//! Output: /photos/IMG_001.jpg (stesso file, ottimizzato)
//! ```
//! 
//! ## Error Handling e Resilienza
//! 
//! - **Tool non disponibili**: Errore immediato (no silent copying)
//! - **Tool falliscono**: Prova tool successivo nella catena
//! - **Path invalidi**: Errore immediato con contesto
//! - **Directory creation**: Fallimento critico (propagato)
//! - **Cancellazione**: Controlli di stop signal durante l'elaborazione
//! 
//! ## Concorrenza e Performance
//! 
//! - **Async/await**: Tutte le operazioni I/O sono non-bloccanti
//! - **tokio::process::Command**: Process spawning asincrono
//! - **tokio::fs**: File operations asincrone
//! - **Platform abstraction**: Command detection cached
//! 
//! ## Esempi d'Uso
//! 
//! ```rust
//! use image_processor::ImageProcessor;
//! 
//! // Creazione processore con configurazione
//! let config = Config {
//!     jpeg_quality: 85,
//!     webp_quality: 80,
//!     convert_to_webp: true,
//!     output_path: Some(PathBuf::from("/output")),
//!     ..Default::default()
//! };
//! 
//! let processor = ImageProcessor::new(config).await?;
//! 
//! // Ottimizzazione singola immagine
//! let optimized = processor.optimize(&input_path, &base_dir).await?;
//! 
//! // Check strumenti disponibili
//! processor.// print_available_tools().await;
//! ```

use crate::config::Config;
use crate::platform::PlatformCommands;
use crate::utils::to_string_vec;
use anyhow::Result;
use std::path::{Path, PathBuf};
use tokio::process::Command;
use tokio::sync::broadcast;
use tracing::{debug, info, warn, error};

/// # Image Processor Module
/// 
/// This module provides image optimization capabilities using only external command-line tools.
/// No in-memory image processing is performed - all operations are delegated to specialized
/// external tools for maximum efficiency and quality.
/// 
/// ## Supported Formats
/// - **JPEG**: Optimized using mozjpeg, jpegoptim, or jpegtran (in order of preference)
/// - **PNG**: Optimized using oxipng, optipng, or pngcrush (in order of preference)  
/// - **WebP**: Optimized using cwebp or converted from other formats
/// 
/// ## Optimization Strategy
/// The processor attempts to use the best available tool for each format:
/// 1. **Quality-aware tools** (mozjpeg, jpegoptim, cwebp) - allow quality parameter adjustment
/// 2. **Lossless optimization tools** (jpegtran, oxipng, optipng, pngcrush) - reduce file size without quality loss
/// 3. **Error on missing tools**: Returns error if no optimization tools are available (no silent copying)
/// 
/// ## Features
/// - Async processing using tokio
/// - Automatic tool availability detection
/// - Configurable quality settings for JPEG and WebP
/// - Optional WebP conversion for all image formats
/// - Preserves directory structure in output
/// - Comprehensive error handling and logging
/// - **Cancellation support**: Can be stopped via broadcast channel
/// - **Strict tool requirement**: Fails if no optimization tools are available
pub struct ImageProcessor {
    /// Configuration settings for optimization (quality, output paths, etc.)
    config: Config,
    /// Cancellation receiver for stopping operations
    stop_receiver: Option<broadcast::Receiver<()>>,
}

impl ImageProcessor {
    /// Creates a new ImageProcessor instance with the provided configuration.
    /// 
    /// # Arguments
    /// * `config` - Configuration containing quality settings, paths, and optimization options
    /// 
    /// # Returns
    /// * `Result<Self>` - A new ImageProcessor instance
    /// 
    /// # Example
    /// ```rust
    /// let config = Config::default();
    /// let processor = ImageProcessor::new(config).await?;
    /// ```
    pub async fn new(config: Config) -> Result<Self> {
        Ok(Self { 
            config,
            stop_receiver: None,
        })
    }

    /// Creates a new ImageProcessor instance with cancellation support.
    /// 
    /// # Arguments
    /// * `config` - Configuration containing quality settings, paths, and optimization options
    /// * `stop_receiver` - Broadcast receiver for cancellation signals
    /// 
    /// # Returns
    /// * `Result<Self>` - A new ImageProcessor instance with cancellation support
    /// 
    /// # Example
    /// ```rust
    /// let (stop_sender, stop_receiver) = broadcast::channel(1);
    /// let config = Config::default();
    /// let processor = ImageProcessor::new_with_cancellation(config, stop_receiver).await?;
    /// 
    /// // To stop processing:
    /// stop_sender.send(()).unwrap();
    /// ```
    pub async fn new_with_cancellation(config: Config, stop_receiver: broadcast::Receiver<()>) -> Result<Self> {
        Ok(Self { 
            config,
            stop_receiver: Some(stop_receiver),
        })
    }

    /// Checks if a stop signal has been received.
    /// 
    /// # Returns
    /// * `bool` - True if stop signal was received, false otherwise
    fn should_stop(&mut self) -> bool {
        if let Some(ref mut receiver) = self.stop_receiver {
            match receiver.try_recv() {
                Ok(_) => {
                    // debug!("Stop signal received, cancelling image processing");
                    return true;
                }
                Err(broadcast::error::TryRecvError::Empty) => {
                    // No signal yet, continue
                    return false;
                }
                Err(broadcast::error::TryRecvError::Lagged(_)) => {
                    // Signal was sent but we missed it, treat as stop
                    // debug!("Stop signal was lagged, cancelling image processing");
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

    /// Optimizes a single image file using the best available external tool.
    /// 
    /// This method performs the following steps:
    /// 1. Checks for cancellation signal before starting
    /// 2. Determines the optimal output path based on the input file and configuration
    /// 3. Creates necessary output directories asynchronously
    /// 4. Detects the image format from file extension (case-insensitive)
    /// 5. Selects the best optimization strategy based on format and configuration
    /// 6. Executes the optimization command asynchronously with cancellation support
    /// 7. Optionally converts to WebP if requested and the original format is not already WebP
    /// 
    /// # Arguments
    /// * `input_path` - Path to the input image file
    /// * `input_base_dir` - Base directory for calculating relative paths in output
    /// 
    /// # Returns
    /// * `Result<PathBuf>` - Path to the optimized output file
    /// 
    /// # Errors
    /// Returns an error if:
    /// - Operation was cancelled via stop signal
    /// - Input file path contains invalid characters
    /// - Output directory cannot be created
    /// - **No optimization tools are available for the format** (no silent copying)
    /// - All optimization tools fail for the format
    /// - File operations fail
    /// 
    /// # Supported Formats
    /// - **JPEG/JPG**: Requires mozjpeg, jpegoptim, or jpegtran
    /// - **PNG**: Requires oxipng, optipng, or pngcrush  
    /// - **WebP**: Requires cwebp
    /// - **Other**: Returns error (no optimization possible)
    /// 
    /// # Example
    /// ```rust
    /// let processor = ImageProcessor::new(config).await?;
    /// let output_path = processor.optimize(
    ///     Path::new("/input/photos/image.jpg"), 
    ///     Path::new("/input")
    /// ).await?;
    /// ```
    pub async fn optimize(&mut self, input_path: &Path, input_base_dir: &Path) -> Result<PathBuf> {
        // Check for cancellation before starting
        if self.should_stop() {
            return Err(anyhow::anyhow!("Image optimization cancelled by user"));
        }

        // Pre-resize large images to 2.5K if needed
        let actual_input_path = if self.is_larger_than_4k(input_path).await.unwrap_or(false) {
            let temp_resized_path = self.create_temp_resized_path(input_path)?;
            self.pre_resize_to_4k(input_path, &temp_resized_path).await?;
            info!("Pre-resized large image {} to 2.5K at {}", 
                  input_path.display(), temp_resized_path.display());
            temp_resized_path
        } else {
            input_path.to_path_buf()
        };

        // Convert input path to string for tool commands
        let input_str = actual_input_path.to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid input path: {:?}", actual_input_path))?;

        // Calculate the output path based on configuration and input structure
        let output_path = self.get_output_path(input_path, input_base_dir);
        let output_str = output_path.to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid output path: {:?}", output_path))?;

        // Ensure output directory exists (create parent directories if needed)
        if let Some(parent) = output_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        // Check for cancellation after directory creation
        if self.should_stop() {
            return Err(anyhow::anyhow!("Image optimization cancelled by user"));
        }

        // Extract and normalize file extension for format detection
        let ext = actual_input_path.extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_lowercase());

        // Route to appropriate optimization method based on format
        let result = match ext.as_deref() {
            Some("jpg") | Some("jpeg") => {
                if self.config.convert_to_webp {
                    self.convert_to_webp(input_str, output_str).await
                } else {
                    self.optimize_jpeg(input_str, output_str).await
                }
            }
            Some("png") => {
                if self.config.convert_to_webp {
                    self.convert_to_webp(input_str, output_str).await
                } else {
                    self.optimize_png(input_str, output_str).await
                }
            }
            Some("webp") => {
                self.optimize_webp(input_str, output_str).await
            }
            _ => {
                // Unsupported format - return error instead of copying
                error!("Unsupported format for optimization: {:?}", actual_input_path);
                Err(anyhow::anyhow!("Unsupported image format: {:?}. Only JPEG, PNG, and WebP are supported.", actual_input_path))
            }
        };

        // Clean up temporary pre-resized file if we created one
        if actual_input_path != input_path {
            if let Err(e) = tokio::fs::remove_file(&actual_input_path).await {
                warn!("Failed to cleanup temporary resized file {}: {}", actual_input_path.display(), e);
            } else {
                debug!("Cleaned up temporary pre-resized file: {}", actual_input_path.display());
            }
        }

        result
    }

    /// Optimizes JPEG images using the best available tool in order of preference.
    /// 
    /// **Tool Priority (best to worst):**
    /// 1. **mozjpeg**: Best compression ratio, precise quality control, progressive encoding
    /// 2. **jpegoptim**: Good compression, quality control, outputs to stdout for safe handling
    /// 3. **jpegtran**: Lossless optimization only, no quality control but very safe
    /// 
    /// **Quality Settings:**
    /// - Uses `config.jpeg_quality` (1-100) for mozjpeg and jpegoptim
    /// - jpegtran performs lossless optimization regardless of quality setting
    /// - **Returns error if no tools are available** (no silent copying)
    async fn optimize_jpeg(&mut self, input: &str, output: &str) -> Result<PathBuf> {
        // debug!("Starting JPEG optimization: {} -> {}", input, output);

        // Special handling for jpegoptim which outputs to stdout
        let platform = PlatformCommands::instance();
        if platform.is_command_available("jpegoptim").await {
            if self.should_stop() {
                return Err(anyhow::anyhow!("JPEG optimization cancelled by user"));
            }

            // debug!("Attempting JPEG optimization with jpegoptim (quality: {})", self.config.jpeg_quality);
            let args = to_string_vec([
                &format!("--max={}", self.config.jpeg_quality),
                "--stdout",
                input,
            ]);

            if self.run_tool_with_stdout_output("jpegoptim", &args, output).await? {
                // debug!("JPEG optimized successfully with jpegoptim");
                return Ok(PathBuf::from(output));
            }
        }

        // Define tools with their argument builders (excluding jpegoptim handled above)
        let tools: &[(&str, fn(&str, &str, &Config) -> Vec<String>)] = &[
            ("mozjpeg", |input, output, config| to_string_vec([
                "-quality", &config.jpeg_quality.to_string(),
                "-optimize",
                "-progressive",
                "-outfile", output,
                input,
            ])),
            ("jpegtran", |input, output, _config| to_string_vec([
                "-optimize",
                "-progressive",
                "-outfile", output,
                input,
            ])),
        ];

        self.try_optimization_tools(input, output, tools, "JPEG").await
    }

    /// Optimizes PNG images using the best available tool in order of preference.
    /// 
    /// **Tool Priority (best to worst):**
    /// 1. **oxipng**: Fast, excellent compression, safe metadata stripping
    /// 2. **optipng**: Aggressive compression with multiple trials, complete metadata stripping
    /// 3. **pngcrush**: Brute force optimization, slower but very effective
    /// 
    /// **Optimization Features:**
    /// - All tools perform lossless compression (no quality degradation)
    /// - Metadata stripping for smaller file sizes and privacy
    /// - Progressive optimization levels for best compression ratios
    /// - **Returns error if no tools are available** (no silent copying)
    async fn optimize_png(&mut self, input: &str, output: &str) -> Result<PathBuf> {
        // debug!("Starting PNG optimization: {} -> {}", input, output);

        // Define tools with their argument builders
        let tools: &[(&str, fn(&str, &str, &Config) -> Vec<String>)] = &[
            ("oxipng", |input, output, _config| to_string_vec([
                "-o", "6",
                "--strip", "all",
                "--out", output,
                input,
            ])),
            ("optipng", |input, output, _config| to_string_vec([
                "-o7",
                "-strip", "all",
                "-out", output,
                input,
            ])),
            ("pngcrush", |input, output, _config| to_string_vec([
                "-rem", "alla",
                "-brute",
                input,
                output,
            ])),
        ];

        self.try_optimization_tools(input, output, tools, "PNG").await
    }

    /// Optimizes existing WebP images using cwebp.
    /// 
    /// WebP optimization re-encodes the image with the configured quality setting
    /// to potentially achieve better compression than the original encoding.
    /// 
    /// **Optimization Features:**
    /// - Quality-controlled re-encoding using `config.webp_quality`
    /// - Multi-threading support for faster processing (`-mt`)
    /// - Balanced encoding method (`-m 4`) for good speed/quality ratio
    /// - **Returns error if cwebp is not available** (no silent copying)
    async fn optimize_webp(&mut self, input: &str, output: &str) -> Result<PathBuf> {
        // debug!("Starting WebP optimization: {} -> {} (quality: {})", input, output, self.config.webp_quality);

        let tools: &[(&str, fn(&str, &str, &Config) -> Vec<String>)] = &[
            ("cwebp", |input, output, config| to_string_vec([
                "-q", &config.webp_quality.to_string(),
                "-m", "4",
                "-mt",
                input,
                "-o", output,
            ])),
        ];

        self.try_optimization_tools(input, output, tools, "WebP").await
    }

    /// Converts any supported image format to WebP using cwebp.
    /// 
    /// This method converts JPEG, PNG, and other formats to WebP format with
    /// the configured quality setting. The conversion can significantly reduce
    /// file sizes while maintaining good visual quality.
    /// 
    /// **Conversion Features:**
    /// - Supports all formats that cwebp can read (JPEG, PNG, TIFF, etc.)
    /// - Quality-controlled encoding using `config.webp_quality`
    /// - Multi-threading support for faster processing
    /// - Optimized encoding method for best compression
    async fn convert_to_webp(&mut self, input: &str, output: &str) -> Result<PathBuf> {
        // debug!("Converting to WebP: {} -> {} (quality: {})", input, output, self.config.webp_quality);

        let tools: &[(&str, fn(&str, &str, &Config) -> Vec<String>)] = &[
            ("cwebp", |input, output, config| to_string_vec([
                "-q", &config.webp_quality.to_string(),
                "-m", "4",
                "-mt",
                input,
                "-o", output,
            ])),
        ];

        self.try_optimization_tools(input, output, tools, "WebP conversion").await
    }

    /// Calculates the output path for an optimized image based on configuration.
    /// 
    /// This method handles two main scenarios:
    /// 
    /// **Output Directory Mode** (`config.output_path = Some(dir)`):
    /// - Preserves the relative directory structure from input_base_dir
    /// - Places files in the specified output directory
    /// - Example: `/src/photos/2023/img.jpg` → `/dest/photos/2023/img.jpg`
    /// 
    /// **In-Place Mode** (`config.output_path = None`):
    /// - Replaces the original file in the same location
    /// - Useful for batch optimization without moving files
    /// - Example: `/photos/img.jpg` → `/photos/img.jpg`
    /// 
    /// **File Extension Handling:**
    /// - Preserves original extension unless `convert_to_webp = true`
    /// - WebP conversion changes extension to `.webp`
    /// - Maintains the base filename (stem) in all cases
    /// 
    /// # Arguments
    /// * `input_path` - Path to the input image file
    /// * `input_base_dir` - Base directory for calculating relative paths
    /// 
    /// # Returns
    /// * `PathBuf` - Calculated output path for the optimized image
    /// 
    /// # Examples
    /// ```rust
    /// // Output directory mode with WebP conversion
    /// let input = Path::new("/src/photos/vacation/IMG_001.jpg");
    /// let base = Path::new("/src/photos");
    /// let output = processor.get_output_path(input, base);
    /// // Result: /dest/photos/vacation/IMG_001.webp
    /// 
    /// // In-place mode without conversion
    /// let input = Path::new("/photos/IMG_001.jpg");
    /// let base = Path::new("/photos");
    /// let output = processor.get_output_path(input, base);
    /// // Result: /photos/IMG_001.jpg
    /// ```
    fn get_output_path(&self, input_path: &Path, input_base_dir: &Path) -> PathBuf {
        // Extract the base filename without extension
        let stem = input_path.file_stem().unwrap_or_default();
        
        // Determine the output extension based on conversion settings
        let extension = if self.config.convert_to_webp {
            "webp"  // Force WebP extension if conversion is enabled
        } else {
            // Preserve original extension, defaulting to "jpg" if none found
            input_path.extension()
                .unwrap_or_default()
                .to_str()
                .unwrap_or("jpg")
        };

        // Construct the new filename with appropriate extension
        let filename = format!("{}.{}", stem.to_string_lossy(), extension);

        if let Some(ref output_dir) = self.config.output_path {
            // Output directory mode: preserve relative directory structure
            let relative_path = input_path.strip_prefix(input_base_dir)
                .unwrap_or(input_path)  // Use full path if strip_prefix fails
                .parent()               // Get the directory part
                .unwrap_or(Path::new("")); // Default to empty path if no parent
            
            output_dir.join(relative_path).join(filename)
        } else {
            // In-place mode: replace file in the same directory
            input_path.with_file_name(filename)
        }
    }

    /// Prints a report of available optimization tools to the log.
    /// 
    /// This method checks for the availability of all external tools used by the
    /// image processor and logs their status. Useful for // debugging and system
    /// setup verification.
    /// 
    /// **Checked Tools:**
    /// - **JPEG**: mozjpeg, jpegoptim, jpegtran
    /// - **PNG**: oxipng, optipng, pngcrush
    /// - **WebP**: cwebp
    /// - **Metadata**: exiftool (for future use)
    /// 
    /// **Output Format:**
    /// ```
    /// 🔧 Checking available optimization tools:
    ///   ✅ mozjpeg - JPEG optimization
    ///   ❌ jpegoptim - JPEG optimization
    ///   ✅ oxipng - PNG optimization
    ///   ...
    /// ```
    /// 
    /// # Example
    /// ```rust
    /// let processor = ImageProcessor::new(config).await?;
    /// processor.// print_available_tools().await;
    /// ```
    pub async fn print_available_tools(&self) {
        let platform = PlatformCommands::instance();
        
        info!("🔧 Checking available optimization tools:");
        
        // List of tools to check with their descriptions
        let tools = [
            ("mozjpeg", "JPEG optimization (best quality)"),
            ("jpegoptim", "JPEG optimization (good alternative)"),
            ("jpegtran", "JPEG optimization (lossless only)"),
            ("oxipng", "PNG optimization (fast and effective)"),
            ("optipng", "PNG optimization (aggressive)"),
            ("pngcrush", "PNG optimization (brute force)"),
            ("cwebp", "WebP conversion and optimization"),
            ("exiftool", "Metadata preservation (future use)"),
        ];

        // Check each tool and log its availability
        for (tool, description) in &tools {
            let available = platform.is_command_available(tool).await;
            let status = if available { "✅" } else { "❌" };
            info!("  {} {} - {}", status, tool, description);
        }
    }

    /// Checks for the availability of optimization tool dependencies.
    /// 
    /// This method provides a comprehensive check to verify that at least some 
    /// optimization tools are available. It's more permissive than before - 
    /// it only requires that at least one optimization tool is available,
    /// rather than requiring tools for all formats.
    /// 
    /// # Validation Strategy
    /// - Checks for availability of any optimization tools
    /// - Warns about missing tool categories but doesn't fail
    /// - Only fails if no optimization tools are available at all
    /// 
    /// # Returns
    /// * `Result<()>` - Success if at least some tools are available, error only if no tools found
    /// 
    /// # Example
    /// ```rust
    /// // Check dependencies before processing
    /// ImageProcessor::check_dependencies().await?;
    /// 
    /// // Proceed with image processing
    /// let processor = ImageProcessor::new(config).await?;
    /// ```
    /// 
    /// # Tool Requirements
    /// - At least one optimization tool must be available
    /// - Missing tools for specific formats will cause errors only when those formats are encountered
    pub async fn check_dependencies() -> Result<()> {
        let platform = PlatformCommands::instance();
        let mut available_tools = Vec::new();
        let mut missing_categories = Vec::new();
        
        info!("🔧 Checking image optimization tool dependencies...");
        
        // Check JPEG tools
        let jpeg_tools = ["mozjpeg", "jpegoptim", "jpegtran"];
        let has_jpeg_tool = jpeg_tools.iter()
            .any(|tool| futures::executor::block_on(platform.is_command_available(tool)));
        
        if has_jpeg_tool {
            available_tools.push("JPEG optimization");
        } else {
            missing_categories.push("JPEG optimization (install one of: mozjpeg, jpegoptim, jpegtran)");
        }
        
        // Check PNG tools  
        let png_tools = ["oxipng", "optipng", "pngcrush"];
        let has_png_tool = png_tools.iter()
            .any(|tool| futures::executor::block_on(platform.is_command_available(tool)));
        
        if has_png_tool {
            available_tools.push("PNG optimization");
        } else {
            missing_categories.push("PNG optimization (install one of: oxipng, optipng, pngcrush)");
        }
        
        // Check WebP tools
        let has_webp_tool = platform.is_command_available("cwebp").await;
        if has_webp_tool {
            available_tools.push("WebP conversion/optimization");
        } else {
            missing_categories.push("WebP conversion/optimization (install: libwebp/webp package with cwebp)");
        }
        
        // Report available tools
        if !available_tools.is_empty() {
            info!("✅ Available optimization tools: {}", available_tools.join(", "));
        }
        
        // Warn about missing tools but don't fail
        if !missing_categories.is_empty() {
            warn!("⚠️ Missing optimization tools (will cause errors if these formats are encountered):");
            for category in &missing_categories {
                warn!("  ❌ {}", category);
            }
        }
        
        // Only fail if no tools are available at all
        if available_tools.is_empty() {
            let error_msg = "No image optimization tools available! Please install at least one of: mozjpeg, jpegoptim, jpegtran, oxipng, optipng, pngcrush, or cwebp";
            error!("{}", error_msg);
            return Err(anyhow::anyhow!(error_msg));
        }
        
        info!("🎯 Tool dependency check passed - {} categories available", available_tools.len());
        Ok(())
    }
    
    /// Checks if WebP conversion/optimization is supported on this system.
    /// 
    /// WebP support requires the `cwebp` tool to be available. This method
    /// provides a quick way to verify WebP capabilities before processing.
    /// 
    /// # Returns
    /// * `bool` - `true` if cwebp is available, `false` otherwise
    /// 
    /// # Example
    /// ```rust
    /// if ImageProcessor::check_webp_support().await {
    ///     // println!("WebP conversion is supported");
    /// } else {
    ///     // println!("WebP conversion requires cwebp tool");
    /// }
    /// ```
    pub async fn check_webp_support() -> bool {
        let platform = PlatformCommands::instance();
        platform.is_command_available("cwebp").await
    }

    /// Creates a broadcast channel for cancellation signals.
    /// 
    /// This utility method creates a broadcast channel that can be used to signal
    /// cancellation to multiple ImageProcessor instances or other components.
    /// 
    /// # Arguments
    /// * `capacity` - Channel capacity (number of buffered messages)
    /// 
    /// # Returns
    /// * `(broadcast::Sender<()>, broadcast::Receiver<()>)` - Sender and receiver for cancellation signals
    /// 
    /// # Example
    /// ```rust
    /// let (stop_sender, stop_receiver) = ImageProcessor::create_cancellation_channel(1);
    /// let processor = ImageProcessor::new_with_cancellation(config, stop_receiver).await?;
    /// 
    /// // To stop processing:
    /// stop_sender.send(()).unwrap();
    /// ```
    pub fn create_cancellation_channel(capacity: usize) -> (broadcast::Sender<()>, broadcast::Receiver<()>) {
        broadcast::channel(capacity)
    }

    /// Generic helper for trying multiple optimization tools in order of preference.
    /// 
    /// This helper reduces code duplication by providing a common pattern for:
    /// 1. Checking tool availability
    /// 2. Checking for cancellation
    /// 3. Running the tool with appropriate arguments
    /// 4. Handling success/failure
    /// 5. Trying the next tool on failure
    /// 
    /// # Arguments
    /// * `input` - Input file path
    /// * `output` - Output file path  
    /// * `tools` - List of (tool_name, args_builder_fn) tuples in order of preference
    /// * `format_name` - Format name for error messages (e.g., "JPEG", "PNG")
    /// 
    /// # Returns
    /// * `Result<PathBuf>` - Success with output path, or error if all tools fail
    async fn try_optimization_tools<F>(
        &mut self,
        input: &str,
        output: &str,
        tools: &[(&str, F)],
        format_name: &str,
    ) -> Result<PathBuf>
    where
        F: Fn(&str, &str, &Config) -> Vec<String>,
    {
        let platform = PlatformCommands::instance();
        let mut any_tool_available = false;

        for (tool_name, args_builder) in tools {
            if platform.is_command_available(tool_name).await {
                any_tool_available = true;

                // Check for cancellation before each tool attempt
                if self.should_stop() {
                    return Err(anyhow::anyhow!("{} optimization cancelled by user", format_name));
                }

                debug!("Attempting {} optimization with {}", format_name, tool_name);
                
                // Get the resolved tool path (bundled or system)
                let tool_path = platform.get_tool_path(tool_name)
                    .unwrap_or_else(|| PathBuf::from(tool_name));
                
                debug!("Using tool path: {:?}", tool_path);
                
                let args = args_builder(input, output, &self.config);
                debug!("Command arguments: {:?}", args);
                
                let start_time = std::time::Instant::now();
                let success = Command::new(&tool_path)
                    .args(&args)
                    .status()
                    .await?
                    .success();
                let elapsed = start_time.elapsed();

                if success {
                    debug!("{} optimized successfully with {} in {:?}", format_name, tool_name, elapsed);
                    return Ok(PathBuf::from(output));
                } else {
                    warn!("{} optimization failed with {} after {:?}, trying next tool", tool_name, format_name, elapsed);
                }
            }
        }

        // Handle case where no tools are available or all failed
        if !any_tool_available {
            let tool_names: Vec<&str> = tools.iter().map(|(name, _)| *name).collect();
            error!("No {} optimization tools available ({})", format_name, tool_names.join("/"));
            Err(anyhow::anyhow!(
                "No {} optimization tools available. Please install one of: {}",
                format_name,
                tool_names.join(", ")
            ))
        } else {
            error!("All {} optimization tools failed for: {}", format_name, input);
            Err(anyhow::anyhow!(
                "All {} optimization tools failed to optimize: {}",
                format_name,
                input
            ))
        }
    }

    /// Helper for tools that output to stdout (like jpegoptim).
    /// 
    /// # Arguments
    /// * `tool_name` - Name of the tool to run
    /// * `args` - Command line arguments
    /// * `output_path` - Where to write the stdout output
    /// 
    /// # Returns
    /// * `Result<bool>` - True if successful, false if failed
    async fn run_tool_with_stdout_output(
        &self,
        tool_name: &str,
        args: &[String],
        output_path: &str,
    ) -> Result<bool> {
        let start_time = std::time::Instant::now();
        let output_data = Command::new(tool_name)
            .args(args)
            .output()
            .await?;
        let elapsed = start_time.elapsed();

        if output_data.status.success() {
            tokio::fs::write(output_path, output_data.stdout).await?;
            debug!("{} completed successfully in {:?}", tool_name, elapsed);
            Ok(true)
        } else {
            warn!("{} failed after {:?}", tool_name, elapsed);
            Ok(false)
        }
    }

    /// Gets image dimensions using ImageMagick identify command.
    /// 
    /// # Arguments
    /// * `image_path` - Path to the image file
    /// 
    /// # Returns
    /// * `Result<(u32, u32)>` - Width and height in pixels, or error if detection failed
    /// 
    /// # Example
    /// ```rust
    /// let (width, height) = processor.get_image_dimensions(&image_path).await?;
    /// println!("Image is {}x{} pixels", width, height);
    /// ```
    pub async fn get_image_dimensions(&self, image_path: &Path) -> Result<(u32, u32)> {
        let platform = PlatformCommands::instance();
        
        // Try ImageMagick 7.x first (magick identify)
        if let Some(magick_path) = platform.get_tool_path("magick") {
            if let Ok(output) = Command::new(magick_path)
                .args(&["identify", "-format", "%w %h", &image_path.to_string_lossy()])
                .output()
                .await
            {
                if output.status.success() {
                    let dimensions_str = String::from_utf8_lossy(&output.stdout);
                    let parts: Vec<&str> = dimensions_str.trim().split_whitespace().collect();
                    if parts.len() == 2 {
                        if let (Ok(width), Ok(height)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
                            debug!("Got dimensions {}x{} for {}", width, height, image_path.display());
                            return Ok((width, height));
                        }
                    }
                }
            }
        }
        
        // Try ImageMagick 6.x (identify)
        if let Some(identify_path) = platform.get_tool_path("identify") {
            if let Ok(output) = Command::new(identify_path)
                .args(&["-format", "%w %h", &image_path.to_string_lossy()])
                .output()
                .await
            {
                if output.status.success() {
                    let dimensions_str = String::from_utf8_lossy(&output.stdout);
                    let parts: Vec<&str> = dimensions_str.trim().split_whitespace().collect();
                    if parts.len() == 2 {
                        if let (Ok(width), Ok(height)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
                            debug!("Got dimensions {}x{} for {}", width, height, image_path.display());
                            return Ok((width, height));
                        }
                    }
                }
            }
        }
        
        Err(anyhow::anyhow!(
            "Unable to detect image dimensions for {}. ImageMagick tools (magick/identify) not available or failed.",
            image_path.display()
        ))
    }

    /// Checks if an image is larger than 2.5K resolution (2560x1440).
    /// 
    /// # Arguments
    /// * `image_path` - Path to the image file
    /// 
    /// # Returns
    /// * `Result<bool>` - True if image is larger than 2.5K, false otherwise
    pub async fn is_larger_than_4k(&self, image_path: &Path) -> Result<bool> {
        const MAX_2_5K_WIDTH: u32 = 2560;
        const MAX_2_5K_HEIGHT: u32 = 1440;
        
        let (width, height) = self.get_image_dimensions(image_path).await?;
        let is_larger = width > MAX_2_5K_WIDTH || height > MAX_2_5K_HEIGHT;
        
        if is_larger {
            info!("Image {}x{} is larger than 2.5K ({}x{}): {}", 
                  width, height, MAX_2_5K_WIDTH, MAX_2_5K_HEIGHT, image_path.display());
        }
        
        Ok(is_larger)
    }

    /// Pre-resizes an image to 2.5K resolution if it's larger, using optimal settings for speed.
    /// This should be called before optimization to avoid working with huge images.
    /// 
    /// # Arguments
    /// * `input_path` - Path to the original image
    /// * `temp_output_path` - Path where to save the resized image
    /// 
    /// # Returns
    /// * `Result<()>` - Success or error
    /// 
    /// # Note
    /// This function uses fast resize algorithms optimized for speed over quality,
    /// since the image will be further optimized afterward.
    pub async fn pre_resize_to_4k(&self, input_path: &Path, temp_output_path: &Path) -> Result<()> {
        const MAX_2_5K_WIDTH: u32 = 2560;
        const MAX_2_5K_HEIGHT: u32 = 1440;
        
        let platform = PlatformCommands::instance();
        let input_str = input_path.to_string_lossy();
        let output_str = temp_output_path.to_string_lossy();
        
        info!("Pre-resizing large image to 2.5K: {} -> {}", input_path.display(), temp_output_path.display());
        
        // Create parent directory if needed
        if let Some(parent) = temp_output_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        
        // Try tools in order of preference: magick, convert, vips
        let tools = ["magick", "convert", "vips"];
        
        for tool_name in &tools {
            if platform.is_command_available(tool_name).await {
                let tool_path = platform.get_tool_path(tool_name)
                    .unwrap_or_else(|| std::path::PathBuf::from(tool_name));
                
                let success = match *tool_name {
                    "magick" | "convert" => {
                        let resize_arg = format!("{}x{}>", MAX_2_5K_WIDTH, MAX_2_5K_HEIGHT); // > only shrinks, never enlarges
                        let args = to_string_vec([
                            &input_str,
                            "-resize", &resize_arg,
                            "-filter", "Lanczos", // Good quality and reasonable speed
                            "-colorspace", "sRGB", // Ensure consistent color space
                            "-quality", "95", // High quality for temp file (will be optimized later)
                            "-strip", // Remove metadata to avoid issues
                            "-limit", "memory", "512MB", // Reduced memory
                            "-limit", "disk", "2GB", // Reduced disk
                            &output_str
                        ]);
                        
                        debug!("Running pre-resize with {}: {:?}", tool_name, args);
                        
                        // Start timing
                        let start_time = std::time::Instant::now();
                        info!("Starting pre-resize command...");
                        
                        // Spawn process with timeout
                        let mut child = Command::new(&tool_path)
                            .args(&args)
                            .spawn()?;
                        
                        info!("Process spawned, waiting for completion...");
                        
                        let status = tokio::time::timeout(
                            std::time::Duration::from_secs(120), // Aumentato a 2 minuti per file grandi
                            child.wait()
                        ).await
                        .map_err(|_| anyhow::anyhow!("Pre-resize command timed out after 2 minutes"))?
                        .map_err(|e| anyhow::anyhow!("Pre-resize command failed: {}", e))?;
                        
                        let elapsed = start_time.elapsed();
                        info!("Pre-resize command completed in {:?}", elapsed);
                        
                        status.success()
                    },
                    "vips" => {
                        let args = to_string_vec([
                            "thumbnail",
                            &input_str,
                            &output_str,
                            &MAX_2_5K_WIDTH.to_string(),
                            "--height", &MAX_2_5K_HEIGHT.to_string(),
                            "--kernel", "mitchell",
                        ]);
                        
                        debug!("Running pre-resize with vips: {:?}", args);
                        let status = Command::new(&tool_path)
                            .args(&args)
                            .status()
                            .await?;
                        status.success()
                    },
                    _ => false,
                };
                
                if success {
                    info!("Pre-resize to 2.5K completed successfully with {}", tool_name);
                    return Ok(());
                } else {
                    warn!("{} pre-resize failed, trying next tool", tool_name);
                }
            }
        }
        
        Err(anyhow::anyhow!(
            "Unable to pre-resize image {}. No working tools (magick/convert/vips) found.",
            input_path.display()
        ))
    }

    /// Creates a temporary path for storing pre-resized images.
    /// 
    /// # Arguments
    /// * `original_path` - Path to the original image
    /// 
    /// # Returns
    /// * `Result<PathBuf>` - Path for the temporary resized image
    fn create_temp_resized_path(&self, original_path: &Path) -> Result<PathBuf> {
        let file_stem = original_path.file_stem()
            .ok_or_else(|| anyhow::anyhow!("Invalid file stem: {}", original_path.display()))?;
        let extension = original_path.extension()
            .and_then(|s| s.to_str())
            .unwrap_or("tmp");
        
        // Create temp path in system temp directory
        let temp_dir = std::env::temp_dir();
        let temp_filename = format!("{}_2_5k_temp.{}", file_stem.to_string_lossy(), extension);
        let temp_path = temp_dir.join(temp_filename);
        
        debug!("Created temp path for pre-resize: {} -> {}", 
               original_path.display(), temp_path.display());
        
        Ok(temp_path)
    }
}
