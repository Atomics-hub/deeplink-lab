#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "error: the npm artifact is a macOS universal binary and must be built on macOS" >&2
  exit 1
fi

TOOLCHAIN="${RUST_TOOLCHAIN:-1.88.0}"
TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target/npm}"
OUT_DIR="$ROOT/dist/npm"

for command in cargo rustup xcrun codesign shasum; do
  if ! command -v "$command" >/dev/null 2>&1; then
    echo "error: required command not found: $command" >&2
    exit 1
  fi
done

for target in aarch64-apple-darwin x86_64-apple-darwin; do
  if ! rustup target list --installed --toolchain "$TOOLCHAIN" | grep -Fxq "$target"; then
    echo "error: Rust target $target is not installed for $TOOLCHAIN" >&2
    echo "run: rustup target add --toolchain $TOOLCHAIN $target" >&2
    exit 1
  fi
done

rm -rf "$OUT_DIR"
mkdir -p "$OUT_DIR"

# Keep workstation and CI paths out of the distributable binary.
REMAP_FLAGS="--remap-path-prefix=$ROOT=deeplink-lab --remap-path-prefix=$HOME=.home"
export RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }$REMAP_FLAGS"

for target in aarch64-apple-darwin x86_64-apple-darwin; do
  cargo "+$TOOLCHAIN" build --locked --release --target "$target" --target-dir "$TARGET_DIR"
done

xcrun lipo -create \
  "$TARGET_DIR/aarch64-apple-darwin/release/deeplink-lab" \
  "$TARGET_DIR/x86_64-apple-darwin/release/deeplink-lab" \
  -output "$OUT_DIR/deeplink-lab"
chmod 0755 "$OUT_DIR/deeplink-lab"

# An ad-hoc signature makes the artifact structurally valid without claiming a
# Developer ID identity or requiring access to an Apple developer account.
codesign --force --sign - --identifier dev.deeplinklab.cli --timestamp=none "$OUT_DIR/deeplink-lab"

(
  cd "$OUT_DIR"
  shasum -a 256 deeplink-lab > deeplink-lab.sha256
)

xcrun lipo "$OUT_DIR/deeplink-lab" -verify_arch arm64 x86_64
codesign --verify --strict "$OUT_DIR/deeplink-lab"
"$OUT_DIR/deeplink-lab" --version

echo "built: dist/npm/deeplink-lab ($(du -h "$OUT_DIR/deeplink-lab" | awk '{print $1}'))"
