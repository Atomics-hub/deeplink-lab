#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

for command in cargo jq; do
  if ! command -v "$command" >/dev/null 2>&1; then
    echo "error: required command not found: $command" >&2
    exit 1
  fi
done

PACKAGE_VERSION="$(jq -er '.version' package.json)"
CARGO_VERSION="$(cargo metadata --no-deps --format-version 1 | jq -er '.packages[] | select(.name == "deeplink-lab") | .version')"
[[ "$PACKAGE_VERSION" == "$CARGO_VERSION" ]] || {
  echo "error: package.json version $PACKAGE_VERSION differs from Cargo.toml version $CARGO_VERSION" >&2
  exit 1
}

jq -e '
  .name == "deeplink-lab" and
  .license == "Apache-2.0" and
  .repository.url == "git+https://github.com/Atomics-hub/deeplink-lab.git" and
  .homepage == "https://github.com/Atomics-hub/deeplink-lab#readme" and
  .bin == {"deeplink-lab": "dist/npm/deeplink-lab"} and
  .os == ["darwin"] and
  (.cpu | sort) == ["arm64", "x64"] and
  (.files | sort) == ["NOTICE", "dist/npm/deeplink-lab", "dist/npm/deeplink-lab.sha256"] and
  .publishConfig == {"access": "public", "registry": "https://registry.npmjs.org/"} and
  (.keywords | index("deep-link")) and
  (.keywords | index("universal-links")) and
  (.keywords | index("android-app-links")) and
  (.keywords | index("aasa")) and
  (.keywords | index("assetlinks")) and
  (.scripts.preinstall == null) and
  (.scripts.install == null) and
  (.scripts.postinstall == null)
' package.json >/dev/null

git check-ignore -q dist/npm/deeplink-lab
if git ls-files --error-unmatch dist/npm/deeplink-lab >/dev/null 2>&1; then
  echo "error: generated npm binary must not be committed" >&2
  exit 1
fi

rg -q 'npm install --global deeplink-lab' README.md
rg -q 'macOS' README.md
rg -q 'no post-install download' README.md

echo "npm metadata: deeplink-lab@$PACKAGE_VERSION (macOS arm64/x64)"
