#!/bin/bash
#
# Immersive Player - macOS PKG Installer Build Script
#
# This script builds a signed macOS pkg installer for Immersive Player.
#
# Prerequisites:
#   - Xcode Command Line Tools (xcode-select --install)
#   - Developer ID Application certificate in Keychain
#   - Developer ID Installer certificate in Keychain
#   - FFmpeg static binaries in assets/ffmpeg/ (optional, will download if missing)
#
# Environment Variables:
#   DEVELOPER_ID_APPLICATION  - e.g., "Developer ID Application: Your Name (TEAMID)"
#   DEVELOPER_ID_INSTALLER    - e.g., "Developer ID Installer: Your Name (TEAMID)"
#   APPLE_ID                  - Apple ID for notarization (optional)
#   APPLE_PASSWORD            - App-specific password for notarization (optional)
#   APPLE_TEAM_ID             - Team ID for notarization (optional)
#   SKIP_NOTARIZATION         - Set to "1" to skip notarization
#
# Usage:
#   ./build-pkg.sh
#

set -e  # Exit on error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Script directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BUILD_DIR="$SCRIPT_DIR/build"
OUTPUT_DIR="$SCRIPT_DIR/output"

# App configuration
APP_NAME="Immersive Player"
APP_BUNDLE_NAME="ImmersivePlayer.app"
BUNDLE_ID="com.immersiveplayer.app"

