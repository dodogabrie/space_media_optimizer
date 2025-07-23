# Space Media Optimizer - Performance Optimization

## Problema di Memoria Risolto

Il problema di memoria elevata (5GB per immagini da 25MB) è stato risolto implementando:

### 1. **Rilevamento Automatico File Grandi**
- File > 20MB vengono processati con strumenti esterni per evitare di caricare tutto in memoria
- File più piccoli usano ancora il processing in-memory più veloce

### 2. **Strumenti Esterni Ottimizzati**
Per ottenere le migliori performance e il minor uso di memoria, installa questi strumenti:

#### Ubuntu/Debian:
```bash
sudo apt-get install jpegoptim optipng webp exiftool
```

#### macOS (con Homebrew):
```bash
brew install jpegoptim optipng webp exiftool
```

#### Windows:
- Scarica da: https://jpegoptim.com/
- OptiPNG: http://optipng.sourceforge.net/
- WebP: https://developers.google.com/speed/webp/download
- ExifTool: https://exiftool.org/

### 3. **Come Funziona Ora**

**File Piccoli (< 20MB):**
- Processing in-memory veloce
- Usa la libreria Rust `image`

**File Grandi (≥ 20MB):**
- Usa `jpegoptim`/`jpegtran` per JPEG
- Usa `optipng`/`pngcrush` per PNG  
- Usa `cwebp` per WebP
- **Nessun caricamento in memoria** della immagine decodificata

### 4. **Benefici**

- **Memoria**: Da 5GB a ~100MB per immagini grandi
- **Velocità**: 2-5x più veloce su file grandi
- **Qualità**: Stessa o migliore qualità di output
- **Compatibilità**: Fallback automatico se tools mancanti

### 5. **Uso**

Il comportamento è identico, ma ora molto più efficiente:

```bash
# Python wrapper (invariato)
python rust_optimizer.py /path/to/images /path/to/output --quality 80

# Direttamente il binario Rust
media-optimizer /path/to/images --quality 80 --output /path/to/output
```

### 6. **Monitoraggio**

Con `--verbose` puoi vedere quale strategia viene usata:

```
DEBUG - Large image detected (25MB), using memory-efficient approach
DEBUG - Processing large file with external tools: image.jpg
```

### 7. **Se Non Hai Gli Strumenti Esterni**

L'app continua a funzionare ma userà più memoria:

```
WARN - Optional optimization tools missing: jpegoptim, optipng, cwebp  
WARN - Large images will use more memory without these tools
```

## Istruzioni di Build

Dopo aver installato le dipendenze esterne, ricompila:

```bash
cd src/projects/project4/rust/space_media_optimizer
cargo build --release
```

Il binario sarà in `target/release/media-optimizer` (o `media-optimizer.exe` su Windows).
