//! # Optimizer Module
//!
//! Modulo ottimizzato che separa le responsabilità in sottomuduli:
//! - `media_optimizer`: Orchestratore principale
//! - `task_optimizer`: Worker per singoli file
//! - `progress_tracker`: Gestione progress unificata
//! - `path_resolver`: Logica di calcolo path centralizzata

pub mod media_optimizer;
pub mod task_optimizer;
pub mod progress_tracker;
pub mod path_resolver;

// Re-export delle struct principali per backward compatibility
pub use media_optimizer::MediaOptimizer;
pub use task_optimizer::TaskOptimizer;
pub use progress_tracker::ProgressTracker;
pub use path_resolver::PathResolver;
