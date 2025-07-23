//! # Image Resize Module
//!
//! Questo modulo gestisce il ridimensionamento veloce delle immagini per creare thumbnails
//! utilizzando **tool esterni** per performance ottimali.
//!
//! ## Caratteristiche
//! - **SOLO RESIZE**: Nessuna compressione o ottimizzazione aggiuntiva
//! - **Massima velocità**: Filtro Mitchell per il miglior bilanciamento velocità/qualità
//! - **Tool esterni**: ImageMagick o libvips per performance native
//! - **Formati supportati**: JPEG, PNG, WebP
//! - **Conversione WebP**: Supporta conversione automatica a WebP se configurata
//! - **Preserva qualità**: I thumbnails mantengono la qualità delle immagini di input
//! - **Struttura directory**: Mantiene la gerarchia originale in /thumbnails
//!
//! ## Tool Strategy
//! **Priorità Tool (decrescente):**
//! 1. **magick** (ImageMagick 7.x) - Più moderno, migliori performance
//! 2. **convert** (ImageMagick 6.x/legacy) - Ampia compatibilità  
//! 3. **vips** (libvips) - Alternativa velocissima per batch processing
//! 4. **Error**: Se nessun tool disponibile
//!
//! ## Struttura Output
//! ```
//! /output
//! ├── originali/
//! │   ├── foto1.jpg
//! │   └── subfolder/
//! │       └── foto2.png
//! └── thumbnails/
//!     ├── gallery/
//!     │   ├── foto1.jpg (800x600)
//!     │   └── subfolder/
//!     │       └── foto2.jpg (800x600)
//!     └── mini/
//!         ├── foto1.jpg (150x150)
//!         └── subfolder/
//!             └── foto2.jpg (150x150)
//! ```
//!
//! ## Ottimizzazioni Velocità
//! - **-filter Mitchell**: Filtro veloce con buona qualità
//! - **-limit thread 1**: Single thread per evitare contesa di risorse
//! - **-limit memory 256MB**: Limita uso memoria per non rallentare il sistema
//! - **-define jpeg:size=WxH**: Pre-riduce JPEG grandi per velocità massima
//! - **-quality 95**: Preserva la qualità originale senza ricompressione aggressiva
//! - **Nessuna compressione aggiuntiva**: Solo resize, mantiene dimensioni appropriate

use crate::config::{Config, ThumbnailSize};
use crate::platform::PlatformCommands;
use crate::utils::to_string_vec;
use anyhow::Result;
use std::path::{Path, PathBuf};
use tokio::process::Command;
use tokio::sync::broadcast;
use tracing::{debug, info, warn, error};

/// Algoritmi di resize disponibili per ImageMagick
#[derive(Debug, Clone, Copy)]
pub enum ResizeAlgorithm {
    /// Lanczos - Migliore qualità per downscaling (default per thumbnails)
    Lanczos,
    /// Mitchell - Buon bilanciamento qualità/velocità
    Mitchell,
    /// Catrom - Catmull-Rom, buona qualità generale
    Catrom,
    /// Triangle - Veloce, qualità accettabile per anteprime veloci
    Triangle,
    /// Point - Pixel perfetto per upscaling pixel art
    Point,
}

impl Default for ResizeAlgorithm {
    fn default() -> Self {
        Self::Lanczos
    }
}

impl ResizeAlgorithm {
    /// Converte l'algoritmo in parametro ImageMagick
    pub fn to_imagemagick_filter(&self) -> &'static str {
        match self {
            ResizeAlgorithm::Lanczos => "Lanczos",
            ResizeAlgorithm::Mitchell => "Mitchell", 
            ResizeAlgorithm::Catrom => "Catrom",
            ResizeAlgorithm::Triangle => "Triangle",
            ResizeAlgorithm::Point => "Point",
        }
    }
}

/// Modalità di resize per gestire aspect ratio
#[derive(Debug, Clone, Copy)]
pub enum ResizeMode {
    /// Ridimensiona mantenendo aspect ratio, aggiunge padding se necessario
    Fit,
    /// Ridimensiona e croppa al centro per riempire esattamente le dimensioni  
    Fill,
    /// Ridimensiona senza preservare aspect ratio (stretching)
    Stretch,
}

impl Default for ResizeMode {
    fn default() -> Self {
        Self::Fit
    }
}

impl ResizeMode {
    /// Converte la modalità in geometry string per ImageMagick
    pub fn to_imagemagick_geometry(&self, width: u32, height: u32) -> String {
        match self {
            ResizeMode::Fit => format!("{}x{}", width, height),           // Preserva aspect ratio
            ResizeMode::Fill => format!("{}x{}^", width, height),         // Riempie, poi croppa
            ResizeMode::Stretch => format!("{}x{}!", width, height),      // Forza dimensioni esatte
        }
    }
}

