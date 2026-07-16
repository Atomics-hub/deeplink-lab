# Reference proof status

This ledger records the sanitized local evidence committed before the initial public push. A proof gate can be `passed`, `failed`, or `unverified`; missing evidence never becomes a pass.

## Current status

| Gate | Status | Evidence |
| --- | --- | --- |
| Five runtime-only failures | Passed | 6 unique classes: `background_handler_ignored`, `fragment_loss`, `query_loss`, `route_collision`, `warm_state_stale`, and `wrong_destination` |
| 95% identical classification over 20 reset runs | Passed | 100.00% minimum per-case modal agreement across 6 failure-class cases × 20 runs; 120 failed as expected, 0 unavailable |
| 20 cases / two platforms / under 10 minutes | Passed | 20 cases, 2 platforms, 90.183 seconds; 8 passed controls, 12 expected runtime failures, 0 unavailable |
| New fixture integrated under 30 minutes without runner code | Passed | Distinct Android package reached `product/7` in 68 seconds; runner SHA-256 was identical before/after |
| Complete evidence on every runtime failure | Passed | 12/12 reference failures include actual destination, screenshot, logs, start state, replay, and artifact hashes |

The machine-evaluated result is [proof/gates.json](../proof/gates.json). The full reference run is [proof/reference/report.html](../proof/reference/report.html) plus its JSON and case files. Repeatability evidence is under `proof/repeatability/`; HTML was intentionally skipped for that large run and can be rebuilt with `deeplink-lab report`. The integration timing and unchanged runner hash are recorded in `proof/integration/integration-evidence.json`.

## Reference setup

- Apple M4 host, macOS 26.5.2;
- Xcode 26.6 with an iOS 26.5 Simulator;
- Android 16 / API 36 arm64 emulator, 720×1600 test viewport;
- Rust 1.93 for the captured run (the package supports Rust 1.88+).

The representative run started at `2026-07-16T03:42:07Z`. The repeatability run started at `2026-07-15T19:41:02Z` and took 782.198 seconds. The integration exercise started at `2026-07-15T20:47:59Z` and took 68 seconds. Durations are host measurements, not universal benchmarks.

## Reproduce

```bash
scripts/check.sh
scripts/prove-local.sh
```

`prove-local.sh` is destructive only to its dedicated fixture apps and `work/` proof directories. It requires already-installed Xcode and Android runtimes; it does not install SDKs or accept licenses.

## Claim ceiling

Even if every local technical gate passes, this remains Simulator/emulator evidence. Physical-device behavior, authenticated source applications, store fallback, and deferred attribution are outside the proof.
