#!/bin/bash
#
# Launch script for Immersive Player
# Run from the Immersiveserver directory
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$SCRIPT_DIR/immersive-player"

# Colors for output
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}🎬 Immersive Player${NC}"
echo "================================"

# Check if Rust is installed
if ! command -v cargo &> /dev/null; then
    echo -e "${YELLOW}Cargo not found. Please install Rust from https://rustup.rs/${NC}"
    exit 1
fi

cd "$PROJECT_DIR"

# Parse arguments
BUILD_MODE="debug"
if [[ "$1" == "--release" || "$1" == "-r" ]]; then
    BUILD_MODE="release"
    echo "Building in release mode..."
    cargo run --release
elif [[ "$1" == "--build" || "$1" == "-b" ]]; then
    echo "Building only (no run)..."
    cargo build
elif [[ "$1" == "--help" || "$1" == "-h" ]]; then
    echo ""
    echo "Usage: ./run.sh [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  -r, --release   Build and run in release mode (optimized)"
    echo "  -b, --build     Build only, don't run"
    echo "  -h, --help      Show this help message"
    echo ""
    echo "Without options, builds and runs in debug mode."
else
    echo "Building and running in debug mode..."
    cargo run
fi