/// Processore per il ridimensionamento delle immagini usando tool esterni
pub struct ImageResizer {
    /// Configurazione principale
    config: Config,
    /// Algoritmo di resize da utilizzare
    algorithm: ResizeAlgorithm,
    /// Modalità di resize
    mode: ResizeMode,
    /// Qualità JPEG per i thumbnails (1-100)
    jpeg_quality: u32,
    /// Se rimuovere i metadati (strip)
    strip_metadata: bool,
    /// Ricevitore per segnali di cancellazione
    stop_receiver: Option<broadcast::Receiver<()>>,
    /// Cache del tool risolto per evitare lookup ripetuti
    cached_tool: Option<(String, PathBuf)>, // (tool_name, tool_path)
}

impl ImageResizer {
    /// Crea un nuovo ridimensionatore di immagini
    ///
    /// # Arguments
    /// * `config` - Configurazione che include le dimensioni dei thumbnails
    /// * `algorithm` - Algoritmo di resize da utilizzare
    /// * `mode` - Modalità di resize per gestire l'aspect ratio
    /// * `jpeg_quality` - Qualità JPEG per i thumbnails (default: 85)
    /// * `strip_metadata` - Se rimuovere i metadati (default: true)
    ///
    /// # Returns
    /// * `Result<Self>` - Nuova istanza del ridimensionatore
    pub fn new(
        config: Config, 
        algorithm: ResizeAlgorithm, 
        mode: ResizeMode,
        jpeg_quality: Option<u32>,
        strip_metadata: bool,
    ) -> Result<Self> {
        Ok(Self {
            config,
            algorithm,
            mode,
            jpeg_quality: jpeg_quality.unwrap_or(85),
            strip_metadata,
            stop_receiver: None,
            cached_tool: None,
        })
    }

    /// Crea un nuovo ridimensionatore con supporto per cancellazione
    pub fn new_with_cancellation(
        config: Config,
        algorithm: ResizeAlgorithm,
        mode: ResizeMode,
        jpeg_quality: Option<u32>,
        strip_metadata: bool,
        stop_receiver: broadcast::Receiver<()>,
    ) -> Result<Self> {
        Ok(Self {
            config,
            algorithm,
            mode,
            jpeg_quality: jpeg_quality.unwrap_or(85),
            strip_metadata,
            stop_receiver: Some(stop_receiver),
            cached_tool: None,
        })
    }

    /// Controlla se è stato ricevuto un segnale di stop
    fn should_stop(&mut self) -> bool {
        if let Some(ref mut receiver) = self.stop_receiver {
            match receiver.try_recv() {
                Ok(_) => {
                    debug!("Stop signal received, cancelling image resizing");
                    return true;
                }
                Err(broadcast::error::TryRecvError::Empty) => return false,
                Err(broadcast::error::TryRecvError::Lagged(_)) => {
                    debug!("Stop signal was lagged, cancelling image resizing");
                    return true;
                }
                Err(broadcast::error::TryRecvError::Closed) => return false,
            }
        }
        false
    }

    /// Crea tutti i thumbnails per un'immagine usando tool esterni
    ///
    /// # Arguments
    /// * `input_path` - Path dell'immagine originale
    /// * `input_base_dir` - Directory base per calcolare i path relativi
    ///
    /// # Returns
    /// * `Result<Vec<PathBuf>>` - Lista dei path dei thumbnails creati
    ///
    /// # Errors
    /// Ritorna errore se:
    /// - L'operazione viene cancellata
    /// - Nessun tool di resize disponibile
    /// - Non è possibile creare le directory di output
    /// - Il ridimensionamento fallisce
    pub async fn create_thumbnails(
        &mut self,
        input_path: &Path,
        input_base_dir: &Path,
    ) -> Result<Vec<PathBuf>> {
        // Controlla cancellazione
        if self.should_stop() {
            return Err(anyhow::anyhow!("Thumbnail creation cancelled by user"));
        }

        // Verifica che thumbnails siano configurati
        if self.config.thumbnails.is_empty() {
            return Ok(Vec::new());
        }

        // Verifica che sia configurata una directory di output
        let output_base = self.config.output_path.as_ref()
            .ok_or_else(|| anyhow::anyhow!(
                "Thumbnail creation requires an output directory to be configured"
            ))?
            .clone(); // Clone per evitare il borrow

        debug!("Creating thumbnails for: {}", input_path.display());

        let mut thumbnail_paths = Vec::new();

        // Crea ogni tipo di thumbnail configurato
        let thumbnails_config = self.config.thumbnails.clone(); // Clone per evitare il borrow
        for (thumbnail_name, thumbnail_size) in &thumbnails_config {
            if self.should_stop() {
                return Err(anyhow::anyhow!("Thumbnail creation cancelled by user"));
            }

            let thumbnail_path = self
                .create_single_thumbnail(
                    input_path,
                    input_base_dir,
                    &output_base,
                    thumbnail_name,
                    thumbnail_size,
                )
                .await?;

            thumbnail_paths.push(thumbnail_path);
        }

        info!(
            "Created {} thumbnails for {}",
            thumbnail_paths.len(),
            input_path.file_name().unwrap_or_default().to_string_lossy()
        );

        Ok(thumbnail_paths)
    }

