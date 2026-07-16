#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "error: native npm artifact checks require macOS" >&2
  exit 1
fi

scripts/check-npm-metadata.sh

BINARY="$ROOT/dist/npm/deeplink-lab"
CHECKSUM="$ROOT/dist/npm/deeplink-lab.sha256"
[[ -x "$BINARY" ]] || { echo "error: missing executable $BINARY" >&2; exit 1; }
[[ -f "$CHECKSUM" ]] || { echo "error: missing checksum $CHECKSUM" >&2; exit 1; }

xcrun lipo "$BINARY" -verify_arch arm64 x86_64
codesign --verify --strict "$BINARY"
(
  cd "$(dirname "$BINARY")"
  shasum -a 256 -c "$(basename "$CHECKSUM")"
)

SIZE="$(stat -f '%z' "$BINARY")"
if (( SIZE > 20 * 1024 * 1024 )); then
  echo "error: npm binary is unexpectedly large: $SIZE bytes" >&2
  exit 1
fi

EXPECTED="deeplink-lab $(jq -er '.version' package.json)"
[[ "$("$BINARY" --version)" == "$EXPECTED" ]]

if [[ "$(uname -m)" == "arm64" ]] && arch -x86_64 /usr/bin/true >/dev/null 2>&1; then
  [[ "$(arch -x86_64 "$BINARY" --version)" == "$EXPECTED" ]]
fi

if strings "$BINARY" | LC_ALL=C grep -Eq '/Users/|/home/[[:alnum:]_.-]+/|Documents/Codex'; then
  echo "error: distributable binary contains a private absolute build path" >&2
  exit 1
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
"$BINARY" validate --spec deeplinklab.yml --format json > "$TMP/preflight.json"
jq -e '.valid == true and .errors == 0' "$TMP/preflight.json" >/dev/null

echo "npm binary: universal, signed, checksummed, and executable ($SIZE bytes)"
