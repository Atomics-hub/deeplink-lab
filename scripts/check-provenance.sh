#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

cargo metadata --locked --format-version 1 >/dev/null
if rg -n 'uses:\s+[^[:space:]]+@(v[0-9]|main|master|stable)(\s|$)' .github/workflows; then
  echo "GitHub Actions must be pinned to full commit SHAs" >&2
  exit 1
fi
if ! command -v cargo-deny >/dev/null 2>&1; then
  echo "cargo-deny is required for the local license/advisory/source gate" >&2
  echo "install it with: cargo install cargo-deny --locked" >&2
  exit 1
fi
cargo deny check