    /// Crea un singolo thumbnail usando tool esterni
    async fn create_single_thumbnail(
        &mut self,
        input_path: &Path,
        input_base_dir: &Path,
        output_base: &Path,
        thumbnail_name: &str,
        thumbnail_size: &ThumbnailSize,
    ) -> Result<PathBuf> {
        // Calcola il path di output per questo thumbnail
        let output_path = self.get_thumbnail_path(
            input_path,
            input_base_dir,
            output_base,
            thumbnail_name,
        )?;

        // Crea le directory necessarie
        if let Some(parent) = output_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        debug!(
            "Resizing {} to {}x{} for thumbnail '{}' using {}",
            input_path.file_name().unwrap_or_default().to_string_lossy(),
            thumbnail_size.width,
            thumbnail_size.height,
            thumbnail_name,
            self.algorithm.to_imagemagick_filter()
        );

        // Prova i tool di resize in ordine di preferenza
        let success = self.try_resize_tools(input_path, &output_path, thumbnail_size).await?;
        
        if !success {
            return Err(anyhow::anyhow!(
                "All resize tools failed for: {}",
                input_path.display()
            ));
        }

        debug!("Saved thumbnail: {}", output_path.display());
        Ok(output_path)
    }

    /// Prova i tool di resize in ordine di preferenza
    async fn try_resize_tools(
        &mut self,
        input_path: &Path,
        output_path: &Path,
        thumbnail_size: &ThumbnailSize,
    ) -> Result<bool> {
        // Usa la cache se disponibile
        if let Some((tool_name, tool_path)) = &self.cached_tool {
            debug!("Using cached tool for thumbnail creation: {}", tool_name);
            return self.run_tool(tool_name, tool_path, input_path, output_path, thumbnail_size).await;
        }

        // Prima volta: trova e casha il tool
        let platform = PlatformCommands::instance();
        
        // Define tools with their argument builders in order of preference  
        let tools: &[&str] = &[
            "magick",      // ImageMagick 7.x
            "convert",     // ImageMagick 6.x/legacy
            "vips",        // libvips (velocissimo)
        ];

        for tool_name in tools {
            if platform.is_command_available(tool_name).await {
                debug!("Found and caching tool for thumbnail creation: {}", tool_name);
                
                // Get the resolved tool path (bundled or system)
                let tool_path = platform.get_tool_path(tool_name)
                    .unwrap_or_else(|| PathBuf::from(tool_name));
                
                // Cache il tool per le prossime volte
                self.cached_tool = Some((tool_name.to_string(), tool_path.clone()));
                
                return self.run_tool(tool_name, &tool_path, input_path, output_path, thumbnail_size).await;
            }
        }

        Err(anyhow::anyhow!("No suitable thumbnail creation tool found"))
    }

    /// Esegue un tool specifico per creare un thumbnail
    async fn run_tool(
        &self,
        tool_name: &str,
        tool_path: &Path,
        input_path: &Path,
        output_path: &Path,
        thumbnail_size: &ThumbnailSize,
    ) -> Result<bool> {
        let input_str = input_path.to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid input path: {:?}", input_path))?;
        let output_str = output_path.to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid output path: {:?}", output_path))?;

        let args = match tool_name {
            "magick" => self.build_magick_args(input_str, output_str, thumbnail_size),
            "convert" => self.build_convert_args(input_str, output_str, thumbnail_size),
            "vips" => self.build_vips_args(input_str, output_str, thumbnail_size),
            _ => return Err(anyhow::anyhow!("Unknown tool: {}", tool_name)),
        };

        debug!("Command: {:?} {:?}", tool_path, args);
        
        let success = Command::new(tool_path)
            .args(&args)
            .status()
            .await?
            .success();

        if success {
            debug!("Thumbnail created successfully with {}", tool_name);
            Ok(true)
        } else {
            warn!("Thumbnail creation failed with {}", tool_name);
            Ok(false)
        }
    }

