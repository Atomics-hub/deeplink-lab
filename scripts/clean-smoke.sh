#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

CARGO_TARGET_DIR="$ROOT/target/clean-smoke" \
  cargo install --locked --path "$ROOT" --root "$TMP/install"
BIN="$TMP/install/bin/deeplink-lab"

"$BIN" schema spec >"$TMP/spec.schema.json"
"$BIN" doctor --format json >"$TMP/doctor.json"
"$BIN" validate --spec "$ROOT/deeplinklab.yml" --format json >"$TMP/preflight.json"
(cd "$TMP" && "$BIN" init --output starter.yml)
set +e
"$BIN" validate --spec "$TMP/starter.yml" --format json >"$TMP/starter-preflight.json" 2>/dev/null
STARTER_STATUS=$?
set -e
if [[ "$STARTER_STATUS" -eq 0 ]]; then
  echo "starter unexpectedly passed without the user's app artifacts" >&2
  exit 1
fi

jq -e '.valid == true' "$TMP/preflight.json" >/dev/null
jq -e '.staticPreflight == true' "$TMP/doctor.json" >/dev/null
jq -e '.title == "LabSpec"' "$TMP/spec.schema.json" >/dev/null
jq -e '.valid == false' "$TMP/starter-preflight.json" >/dev/null
echo "clean install and README command smoke passed"
