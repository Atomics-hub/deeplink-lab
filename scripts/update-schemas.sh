#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mkdir -p "$ROOT/schemas"

cargo run --quiet --manifest-path "$ROOT/Cargo.toml" -- schema spec \
  --output "$ROOT/schemas/deeplinklab.schema.json"
cargo run --quiet --manifest-path "$ROOT/Cargo.toml" -- schema run-report \
  --output "$ROOT/schemas/run-report.schema.json"
cargo run --quiet --manifest-path "$ROOT/Cargo.toml" -- schema gate-report \
  --output "$ROOT/schemas/gate-report.schema.json"
