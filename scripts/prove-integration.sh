#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEVICE="${ANDROID_DEVICE:-${1:-}}"
OUT="$ROOT/work/integration-proof"

if [[ -z "$DEVICE" ]]; then
  echo "usage: ANDROID_DEVICE=emulator-5554 scripts/prove-integration.sh" >&2
  exit 1
fi

cd "$ROOT"
STARTED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
START_SECONDS="$SECONDS"
RUNTIME_BEFORE="$(shasum -a 256 src/runtime.rs | awk '{print $1}')"

fixtures/android-candidate/build.sh >/dev/null
cargo build --locked >/dev/null
target/debug/deeplink-lab validate --spec examples/integration-candidate.yml --strict >/dev/null
rm -rf "$OUT"
target/debug/deeplink-lab run \
  --spec examples/integration-candidate.yml \
  --case candidate-product-cold \
  --android-device "$DEVICE" \
  --out "$OUT" >/dev/null

RUNTIME_AFTER="$(shasum -a 256 src/runtime.rs | awk '{print $1}')"
DURATION_SECONDS="$((SECONDS - START_SECONDS))"
CLASSIFICATION="$(jq -r '.results[0].classification' "$OUT/report.json")"
ACTUAL_DESTINATION="$(jq -r '.results[0].actual.destination' "$OUT/report.json")"

if [[ "$RUNTIME_BEFORE" != "$RUNTIME_AFTER" ]]; then
  echo "runner code changed during the integration exercise" >&2
  exit 1
fi
if [[ "$CLASSIFICATION" != "passed" || "$ACTUAL_DESTINATION" != "product/7" ]]; then
  echo "candidate app did not reach its declared destination" >&2
  exit 1
fi

jq -n \
  --arg startedAt "$STARTED_AT" \
  --argjson durationSeconds "$DURATION_SECONDS" \
  --arg spec "examples/integration-candidate.yml" \
  --arg caseId "candidate-product-cold" \
  --arg classification "$CLASSIFICATION" \
  --arg actualDestination "$ACTUAL_DESTINATION" \
  --arg runtimeSha256 "$RUNTIME_AFTER" \
  '{
    schemaVersion: 1,
    startedAt: $startedAt,
    durationSeconds: $durationSeconds,
    spec: $spec,
    caseId: $caseId,
    classification: $classification,
    actualDestination: $actualDestination,
    runnerCodeChanged: false,
    runtimeSha256: $runtimeSha256,
    report: "report.json"
  }' >"$OUT/integration-evidence.json"

jq . "$OUT/integration-evidence.json"
