//! # Space Media Optimizer Library
//!
//! Questo è il modulo principale della libreria che espone tutte le API pubbliche.
//! 
//! ## Responsabilità:
//! - Definisce la struttura modulare dell'applicazione
//! - Espone i tipi e le funzioni principali tramite re-exports
//! - Fornisce un'interfaccia pulita per il main.rs e per altri consumatori
//! 
//! ## Architettura dei moduli:
//! - `config`: Gestione configurazione e validazione parametri
//! - `error`: Tipi di errore custom per diverse operazioni
//! - `state`: Tracking file processati e persistenza stato
//! - `file_manager`: Operazioni sui file e discovery media
//! - `image_processor`: Ottimizzazione immagini (JPEG/PNG/WebP)
//! - `video_processor`: Ottimizzazione video (MP4/MOV/AVI)
//! - `optimizer`: Orchestratore principale del processo
//! - `progress`: Progress tracking e statistiche
//! 
//! ## Utilizzo:
//! ```rust
//! use space_media_optimizer::{Config, MediaOptimizer};
//! 
//! let config = Config::default();
//! let optimizer = MediaOptimizer::new(&path, config).await?;
//! optimizer.run(&path).await?;
//! ```

pub mod config;
pub mod error;
pub mod state;
pub mod optimizer;
pub mod image_processor;
pub mod video_processor;
pub mod file_manager;
pub mod platform;
pub mod progress;

pub use config::Config;
pub use error::OptimizeError;
pub use state::{StateFile, ProcessedFile};
pub use optimizer::MediaOptimizer;
