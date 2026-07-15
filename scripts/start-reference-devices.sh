#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SDK="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}"
ADB="$SDK/platform-tools/adb"
EMULATOR="$SDK/emulator/emulator"
IOS_NAME="DeepLink Lab iPhone 17e"
ANDROID_NAME="DeepLinkLab_API_36"

if ! IOS_ID="$(xcrun simctl list devices available -j | jq -r --arg name "$IOS_NAME" '.devices[][] | select(.name == $name) | .udid' | head -n 1)" || [[ -z "$IOS_ID" ]]; then
  IOS_ID="$(xcrun simctl create "$IOS_NAME" com.apple.CoreSimulator.SimDeviceType.iPhone-17e com.apple.CoreSimulator.SimRuntime.iOS-26-5)"
fi
xcrun simctl boot "$IOS_ID" 2>/dev/null || true
xcrun simctl bootstatus "$IOS_ID" -b

if ! "$EMULATOR" -list-avds | grep -Fxq "$ANDROID_NAME"; then
  AVDMANAGER="$(find "$SDK/cmdline-tools" -type f -name avdmanager | head -n 1)"
  IMAGE="$(find "$SDK/system-images" -mindepth 3 -maxdepth 3 -type d -name arm64-v8a | sort -V | tail -n 1)"
  if [[ -z "$AVDMANAGER" || -z "$IMAGE" ]]; then
    echo "an installed Android arm64 system image and avdmanager are required" >&2
    exit 1
  fi
  PACKAGE="system-images;$(basename "$(dirname "$(dirname "$IMAGE")")");$(basename "$(dirname "$IMAGE")");$(basename "$IMAGE")"
  printf 'no\n' | "$AVDMANAGER" create avd --force --name "$ANDROID_NAME" --package "$PACKAGE" --device pixel_6
fi

if ! "$ADB" devices | grep -q '^emulator-'; then
  "$EMULATOR" -avd "$ANDROID_NAME" -wipe-data -no-snapshot -no-window -no-audio -gpu swiftshader_indirect >"$ROOT/work/android-emulator.log" 2>&1 &
fi
"$ADB" wait-for-device
ANDROID_ID="$("$ADB" devices | awk '/^emulator-/{print $1; exit}')"
until [[ "$("$ADB" -s "$ANDROID_ID" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" == "1" ]]; do
  sleep 2
done
"$ADB" -s "$ANDROID_ID" shell wm size 720x1600 >/dev/null
"$ADB" -s "$ANDROID_ID" shell wm density 320 >/dev/null

printf 'IOS_DEVICE=%s\nANDROID_DEVICE=%s\n' "$IOS_ID" "$ANDROID_ID"
