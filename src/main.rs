//! # Space Media Optimizer - Main Entry Point
//!
//! Questo è il punto di ingresso principale dell'applicazione.
//! 
//! ## Responsabilità:
//! - Parsing degli argomenti della command line con `clap`
//! - Inizializzazione del sistema di logging con `tracing`
//! - Validazione degli input dell'utente
//! - Creazione della configurazione e avvio dell'optimizer
//! 
//! ## Flusso di esecuzione:
//! 1. Parsa gli argomenti CLI (directory, quality, crf, workers, etc.)
//! 2. Configura il logging (INFO o DEBUG a seconda del flag verbose)
//! 3. Valida che la directory media esista
//! 4. Crea un oggetto Config con tutti i parametri
//! 5. Istanzia MediaOptimizer e avvia il processo di ottimizzazione
//! 
//! ## Esempio di utilizzo:
//! ```bash
//! media-optimizer /path/to/media --quality 85 --workers 8 --verbose
//! ```

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use tracing::info;

use space_media_optimizer::{Config, MediaOptimizer};

#[derive(Parser)]
#[command(name = "media-optimizer")]
#[command(about = "Optimize images and videos with smart deduplication")]
struct Args {
    /// Directory containing media files to optimize
    media_directory: PathBuf,
    
    /// JPEG quality (1-100)
    #[arg(short, long, default_value = "80")]
    quality: u8,
    
    /// Video CRF value (0-51, lower = better quality)
    #[arg(short, long, default_value = "26")]
    crf: u8,
    
    /// Video audio bitrate
    #[arg(short, long, default_value = "128k")]
    audio_bitrate: String,
    
    /// Size threshold (keep if new size < original * threshold)
    #[arg(short, long, default_value = "0.9")]
    threshold: f64,
    
    /// Number of parallel workers
    #[arg(short, long, default_value = "4")]
    workers: usize,
    
    /// Dry run - don't actually replace files
    #[arg(long)]
    dry_run: bool,
    
    /// Output directory for optimized files (if not specified, replace originals in place)
    #[arg(short, long)]
    output: Option<PathBuf>,
    
    /// Convert all media to WebP format for maximum compression
    #[arg(long)]
    webp: bool,
    
    /// WebP quality (1-100, only used when --webp is enabled)
    #[arg(long, default_value = "80")]
    webp_quality: u8,
    
    /// Skip files that have already been processed (even when using output directory)
    #[arg(long)]
    keep_processed: bool,
    
    /// Skip video compression (just copy videos to output)
    #[arg(long)]
    skip_video_compression: bool,
    
    /// Verbose logging
    #[arg(short, long)]
    verbose: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    
    // Initialize logging
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(if args.verbose {
            tracing::Level::DEBUG
        } else {
            tracing::Level::INFO
        })
        .finish();
    
    tracing::subscriber::set_global_default(subscriber)?;
    
    // Validate arguments
    if !args.media_directory.exists() {
        return Err(anyhow::anyhow!("Media directory does not exist: {}", args.media_directory.display()));
    }
    
    // Validate and create output directory if specified
    if let Some(ref output_dir) = args.output {
        if !output_dir.exists() {
            std::fs::create_dir_all(output_dir)?;
            info!("Created output directory: {}", output_dir.display());
        }
        if !output_dir.is_dir() {
            return Err(anyhow::anyhow!("Output path is not a directory: {}", output_dir.display()));
        }
    }
    
    let config = Config {
        jpeg_quality: args.quality,
        video_crf: args.crf,
        audio_bitrate: args.audio_bitrate,
        size_threshold: args.threshold,
        dry_run: args.dry_run,
        workers: args.workers,
        output_path: args.output,
        convert_to_webp: args.webp,
        webp_quality: args.webp_quality,
        keep_processed: args.keep_processed,
        skip_video_compression: args.skip_video_compression,
    };
    
    let mut optimizer = MediaOptimizer::new(&args.media_directory, config).await?;
    optimizer.run(&args.media_directory).await?;
    
    Ok(())
}