//! # Error Types Module
//!
//! Questo modulo definisce tutti i tipi di errore custom dell'applicazione.
//! 
//! ## Responsabilità:
//! - Definisce `OptimizeError` enum per categorizzare tutti gli errori possibili
//! - Fornisce messaggi di errore descrittivi e strutturati
//! - Integra con `thiserror` per automatic error conversion
//! - Supporta error chaining per mantenere il contesto degli errori
//! 
//! ## Categorie di errori:
//! - `Io`: Errori di I/O (file non trovati, permessi, etc.)
//! - `Image`: Errori di elaborazione immagini (formati corrotti, etc.)
//! - `FFmpeg`: Errori di elaborazione video con FFmpeg
//! - `Metadata`: Errori di preservazione metadata EXIF
//! - `State`: Errori di gestione file di stato
//! - `UnsupportedFormat`: Formato file non supportato
//! - `MissingDependency`: Tool esterno mancante (ffmpeg, exiftool)
//! - `Validation`: Errori di validazione input
//! 
//! ## Vantaggi:
//! - Errori tipizzati per handling specifico
//! - Messaggi chiari per // debugging
//! - Automatic conversion da errori standard
//! - Integration con `anyhow` per error propagation
//! 
//! ## Esempio:
//! ```rust
//! if !tool_exists {
//!     return Err(OptimizeError::MissingDependency("ffmpeg".to_string()));
//! }
//! ```

/// Custom error types for media optimization
#[derive(thiserror::Error, Debug)]
pub enum OptimizeError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Image processing error: {0}")]
    Image(#[from] image::ImageError),
    
    #[error("FFmpeg error: {0}")]
    FFmpeg(String),
    
    #[error("Metadata preservation error: {0}")]
    Metadata(String),
    
    #[error("State file error: {0}")]
    State(String),
    
    #[error("Unsupported file format: {0}")]
    UnsupportedFormat(String),
    
    #[error("Dependency missing: {0}")]
    MissingDependency(String),
    
    #[error("File validation error: {0}")]
    Validation(String),
}
