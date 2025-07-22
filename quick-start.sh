#!/bin/bash

# Quick start script for Space Media Optimizer

set -e

echo "🚀 Space Media Optimizer - Quick Start"
echo "======================================"

# Check if binary exists
if [ ! -f "target/release/media-optimizer" ]; then
    echo "❌ Binary not found. Building first..."
    cargo build --release
    echo "✅ Build complete!"
fi

# Show help if no arguments
if [ $# -eq 0 ]; then
    echo ""
    echo "Usage: $0 <media-directory> [options]"
    echo ""
    echo "Examples:"
    echo "  $0 /path/to/photos"
    echo "  $0 /path/to/videos --dry-run"
    echo "  $0 /path/to/media --quality 85 --workers 8"
    echo ""
    echo "Available options:"
    ./target/release/media-optimizer --help
    exit 0
fi

# Run the optimizer
echo "🔍 Starting optimization..."
echo "Directory: $1"
echo ""

./target/release/media-optimizer "$@"

echo ""
echo "✅ Optimization complete!"
echo "📊 Check ~/.media-optimizer/ for processing state files"
