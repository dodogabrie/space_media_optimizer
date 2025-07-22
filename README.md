# Space Media Optimizer

Un ottimizzatore di file multimediali in Rust che riduce le dimensioni di immagini e video.

## Caratteristiche

-  **Performance elevate**: Elaborazione parallela con controllo della concorrenza
-  **Gestione degli stati dei files**: Evita la rielaborazione di file già ottimizzati
- ️ **Supporto immagini**: JPEG, PNG, WebP con preservazione metadati EXIF
-  **Supporto video**: MP4, MOV, AVI, MKV, WebM con compressione H.264
-  **Progress tracking**: Barre di progresso e statistiche dettagliate
-  **Sicurezza**: Backup automatici e validazione dell'input
-  **Configurabile**: Parametri e soglie personalizzabili

## Struttura del Progetto

```
src/
├── lib.rs              # Modulo principale e re-exports
├── main.rs             # Entry point del programma
├── config.rs           # Gestione configurazione
├── error.rs            # Tipi di errore personalizzati
├── state.rs            # Gestione stato e tracking file processati
├── file_manager.rs     # Operazioni sui file e discovery
├── image_processor.rs  # Ottimizzazione immagini
├── video_processor.rs  # Ottimizzazione video
├── optimizer.rs        # Orchestratore principale
└── progress.rs         # Progress tracking e statistiche
```

### Architettura Modulare

Ogni modulo ha una responsabilità specifica:

#### `config.rs`
- Gestione configurazione con validazione
- Caricamento/salvataggio da file JSON
- Valori di default sensati

#### `error.rs`
- Tipi di errore specifici per ogni operazione
- Gestione errori robusta con `thiserror`

#### `state.rs`
- Tracking dei file processati con hash della directory
- Prevenzione rielaborazione basata su modification time
- Cleanup automatico di entry obsolete

#### `file_manager.rs`
- Discovery di file media supportati
- Operazioni sicure sui file con backup
- Utilità per formattazione dimensioni e calcoli

#### `image_processor.rs`
- Ottimizzazione JPEG, PNG, WebP
- Preservazione metadata EXIF
- Controllo qualità configurabile

#### `video_processor.rs`
- Compressione video con FFmpeg
- Preservazione metadata video
- Informazioni video con ffprobe

#### `optimizer.rs`
- Orchestratore principale del processo
- Gestione concorrenza con semafori
- Coordinamento tra tutti i componenti

#### `progress.rs`
- Progress bars con `indicatif`
- Tracking statistiche di ottimizzazione
- Report finali dettagliati

## Installazione

### Prerequisiti

```bash
# Su Ubuntu/Debian
sudo apt install ffmpeg exiftool

# Su macOS
brew install ffmpeg exiftool

# Su Fedora/RHEL
sudo dnf install ffmpeg exiftool
```

### Compilazione

```bash
git clone <repository>
cd space_media_optimizer
cargo build --release
```

## Utilizzo

### Comando base
```bash
./quick-start.sh /path/to/media/directory
```

### Opzioni avanzate
```bash
./quick-start.sh \
  /path/to/media \
  --quality 85 \
  --crf 24 \
  --audio-bitrate 192k \
  --threshold 0.85 \
  --workers 8 \
  --dry-run \
  --verbose
```

### Parametri

- `--quality, -q`: Qualità JPEG (1-100, default: 80)
- `--crf, -c`: CRF video (0-51, default: 26, più basso = migliore qualità)
- `--audio-bitrate, -a`: Bitrate audio (default: "128k")
- `--threshold, -t`: Soglia dimensione (0.0-1.0, default: 0.9)
- `--workers, -w`: Worker paralleli (default: 4)
- `--dry-run`: Simula senza modificare file
- `--verbose, -v`: Logging dettagliato

## Gestione Stato

Il tool mantiene uno stato per directory usando file JSON in `~/.media-optimizer/`:

```
~/.media-optimizer/
├── processed_files_abc123def456.json  # Hash della directory 1
└── processed_files_789xyz456abc.json  # Hash della directory 2
```

Ogni file contiene:
```json
{
  "processed_files": {
    "/path/to/file.jpg": {
      "path": "/path/to/file.jpg",
      "modified_time": 1642680000,
      "original_size": 1048576,
      "optimized_size": 524288,
      "reduction_percent": 50.0,
      "processed_at": 1642680000
    }
  }
}
```

## Testing

```bash
# Test unitari
cargo test

# Test con output
cargo test -- --nocapture

# Test specifici
cargo test config::tests
```

## Performance

- **Elaborazione parallela**: Configura workers in base al tuo hardware
- **Memory footprint basso**: Stream processing senza caricare file interi
- **I/O ottimizzato**: Operazioni asincrone con tokio
- **State efficiente**: Hash-based tracking evita scan completi

## Altri punti forti di Rust

1. **Type Safety**: Rust previene molti errori a compile-time
2. **Gestione Errori**: Robusta con `Result<T, E>` e error propagation
3. **Performance**: Elaborazione parallela nativa e memory-safe
4. **Manutenibilità**: Architettura modulare e ben documentata
5. **Cross-platform**: Funziona su Linux, macOS, Windows
6. **Testing**: Framework di testing integrato
7. **Dependencies**: Gestione dipendenze con Cargo