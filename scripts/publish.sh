#!/bin/bash
set -e

# ─── Shelly Publish Script ───────────────────────────────────────────
# Builds, signs, and creates a GitHub release with auto-update support.
#
# Prerequisites:
#   1. Generate signing keys (one-time):
#      cargo tauri signer generate --password "your-password" -w ~/.tauri-keys/shelly.key
#
#   2. Set environment variables:
#      export TAURI_SIGNING_PRIVATE_KEY=$(cat ~/.tauri-keys/shelly.key)
#      export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="your-password"
#
#   3. Update the pubkey in tauri.conf.json with the public key from step 1
#
#   4. For macOS code signing + notarization:
#      export APPLE_SIGNING_IDENTITY="Developer ID Application: Your Name (TEAMID)"
#      export APPLE_ID="your@email.com"
#      export APPLE_PASSWORD="app-specific-password"
#      export APPLE_TEAM_ID="YOUR_TEAM_ID"
#
#   5. Install GitHub CLI: brew install gh
#
# Usage:
#   ./scripts/publish.sh          # Uses version from tauri.conf.json
#   ./scripts/publish.sh 1.2.0    # Override version
# ──────────────────────────────────────────────────────────────────────

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
TAURI_CONF="$PROJECT_DIR/src-tauri/tauri.conf.json"
SECRETS_FILE="/Users/johnny/Desktop/Files/secrets/apple_id_password.txt"
P12_FILE="/Users/johnny/Desktop/Files/secrets/cert_p12.p12"
BUILD_KEYCHAIN="$HOME/Library/Keychains/shelly-build.keychain-db"
TAURI_PRIVATE_KEY_PATH="${TAURI_PRIVATE_KEY_PATH:-$HOME/.tauri-keys/shelly.key}"

load_release_env() {
  if [ -f "$SECRETS_FILE" ]; then
    set -a
    # shellcheck disable=SC1090
    source "$SECRETS_FILE"
    set +a
  fi

  if [ -n "$APPLE_ID_PASSWORD" ] && [ -z "$APPLE_PASSWORD" ]; then
    export APPLE_PASSWORD="$APPLE_ID_PASSWORD"
  fi

  if [ -z "$TAURI_SIGNING_PRIVATE_KEY" ] && [ -f "$TAURI_PRIVATE_KEY_PATH" ]; then
    export TAURI_SIGNING_PRIVATE_KEY="$(cat "$TAURI_PRIVATE_KEY_PATH")"
  fi

  if [ -z "$TAURI_SIGNING_PRIVATE_KEY_PASSWORD" ] && [ -n "${TAURI_PRIVATE_KEY_PASSWORD:-}" ]; then
    export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$TAURI_PRIVATE_KEY_PASSWORD"
  fi

  if [ -z "$TAURI_SIGNING_PRIVATE_KEY_PASSWORD" ]; then
    export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""
  fi
}

require_env() {
  local name="$1"
  if [ -z "${!name}" ]; then
    echo "ERROR: $name is not set."
    exit 1
  fi
}

setup_build_keychain() {
  if [ ! -f "$P12_FILE" ]; then
    echo "ERROR: Developer ID certificate export not found at $P12_FILE"
    exit 1
  fi

  security delete-keychain "$BUILD_KEYCHAIN" 2>/dev/null || true
  security create-keychain -p '' "$BUILD_KEYCHAIN"
  security unlock-keychain -p '' "$BUILD_KEYCHAIN"
  security import "$P12_FILE" -k "$BUILD_KEYCHAIN" -P '' -A -T /usr/bin/codesign -T /usr/bin/security >/dev/null
  security list-keychains -d user -s "$BUILD_KEYCHAIN" "$HOME/Library/Keychains/login.keychain-db"
  security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k '' "$BUILD_KEYCHAIN" >/dev/null
}

verify_release_signing() {
  local app="$1"
  local dmg="$2"

  echo "Verifying signed artifacts..."
  codesign --verify --deep --strict --verbose=2 "$app"
  spctl -a -vvv --type exec "$app"
  codesign -dv --verbose=4 "$dmg" >/dev/null 2>&1
  spctl -a -vvv --type open "$dmg"
}

load_release_env

