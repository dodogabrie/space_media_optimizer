#!/bin/bash

# Build script for the Rust media optimizer

echo "🦀 Building Rust Media Optimizer..."

# Check if Rust is installed
if ! command -v cargo &> /dev/null; then
    echo "❌ Rust/Cargo is not installed. Please install Rust first:"
    echo "   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

# Check if required system dependencies are installed
echo "🔍 Checking system dependencies..."

missing_deps=()

if ! command -v ffmpeg &> /dev/null; then
    missing_deps+=("ffmpeg")
fi

if ! command -v exiftool &> /dev/null; then
    missing_deps+=("exiftool")
fi

if [ ${#missing_deps[@]} -ne 0 ]; then
    echo "❌ Missing required dependencies: ${missing_deps[*]}"
    echo "Please install them:"
    echo "  Ubuntu/Debian: sudo apt-get install ffmpeg libimage-exiftool-perl"
    echo "  macOS: brew install ffmpeg exiftool"
    echo "  Arch: sudo pacman -S ffmpeg perl-image-exiftool"
    exit 1
fi

echo "✅ All system dependencies found"

# Build the project
echo "🔨 Building optimized release binary..."
cargo build --release

if [ $? -eq 0 ]; then
    echo "✅ Build successful!"
    echo "📍 Binary location: target/release/media-optimizer"
    echo ""
    echo "🚀 Usage examples:"
    echo "  ./target/release/media-optimizer /path/to/media"
    echo "  ./target/release/media-optimizer --dry-run --verbose /path/to/media"
    echo "  ./target/release/media-optimizer --quality 85 --crf 23 /path/to/media"
else
    echo "❌ Build failed!"
    exit 1
fi