    /// Costruisce gli argomenti per ImageMagick 7.x (magick) - SOLO RESIZE VELOCE
    fn build_magick_args(&self, input: &str, output: &str, thumbnail_size: &ThumbnailSize) -> Vec<String> {
        let geometry = self.mode.to_imagemagick_geometry(thumbnail_size.width, thumbnail_size.height);
        
        let mut args = to_string_vec([input]);

        // Pre-ridimensionamento veloce per JPEG grandi (ottimizzazione di velocità)
        if input.ends_with(".jpg") || input.ends_with(".jpeg") {
            let pre_size = (thumbnail_size.width.max(thumbnail_size.height) * 2).min(2048);
            args.extend(to_string_vec([
                "-define", &format!("jpeg:size={}x{}", pre_size, pre_size),
            ]));
        }

        // Usa sempre Mitchell per massima velocità
        args.extend(to_string_vec([
            "-filter", "Mitchell",  // Veloce e qualità decente
            "-resize", &geometry,
        ]));

        // Aggiungi crop per modalità Fill
        if matches!(self.mode, ResizeMode::Fill) {
            args.extend(to_string_vec([
                "-gravity", "center",
                "-extent", &format!("{}x{}", thumbnail_size.width, thumbnail_size.height),
            ]));
        }

        // Limiti memoria per non rallentare il sistema
        args.extend(to_string_vec([
            "-limit", "memory", "256MB",
            "-limit", "disk", "1GB",
            "-limit", "thread", "1",  // Single thread per evitare contesa
        ]));

        // IMPORTANTE: Preserva la qualità originale per tutti i formati
        if output.ends_with(".jpg") || output.ends_with(".jpeg") {
            args.extend(to_string_vec(["-quality", "95"])); // Alta qualità per preservare l'originale
        } else if output.ends_with(".webp") {
            // Per WebP thumbnails usa sempre qualità fissa 80 per buon bilanciamento qualità/dimensioni
            args.extend(to_string_vec(["-quality", "80"]));
        }

        args.push(output.to_string());
        args
    }

    /// Costruisce gli argomenti per ImageMagick 6.x (convert) - SOLO RESIZE VELOCE
    fn build_convert_args(&self, input: &str, output: &str, thumbnail_size: &ThumbnailSize) -> Vec<String> {
        let geometry = self.mode.to_imagemagick_geometry(thumbnail_size.width, thumbnail_size.height);
        
        let mut args = to_string_vec([input]);

        // Pre-ridimensionamento veloce per JPEG grandi (ottimizzazione di velocità)
        if input.ends_with(".jpg") || input.ends_with(".jpeg") {
            let pre_size = (thumbnail_size.width.max(thumbnail_size.height) * 2).min(2048);
            args.extend(to_string_vec([
                "-define", &format!("jpeg:size={}x{}", pre_size, pre_size),
            ]));
        }

        // Usa sempre Mitchell per massima velocità
        args.extend(to_string_vec([
            "-filter", "Mitchell",  // Veloce e qualità decente
            "-resize", &geometry,
        ]));

        // Aggiungi crop per modalità Fill
        if matches!(self.mode, ResizeMode::Fill) {
            args.extend(to_string_vec([
                "-gravity", "center",
                "-extent", &format!("{}x{}", thumbnail_size.width, thumbnail_size.height),
            ]));
        }

        // Limiti memoria per non rallentare il sistema  
        args.extend(to_string_vec([
            "-limit", "memory", "256MB",
            "-limit", "disk", "1GB",
            "-limit", "thread", "1",  // Single thread per evitare contesa
        ]));

        // IMPORTANTE: Preserva la qualità originale per tutti i formati
        if output.ends_with(".jpg") || output.ends_with(".jpeg") {
            args.extend(to_string_vec(["-quality", "95"])); // Alta qualità per preservare l'originale
        } else if output.ends_with(".webp") {
            // Per WebP thumbnails usa sempre qualità fissa 80 per buon bilanciamento qualità/dimensioni
            args.extend(to_string_vec(["-quality", "80"]));
        }

        args.push(output.to_string());
        args
    }