# Get version
if [ -n "$1" ]; then
  VERSION="$1"
  # Update version in tauri.conf.json
  sed -i '' "s/\"version\": \".*\"/\"version\": \"$VERSION\"/" "$TAURI_CONF"
  # Update Cargo.toml version
  sed -i '' "s/^version = \".*\"/version = \"$VERSION\"/" "$PROJECT_DIR/src-tauri/Cargo.toml"
  # Update package.json version
  sed -i '' "s/\"version\": \".*\"/\"version\": \"$VERSION\"/" "$PROJECT_DIR/package.json"
  echo "Version bumped to $VERSION"
else
  VERSION=$(grep '"version"' "$TAURI_CONF" | head -1 | sed 's/.*: "\(.*\)".*/\1/')
fi

echo "Publishing Shelly v$VERSION"
echo "──────────────────────────────"

# Verify signing + notarization env
require_env TAURI_SIGNING_PRIVATE_KEY
require_env APPLE_SIGNING_IDENTITY
require_env APPLE_ID
require_env APPLE_PASSWORD
require_env APPLE_TEAM_ID

setup_build_keychain
if ! security find-identity -v -p codesigning "$BUILD_KEYCHAIN" | grep -F "$APPLE_SIGNING_IDENTITY" >/dev/null; then
  echo "ERROR: Expected signing identity not available: $APPLE_SIGNING_IDENTITY"
  exit 1
fi

# Build frontend
echo "Building frontend..."
cd "$PROJECT_DIR"
npm run build:frontend

# Build Tauri app (produces .dmg, .app, .app.tar.gz, .app.tar.gz.sig)
echo "Building Tauri app..."
cargo tauri build

# Find build artifacts
BUNDLE_DIR="$PROJECT_DIR/src-tauri/target/release/bundle"
DMG=$(find "$BUNDLE_DIR/dmg" -name "*.dmg" | head -1)
APP=$(find "$BUNDLE_DIR/macos" -name "*.app" -maxdepth 1 | head -1)
APP_TAR_GZ=$(find "$BUNDLE_DIR/macos" -name "*.app.tar.gz" | head -1)
APP_TAR_GZ_SIG=$(find "$BUNDLE_DIR/macos" -name "*.app.tar.gz.sig" | head -1)

if [ -z "$DMG" ] || [ -z "$APP" ] || [ -z "$APP_TAR_GZ" ] || [ -z "$APP_TAR_GZ_SIG" ]; then
  echo "ERROR: Build artifacts not found."
  echo "  APP: $APP"
  echo "  DMG: $DMG"
  echo "  TAR.GZ: $APP_TAR_GZ"
  echo "  SIG: $APP_TAR_GZ_SIG"
  exit 1
fi

echo "Build artifacts:"
echo "  DMG: $DMG"
echo "  TAR.GZ: $APP_TAR_GZ"
echo "  SIG: $APP_TAR_GZ_SIG"

verify_release_signing "$APP" "$DMG"

# Read signature
SIGNATURE=$(cat "$APP_TAR_GZ_SIG")

# Detect architecture
ARCH=$(uname -m)
if [ "$ARCH" = "arm64" ]; then
  PLATFORM="darwin-aarch64"
else
  PLATFORM="darwin-x86_64"
fi

# Generate latest.json for the updater
LATEST_JSON="$BUNDLE_DIR/latest.json"
cat > "$LATEST_JSON" << EOF
{
  "version": "$VERSION",
  "notes": "Shelly v$VERSION",
  "pub_date": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "platforms": {
    "$PLATFORM": {
      "signature": "$SIGNATURE",
      "url": "https://github.com/aiwhiteteam/shelly/releases/download/v$VERSION/Shelly.app.tar.gz"
    }
  }
}
EOF

echo "Generated latest.json"

# Create GitHub release
echo "Creating GitHub release v$VERSION..."
gh release create "v$VERSION" \
  --title "Shelly v$VERSION" \
  --notes "## Shelly v$VERSION

### Download
- **macOS**: Download the DMG below and drag Shelly to Applications.

### Auto-Update
Existing installations will update automatically." \
  "$DMG" \
  "$APP_TAR_GZ" \
  "$APP_TAR_GZ_SIG" \
  "$LATEST_JSON"

echo ""
echo "Published Shelly v$VERSION"
echo "Release: https://github.com/aiwhiteteam/shelly/releases/tag/v$VERSION"
echo ""
echo "IMPORTANT: If you build for both architectures, update latest.json"
echo "to include both darwin-aarch64 and darwin-x86_64 platform entries."
