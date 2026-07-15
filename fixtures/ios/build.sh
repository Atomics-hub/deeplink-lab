#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BUILD="$ROOT/build"
APP="$BUILD/DeepLinkFixture.app"
SDK="$(xcrun --sdk iphonesimulator --show-sdk-path)"
ARCH="$(uname -m)"

rm -rf "$BUILD"
mkdir -p "$APP"
cp "$ROOT/Info.plist" "$APP/Info.plist"

xcrun swiftc \
  -sdk "$SDK" \
  -target "$ARCH-apple-ios18.0-simulator" \
  -parse-as-library \
  -framework UIKit \
  "$ROOT/AppDelegate.swift" \
  -o "$APP/DeepLinkFixture"

# The source entitlement is validated by the static fixture. Embedding the
# associated-domains entitlement in an ad-hoc Simulator signature is rejected
# by newer simulator runtimes because no Apple development team granted it.
codesign --force --sign - "$APP"
echo "$APP"
