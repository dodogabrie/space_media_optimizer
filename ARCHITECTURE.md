# Space Media Optimizer - Documentazione Tecnica

## 📋 Panoramica Architetturale

Questa documentazione spiega nel dettaglio ogni modulo e le sue responsabilità specifiche.

## 🏗️ Architettura Modulare

### 📁 Moduli e Responsabilità

| Modulo | Scopo Principale | Componenti Chiave |
|--------|------------------|-------------------|
| `main.rs` | **Entry Point** | CLI parsing, logging setup, orchestrazione |
| `lib.rs` | **Library Interface** | Re-exports, API pubblica |
| `config.rs` | **Configuration** | Parametri, validazione, persistence |
| `error.rs` | **Error Handling** | Tipi errore, categorizzazione |
| `state.rs` | **State Management** | File tracking, persistence, cleanup |
| `file_manager.rs` | **File Operations** | Discovery, operazioni sicure |
| `image_processor.rs` | **Image Optimization** | JPEG/PNG/WebP processing |
| `video_processor.rs` | **Video Optimization** | FFmpeg integration |
| `progress.rs` | **Progress & Stats** | Visual feedback, statistiche |
| `optimizer.rs` | **Main Orchestrator** | Coordinamento, concorrenza |

## 🔄 Flusso di Esecuzione Dettagliato

### 1. Inizializzazione (`main.rs`)
```
┌─────────────────┐
│ Parse CLI Args  │
├─────────────────┤
│ Setup Logging   │
├─────────────────┤
│ Validate Input  │
├─────────────────┤
│ Create Config   │
└─────────────────┘
        │
        ▼
```

### 2. Orchestrazione (`optimizer.rs`)
```
┌──────────────────┐
│ Create Optimizer │
├──────────────────┤
│ Check Deps       │
├──────────────────┤
│ Load State       │
├──────────────────┤
│ Find Media Files │
├──────────────────┤
│ Start Workers    │
└──────────────────┘
        │
        ▼
```

### 3. Processing Parallelo
```
┌─────────────────┬─────────────────┬─────────────────┐
│   Worker 1      │   Worker 2      │   Worker N      │
│                 │                 │                 │
│ ┌─────────────┐ │ ┌─────────────┐ │ ┌─────────────┐ │
│ │Check State  │ │ │Check State  │ │ │Check State  │ │
│ ├─────────────┤ │ ├─────────────┤ │ ├─────────────┤ │
│ │Determine    │ │ │Determine    │ │ │Determine    │ │
│ │Type         │ │ │Type         │ │ │Type         │ │
│ ├─────────────┤ │ ├─────────────┤ │ ├─────────────┤ │
│ │Optimize     │ │ │Optimize     │ │ │Optimize     │ │
│ ├─────────────┤ │ ├─────────────┤ │ ├─────────────┤ │
│ │Check Size   │ │ │Check Size   │ │ │Check Size   │ │
│ ├─────────────┤ │ ├─────────────┤ │ ├─────────────┤ │
│ │Replace/Skip │ │ │Replace/Skip │ │ │Replace/Skip │ │
│ ├─────────────┤ │ ├─────────────┤ │ ├─────────────┤ │
│ │Update State │ │ │Update State │ │ │Update State │ │
│ └─────────────┘ │ └─────────────┘ │ └─────────────┘ │
└─────────────────┴─────────────────┴─────────────────┘
```

## 🔧 Dettagli Implementativi

### Gestione Stato (`state.rs`)
- **File location**: `~/.media-optimizer/processed_files_<hash>.json`
- **Hash strategy**: SHA256 del path directory (16 char)
- **Tracking method**: Path + modification time
- **Persistence**: Async JSON serialization

### Concorrenza (`optimizer.rs`)
- **Semaphore-based**: Limita worker concorrenti
- **Task isolation**: Ogni worker ha propri processors
- **Thread-safe progress**: Arc<ProgressBar> per updates
- **Error isolation**: Errori per file non bloccano altri

### Processing Pipeline

#### Per Immagini (`image_processor.rs`):
```
Original File → Load → Compress → Save Temp → Preserve EXIF → Return
     │            │        │          │            │
     │         image     format     temp file   exiftool
     │         crate    specific                command
```

