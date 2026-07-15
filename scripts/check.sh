#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo run --quiet -- validate --spec deeplinklab.yml --strict

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
cargo run --quiet -- schema spec --output "$TMP/deeplinklab.schema.json"
cargo run --quiet -- schema run-report --output "$TMP/run-report.schema.json"
cargo run --quiet -- schema gate-report --output "$TMP/gate-report.schema.json"
diff -u schemas/deeplinklab.schema.json "$TMP/deeplinklab.schema.json"
diff -u schemas/run-report.schema.json "$TMP/run-report.schema.json"
diff -u schemas/gate-report.schema.json "$TMP/gate-report.schema.json"

scripts/check-public.sh
bash -n fixtures/ios/build.sh fixtures/android/build.sh scripts/*.sh
