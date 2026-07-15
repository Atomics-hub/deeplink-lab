#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
mkdir -p work

fixtures/ios/build.sh
fixtures/android/build.sh
DEVICES="$(scripts/start-reference-devices.sh)"
IOS_DEVICE="$(printf '%s\n' "$DEVICES" | awk -F= '/^IOS_DEVICE=/{print $2}')"
ANDROID_DEVICE="$(printf '%s\n' "$DEVICES" | awk -F= '/^ANDROID_DEVICE=/{print $2}')"

rm -rf work/proof-reference work/proof-repeatability
set +e
cargo run --quiet -- run --spec deeplinklab.yml \
  --ios-device "$IOS_DEVICE" --android-device "$ANDROID_DEVICE" \
  --out work/proof-reference
REFERENCE_EXIT=$?
cargo run --quiet -- run --spec deeplinklab.yml \
  --ios-device "$IOS_DEVICE" --android-device "$ANDROID_DEVICE" \
  --case android-wrong-destination \
  --case android-query-loss \
  --case android-fragment-loss \
  --case android-route-collision \
  --case android-warm-stale \
  --case android-background-ignored \
  --repeat 20 --reset-between-runs true \
  --skip-html \
  --out work/proof-repeatability
REPEAT_EXIT=$?
set -e

if [[ "$REFERENCE_EXIT" -ne 2 || "$REPEAT_EXIT" -ne 2 ]]; then
  echo "fixture runs must exit 2 because intentional failures were observed" >&2
  exit 1
fi

ANDROID_DEVICE="$ANDROID_DEVICE" scripts/prove-integration.sh >/dev/null

set +e
cargo run --quiet -- gates --spec deeplinklab.yml \
  --representative work/proof-reference/report.json \
  --repeatability work/proof-repeatability/report.json \
  --format json --output work/proof-gates.json
GATE_EXIT=$?
set -e
if [[ "$GATE_EXIT" -ne 3 && "$GATE_EXIT" -ne 0 ]]; then
  echo "unexpected gate evaluator exit: $GATE_EXIT" >&2
  exit 1
fi

jq '{summary, generatedAt, runId, referenceSetup}' work/proof-reference/report.json
jq . work/proof-gates.json