    /// Costruisce gli argomenti per libvips - SOLO RESIZE VELOCE
    fn build_vips_args(&self, input: &str, output: &str, thumbnail_size: &ThumbnailSize) -> Vec<String> {
        let mut args = to_string_vec([
            "thumbnail",
            input,
            output,
            &thumbnail_size.width.to_string(),
        ]);

        // Parametri di base per libvips
        args.extend(to_string_vec([
            "--height", &thumbnail_size.height.to_string(),
            "--kernel", "mitchell",  // Veloce e buona qualità
        ]));

        // Modalità resize per libvips
        match self.mode {
            ResizeMode::Fit => {
                // Default behavior - mantiene aspect ratio
            }
            ResizeMode::Fill => {
                args.extend(to_string_vec(["--crop", "centre"]));
            }
            ResizeMode::Stretch => {
                args.push("--no-rotate".to_string());
            }
        }

        // IMPORTANTE: Preserva la qualità originale
        if output.ends_with(".jpg") || output.ends_with(".jpeg") {
            args.extend(to_string_vec(["--Q", "95"])); // Alta qualità per JPEG
        } else if output.ends_with(".webp") {
            // Per WebP thumbnails usa sempre qualità fissa 80 per buon bilanciamento qualità/dimensioni
            args.extend(to_string_vec(["--Q", "80"]));
        }

        args
    }

    /// Calcola il path per un thumbnail
    fn get_thumbnail_path(
        &self,
        input_path: &Path,
        input_base_dir: &Path,
        output_base: &Path,
        thumbnail_name: &str,
    ) -> Result<PathBuf> {
        // Estrae il nome del file e l'estensione
        let file_stem = input_path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid file name: {:?}", input_path))?;

        // Determina l'estensione basandosi sulla configurazione
        let extension = if self.config.convert_to_webp {
            // Se la conversione a WebP è abilitata, usa WebP per i thumbnails
            "webp"
        } else {
            // Altrimenti mantieni il formato dell'immagine originale
            input_path.extension()
                .and_then(|s| s.to_str())
                .unwrap_or("jpg") // Fallback a JPEG
        };

        let filename = format!("{}.{}", file_stem, extension);

        // Calcola il path relativo dall'input base
        let relative_path = input_path
            .strip_prefix(input_base_dir)
            .unwrap_or(input_path)
            .parent()
            .unwrap_or(Path::new(""));

        // Costruisce: output_base/thumbnails/thumbnail_name/relative_path/filename
        let thumbnail_path = output_base
            .join("thumbnails")
            .join(thumbnail_name)
            .join(relative_path)
            .join(filename);

        Ok(thumbnail_path)
    }

    /// Controlla che tutti i tool necessari per i thumbnails siano disponibili
    pub async fn check_dependencies() -> Result<()> {
        let platform = PlatformCommands::instance();
        let mut available_tools = Vec::new();
        
        info!("🔧 Checking thumbnail creation tool dependencies...");
        
        // Check ImageMagick tools
        if platform.is_command_available("magick").await {
            available_tools.push("ImageMagick 7.x (magick)");
        } else if platform.is_command_available("convert").await {
            available_tools.push("ImageMagick 6.x (convert)");
        }
        
        // Check libvips
        if platform.is_command_available("vips").await {
            available_tools.push("libvips (vips)");
        }
        
        if available_tools.is_empty() {
            let error_msg = "No thumbnail creation tools available! Please install ImageMagick or libvips";
            error!("{}", error_msg);
            return Err(anyhow::anyhow!(error_msg));
        }
        
        info!("✅ Available thumbnail tools: {}", available_tools.join(", "));
        Ok(())
    }

    /// Stampa informazioni sui thumbnails configurati
    pub fn print_thumbnail_config(&self) {
        if self.config.thumbnails.is_empty() {
            info!("📸 No thumbnails configured");
            return;
        }

        info!("📸 Configured thumbnails:");
        for (name, size) in &self.config.thumbnails {
            info!(
                "  • {} - {}x{} (mode: {:?}, algorithm: {:?})",
                name, size.width, size.height, self.mode, self.algorithm
            );
        }
    }
}

/// Funzioni di utilità per la gestione dei thumbnails
impl ImageResizer {
    /// Verifica se un file è un'immagine supportata per il resize
    pub fn is_supported_for_resize(path: &Path) -> bool {
        match path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()).as_deref() {
            Some("jpg") | Some("jpeg") | Some("png") | Some("webp") => true,
            _ => false,
        }
    }

    /// Crea un canale di cancellazione per i thumbnails
    pub fn create_cancellation_channel(capacity: usize) -> (broadcast::Sender<()>, broadcast::Receiver<()>) {
        broadcast::channel(capacity)
    }

    /// Stima la dimensione totale dei thumbnails che verranno creati
    pub fn estimate_thumbnail_count(&self, image_files: &[PathBuf]) -> usize {
        let supported_files = image_files
            .iter()
            .filter(|path| Self::is_supported_for_resize(path))
            .count();

        supported_files * self.config.thumbnails.len()
    }
}
