#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

for command in jq npm tar; do
  if ! command -v "$command" >/dev/null 2>&1; then
    echo "error: required command not found: $command" >&2
    exit 1
  fi
done

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

TARBALL_NAME="$(npm pack --silent --pack-destination "$TMP" | tail -n 1)"
TARBALL="$TMP/$TARBALL_NAME"
[[ -f "$TARBALL" ]] || { echo "error: npm pack did not create $TARBALL" >&2; exit 1; }

tar -tzf "$TARBALL" | LC_ALL=C sort > "$TMP/contents.txt"
cat > "$TMP/expected.txt" <<'EOF'
package/LICENSE
package/NOTICE
package/README.md
package/dist/npm/deeplink-lab
package/dist/npm/deeplink-lab.sha256
package/package.json
EOF
diff -u "$TMP/expected.txt" "$TMP/contents.txt"

mkdir -p "$TMP/unpacked"
tar -xzf "$TARBALL" -C "$TMP/unpacked"
if rg -a -n '/Users/|/home/[[:alnum:]_.-]+/|Documents/Codex' "$TMP/unpacked/package"; then
  echo "error: npm tarball contains a private absolute path" >&2
  exit 1
fi

npm install --ignore-scripts --no-audit --no-fund --prefix "$TMP/install" "$TARBALL" >/dev/null
INSTALLED="$TMP/install/node_modules/.bin/deeplink-lab"
[[ -x "$INSTALLED" ]] || { echo "error: installed CLI shim is missing" >&2; exit 1; }
"$INSTALLED" --version
"$INSTALLED" validate --spec deeplinklab.yml --format json > "$TMP/installed-preflight.json"
jq -e '.valid == true and .errors == 0' "$TMP/installed-preflight.json" >/dev/null

echo "npm tarball: $TARBALL_NAME ($(stat -f '%z' "$TARBALL") bytes)"
shasum -a 256 "$TARBALL"
