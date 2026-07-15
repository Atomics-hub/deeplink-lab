#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SDK="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}"
PLATFORM="$(find "$SDK/platforms" -maxdepth 1 -type d -name 'android-*' | sort -V | tail -n 1)"
TOOLS="$(find "$SDK/build-tools" -maxdepth 1 -type d | sort -V | tail -n 1)"
MANIFEST="${DEEPLINKLAB_MANIFEST:-$ROOT/AndroidManifest.xml}"
SOURCE="${DEEPLINKLAB_SOURCE:-$ROOT/src/dev/deeplinklab/fixture/MainActivity.java}"
BUILD="${DEEPLINKLAB_BUILD:-$ROOT/build}"
APK_BASENAME="${DEEPLINKLAB_APK_BASENAME:-DeepLinkFixture.apk}"
DNAME="${DEEPLINKLAB_DNAME:-CN=DeepLink Lab Fixture,OU=Testing,O=Fixture,L=Local,ST=None,C=US}"
CLASSES="$BUILD/classes"
DEX="$BUILD/dex"
UNALIGNED="$BUILD/fixture-unaligned.apk"
ALIGNED="$BUILD/fixture-aligned.apk"
APK="$BUILD/$APK_BASENAME"
KEYSTORE="$BUILD/fixture-only.keystore"

if [[ ! -f "$PLATFORM/android.jar" || ! -x "$TOOLS/aapt2" ]]; then
  echo "Android platform and build tools are required under $SDK" >&2
  exit 1
fi

rm -rf "$BUILD"
mkdir -p "$CLASSES" "$DEX"

javac -source 8 -target 8 -Xlint:-options \
  -classpath "$PLATFORM/android.jar" \
  -d "$CLASSES" \
  "$SOURCE"

find "$CLASSES" -name '*.class' -print0 | xargs -0 "$TOOLS/d8" --lib "$PLATFORM/android.jar" --output "$DEX"

"$TOOLS/aapt2" link \
  -I "$PLATFORM/android.jar" \
  --manifest "$MANIFEST" \
  --min-sdk-version 23 \
  --target-sdk-version 36 \
  -o "$UNALIGNED"

(cd "$DEX" && zip -q -u "$UNALIGNED" classes.dex)
"$TOOLS/zipalign" -f 4 "$UNALIGNED" "$ALIGNED"

keytool -genkeypair -noprompt \
  -keystore "$KEYSTORE" \
  -storepass fixture-only \
  -keypass fixture-only \
  -alias fixture \
  -keyalg RSA \
  -keysize 2048 \
  -validity 30 \
  -dname "$DNAME" >/dev/null 2>&1

"$TOOLS/apksigner" sign \
  --ks "$KEYSTORE" \
  --ks-pass pass:fixture-only \
  --key-pass pass:fixture-only \
  --out "$APK" \
  "$ALIGNED"
"$TOOLS/apksigner" verify --verbose "$APK"
echo "$APK"
