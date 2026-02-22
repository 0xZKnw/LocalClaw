#!/bin/bash
# Build script for macOS with Metal GPU support
# Run this on your Mac

set -e

echo "==================================================="
echo "  Building ClawRS for macOS with Metal GPU support  "
echo "==================================================="
echo ""

# Detect architecture
ARCH=$(uname -m)
echo "Detected architecture: $ARCH"

# Install prerequisites if needed:
# brew install cmake rust openssl pkg-config

# Determine the Rust target
if [ "$ARCH" = "arm64" ]; then
    TARGET="aarch64-apple-darwin"
    echo "Building for Apple Silicon (arm64)..."
elif [ "$ARCH" = "x86_64" ]; then
    TARGET="x86_64-apple-darwin"
    echo "Building for Intel Mac (x86_64)..."
else
    echo "Unknown architecture: $ARCH"
    exit 1
fi

# Ensure the target is installed
rustup target add "$TARGET" 2>/dev/null || true

# Build with Metal support (uses llama.cpp Metal backend for GPU acceleration)
echo ""
echo "Building release with Metal GPU support..."
cargo build --release --target "$TARGET" --features metal

echo ""
echo "==================================================="
echo "  Build complete!"
echo "==================================================="
echo ""
echo "Executable: target/$TARGET/release/clawrs"
echo ""
echo "To run:"
echo "  ./target/$TARGET/release/clawrs"
echo ""
echo "To create a distributable .app bundle:"
echo "  cargo install cargo-bundle"
echo "  cargo bundle --release --target $TARGET --features metal"