#### Per Video (`video_processor.rs`):
```
Original File → FFmpeg Compress → Preserve Metadata → Return
     │               │                    │
     │          H.264 + AAC         exiftool
     │         CRF + bitrate        command
```

### Threshold Logic
```rust
fn should_replace(original: u64, optimized: u64, threshold: f64) -> bool {
    (optimized as f64) < (original as f64 * threshold)
}

// Esempi con threshold=0.9:
// 1000 bytes → 800 bytes = 80% of original → REPLACE (20% saved)
// 1000 bytes → 950 bytes = 95% of original → SKIP (5% saved)
```

## 🧪 Testing Strategy

### Test Coverage per Modulo:
- `config.rs`: ✅ Validation, serialization, defaults
- `error.rs`: ✅ Error conversion, messages
- `state.rs`: 🔄 File operations, cleanup
- `file_manager.rs`: 🔄 Discovery, safe operations
- `image_processor.rs`: 🔄 Format handling
- `video_processor.rs`: 🔄 FFmpeg integration
- `progress.rs`: 🔄 Statistics calculation
- `optimizer.rs`: 🔄 End-to-end flows

### Tipi di Test:
- **Unit tests**: Singole funzioni
- **Integration tests**: Moduli insieme
- **End-to-end tests**: Flusso completo
- **Property tests**: Input edge cases

## 🚀 Performance Characteristics

### Bottlenecks Identificati:
1. **I/O bound**: File reading/writing
2. **CPU bound**: Image/video compression
3. **Memory**: File buffers durante processing

### Ottimizzazioni Implementate:
1. **Parallelismo**: Worker pool configurabile
2. **Async I/O**: Tokio per non-blocking operations
3. **Streaming**: No caricamento file interi in memory
4. **State caching**: Evita reprocessing inutile

### Metrics Tipiche:
- **Throughput**: 4-8 file/secondo (dipende da size/workers)
- **Memory footprint**: ~10-50MB (constant)
- **CPU usage**: ~80% durante processing attivo
- **I/O pattern**: Bursty (read original → write compressed)

## 🔐 Sicurezza e Robustezza

### Sicurezza File Operations:
- **Backup automatico** prima di sostituire
- **Atomic operations** dove possibile
- **Validation** esistenza file
- **Rollback** in caso di errore

### Error Recovery:
- **Graceful degradation**: Errori singoli non bloccano batch
- **Detailed logging**: Per debugging issues
- **State consistency**: State sempre valido anche con crash
- **Resource cleanup**: Temp files sempre rimossi

### Input Validation:
- **Path validation**: Esistenza directory
- **Parameter bounds**: Quality, CRF, threshold ranges
- **Dependency checks**: Tool esterni disponibili

## 📊 Monitoring e Observability

### Logging Levels:
- **ERROR**: Errori critici, dependency missing
- **WARN**: Metadata failures, tool warnings
- **INFO**: Progress updates, file operations
- **DEBUG**: Detailed execution flow

### Metriche Disponibili:
- Files processed/optimized/skipped
- Bytes saved (totale e per file)
- Percentuali riduzione
- Tempo processing
- Error rate

### State Introspection:
```bash
# Vedi file state
cat ~/.media-optimizer/processed_files_*.json | jq

# Statistics storiche
cat ~/.media-optimizer/processed_files_*.json | jq '.processed_files | length'
```

## 🔮 Estensibilità Futura

### Nuovi Formati:
- Aggiungere entry in `FileManager::is_supported_format()`
- Implementare handler in processor appropriato
- Aggiungere dependency check se richiesto

### Nuovi Processor:
- Implementare trait comune per processors
- Aggiungere al factory pattern in `optimizer.rs`
- Estendere error types se necessario

### Nuove Features:
- **Batch configuration**: Config files per progetti
- **Cloud storage**: S3, Azure Blob support
- **Backup strategies**: Multiple backup locations
- **Quality analysis**: SSIM, PSNR metrics
- **Web interface**: REST API + web UI

Questa architettura modulare rende molto semplice aggiungere nuove funzionalità mantenendo il codice esistente stabile e testabile.