# Get version from Cargo.toml
VERSION=$(grep '^version' "$PROJECT_DIR/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/')
echo -e "${BLUE}Building Immersive Player v${VERSION}${NC}"

# Architecture detection
ARCH=$(uname -m)
if [ "$ARCH" = "arm64" ]; then
    RUST_TARGET="aarch64-apple-darwin"
    FFMPEG_ARCH="arm64"
else
    RUST_TARGET="x86_64-apple-darwin"
    FFMPEG_ARCH="x64"
fi

echo -e "${BLUE}Target architecture: ${ARCH} (${RUST_TARGET})${NC}"

# Helper functions
log_step() {
    echo -e "\n${GREEN}==> $1${NC}"
}

log_warning() {
    echo -e "${YELLOW}Warning: $1${NC}"
}

log_error() {
    echo -e "${RED}Error: $1${NC}"
    exit 1
}

check_command() {
    if ! command -v "$1" &> /dev/null; then
        log_error "$1 is required but not installed."
    fi
}

# Check prerequisites
log_step "Checking prerequisites..."
check_command cargo
check_command codesign
check_command pkgbuild
check_command productbuild

# Check for signing certificates
if [ -z "$DEVELOPER_ID_APPLICATION" ]; then
    log_warning "DEVELOPER_ID_APPLICATION not set. Will attempt to find certificate automatically."
    DEVELOPER_ID_APPLICATION=$(security find-identity -v -p codesigning | grep "Developer ID Application" | head -1 | sed 's/.*"\(Developer ID Application[^"]*\)".*/\1/' || true)
    if [ -z "$DEVELOPER_ID_APPLICATION" ]; then
        log_error "No Developer ID Application certificate found. Please set DEVELOPER_ID_APPLICATION environment variable."
    fi
    echo "Found certificate: $DEVELOPER_ID_APPLICATION"
fi

if [ -z "$DEVELOPER_ID_INSTALLER" ]; then
    log_warning "DEVELOPER_ID_INSTALLER not set. Will attempt to find certificate automatically."
    DEVELOPER_ID_INSTALLER=$(security find-identity -v | grep "Developer ID Installer" | head -1 | sed 's/.*"\(Developer ID Installer[^"]*\)".*/\1/' || true)
    if [ -z "$DEVELOPER_ID_INSTALLER" ]; then
        log_error "No Developer ID Installer certificate found. Please set DEVELOPER_ID_INSTALLER environment variable."
    fi
    echo "Found certificate: $DEVELOPER_ID_INSTALLER"
fi

# Clean and create build directories
log_step "Preparing build directories..."
rm -rf "$BUILD_DIR"
rm -rf "$OUTPUT_DIR"
mkdir -p "$BUILD_DIR"
mkdir -p "$OUTPUT_DIR"

# Build the Rust application
log_step "Building release binary..."
cd "$PROJECT_DIR"
cargo build --release --target "$RUST_TARGET"

BINARY_PATH="$PROJECT_DIR/target/$RUST_TARGET/release/immersive-player"
if [ ! -f "$BINARY_PATH" ]; then
    # Fallback to non-target specific path
    BINARY_PATH="$PROJECT_DIR/target/release/immersive-player"
fi

if [ ! -f "$BINARY_PATH" ]; then
    log_error "Binary not found at expected path"
fi

echo "Binary built at: $BINARY_PATH"

# Create app bundle structure
log_step "Creating app bundle..."
APP_BUNDLE="$BUILD_DIR/$APP_BUNDLE_NAME"
mkdir -p "$APP_BUNDLE/Contents/MacOS"
mkdir -p "$APP_BUNDLE/Contents/Resources"
mkdir -p "$APP_BUNDLE/Contents/Resources/assets"
mkdir -p "$APP_BUNDLE/Contents/Resources/ffmpeg"
mkdir -p "$APP_BUNDLE/Contents/Frameworks"

# Copy Info.plist
cp "$SCRIPT_DIR/Info.plist" "$APP_BUNDLE/Contents/"

# Update version in Info.plist
/usr/libexec/PlistBuddy -c "Set :CFBundleShortVersionString $VERSION" "$APP_BUNDLE/Contents/Info.plist"

# Copy binary
cp "$BINARY_PATH" "$APP_BUNDLE/Contents/MacOS/"

# Copy assets
log_step "Copying assets..."
if [ -d "$PROJECT_DIR/assets" ]; then
    cp -R "$PROJECT_DIR/assets/"* "$APP_BUNDLE/Contents/Resources/assets/" 2>/dev/null || true
fi

# Copy/Download FFmpeg binaries
log_step "Setting up FFmpeg binaries..."
FFMPEG_DIR="$APP_BUNDLE/Contents/Resources/ffmpeg"

# Check if FFmpeg is already in assets
if [ -f "$PROJECT_DIR/assets/ffmpeg/ffmpeg" ]; then
    echo "Using bundled FFmpeg from assets/ffmpeg/"
    cp "$PROJECT_DIR/assets/ffmpeg/ffmpeg" "$FFMPEG_DIR/"
    cp "$PROJECT_DIR/assets/ffmpeg/ffprobe" "$FFMPEG_DIR/" 2>/dev/null || true
else
    # Try to find system FFmpeg and copy it
    SYSTEM_FFMPEG=$(which ffmpeg 2>/dev/null || true)
    if [ -n "$SYSTEM_FFMPEG" ] && [ -f "$SYSTEM_FFMPEG" ]; then
        echo "Copying system FFmpeg from: $SYSTEM_FFMPEG"
        cp "$SYSTEM_FFMPEG" "$FFMPEG_DIR/"
        
        SYSTEM_FFPROBE=$(which ffprobe 2>/dev/null || true)
        if [ -n "$SYSTEM_FFPROBE" ] && [ -f "$SYSTEM_FFPROBE" ]; then
            cp "$SYSTEM_FFPROBE" "$FFMPEG_DIR/"
        fi
    else
        log_warning "FFmpeg not found. The converter feature will require users to install FFmpeg separately."
        log_warning "To bundle FFmpeg, place static binaries in assets/ffmpeg/ or install via Homebrew."
    fi
fi

# Make FFmpeg binaries executable
chmod +x "$FFMPEG_DIR/"* 2>/dev/null || true

# Create icon placeholder if no icon exists
if [ ! -f "$APP_BUNDLE/Contents/Resources/AppIcon.icns" ]; then
    log_warning "No AppIcon.icns found. Using placeholder. Add your icon to installer/resources/AppIcon.icns"
    # Create a minimal placeholder - the app will still work without a custom icon
fi

# Code signing
log_step "Code signing..."

# Sign FFmpeg binaries first (if present)
if [ -f "$FFMPEG_DIR/ffmpeg" ]; then
    echo "Signing FFmpeg binaries..."
    codesign --force --options runtime --sign "$DEVELOPER_ID_APPLICATION" \
        --entitlements "$SCRIPT_DIR/entitlements.plist" \
        "$FFMPEG_DIR/ffmpeg"
    
    if [ -f "$FFMPEG_DIR/ffprobe" ]; then
        codesign --force --options runtime --sign "$DEVELOPER_ID_APPLICATION" \
            --entitlements "$SCRIPT_DIR/entitlements.plist" \
            "$FFMPEG_DIR/ffprobe"
    fi
fi

# Sign any frameworks/dylibs
find "$APP_BUNDLE/Contents/Frameworks" -name "*.dylib" -o -name "*.framework" 2>/dev/null | while read -r lib; do
    echo "Signing: $lib"
    codesign --force --options runtime --sign "$DEVELOPER_ID_APPLICATION" "$lib"
done

# Sign the main application bundle
echo "Signing application bundle..."
codesign --force --options runtime --sign "$DEVELOPER_ID_APPLICATION" \
    --entitlements "$SCRIPT_DIR/entitlements.plist" \
    --deep \
    "$APP_BUNDLE"

# Verify signature
echo "Verifying signature..."
codesign --verify --verbose=4 "$APP_BUNDLE"
spctl --assess --type exec --verbose=4 "$APP_BUNDLE" || log_warning "spctl assessment failed (may be expected without notarization)"

# Build component package
log_step "Building component package..."
COMPONENT_PKG="$BUILD_DIR/ImmersivePlayer.pkg"
pkgbuild \
    --root "$BUILD_DIR" \
    --install-location "/Applications" \
    --component-plist <(cat <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<array>
    <dict>
        <key>BundleIsRelocatable</key>
        <false/>
        <key>BundleIsVersionChecked</key>
        <true/>
        <key>BundleHasStrictIdentifier</key>
        <true/>
        <key>RootRelativeBundlePath</key>
        <string>$APP_BUNDLE_NAME</string>
    </dict>
</array>
</plist>
EOF
) \
    --identifier "$BUNDLE_ID" \
    --version "$VERSION" \
    "$COMPONENT_PKG"

# Create distribution package
log_step "Building distribution package..."
FINAL_PKG="$OUTPUT_DIR/ImmersivePlayer-${VERSION}.pkg"

# Copy resources for the installer
RESOURCES_DIR="$BUILD_DIR/resources"
mkdir -p "$RESOURCES_DIR"
cp "$SCRIPT_DIR/resources/"*.html "$RESOURCES_DIR/" 2>/dev/null || true

# Update distribution.xml with correct package reference
DIST_XML="$BUILD_DIR/distribution.xml"
sed "s|ImmersivePlayer.pkg|$COMPONENT_PKG|g" "$SCRIPT_DIR/distribution.xml" > "$DIST_XML"
sed -i '' "s|version=\"0.1.0\"|version=\"$VERSION\"|g" "$DIST_XML"

productbuild \
    --distribution "$DIST_XML" \
    --resources "$RESOURCES_DIR" \
    --package-path "$BUILD_DIR" \
    "$FINAL_PKG.unsigned"

# Sign the installer package
log_step "Signing installer package..."
productsign \
    --sign "$DEVELOPER_ID_INSTALLER" \
    "$FINAL_PKG.unsigned" \
    "$FINAL_PKG"

rm "$FINAL_PKG.unsigned"

# Verify package signature
echo "Verifying package signature..."
pkgutil --check-signature "$FINAL_PKG"

# Notarization (optional)
if [ "$SKIP_NOTARIZATION" != "1" ] && [ -n "$APPLE_ID" ] && [ -n "$APPLE_PASSWORD" ]; then
    log_step "Notarizing package..."
    
    TEAM_ID="${APPLE_TEAM_ID:-}"
    if [ -z "$TEAM_ID" ]; then
        # Try to extract team ID from certificate
        TEAM_ID=$(echo "$DEVELOPER_ID_APPLICATION" | sed 's/.*(\([^)]*\)).*/\1/')
    fi
    
    xcrun notarytool submit "$FINAL_PKG" \
        --apple-id "$APPLE_ID" \
        --password "$APPLE_PASSWORD" \
        --team-id "$TEAM_ID" \
        --wait
    
    log_step "Stapling notarization ticket..."
    xcrun stapler staple "$FINAL_PKG"
    
    echo "Verifying notarization..."
    spctl --assess --type install --verbose=4 "$FINAL_PKG"
else
    if [ "$SKIP_NOTARIZATION" = "1" ]; then
        log_warning "Notarization skipped (SKIP_NOTARIZATION=1)"
    else
        log_warning "Notarization skipped. Set APPLE_ID and APPLE_PASSWORD environment variables to enable."
    fi
fi

# Summary
log_step "Build complete!"
echo ""
echo -e "${GREEN}Package created:${NC} $FINAL_PKG"
echo -e "${GREEN}Version:${NC} $VERSION"
echo -e "${GREEN}Architecture:${NC} $ARCH"
echo ""
ls -lh "$FINAL_PKG"
echo ""

# Cleanup option
if [ "$KEEP_BUILD" != "1" ]; then
    echo "Cleaning up build directory..."
    rm -rf "$BUILD_DIR"
fi

echo -e "${GREEN}Done!${NC}"


