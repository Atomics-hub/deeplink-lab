# DeepLink Lab

**Test where a deep link lands—not merely whether its association files parse.**

[![CI](https://github.com/Atomics-hub/deeplink-lab/actions/workflows/ci.yml/badge.svg)](https://github.com/Atomics-hub/deeplink-lab/actions/workflows/ci.yml)
[![npm](https://img.shields.io/npm/v/deeplink-lab)](https://www.npmjs.com/package/deeplink-lab)
[![Security](https://img.shields.io/badge/security-policy-1769e0)](SECURITY.md)
[![License](https://img.shields.io/badge/license-Apache--2.0-1769e0)](LICENSE)

DeepLink Lab is a vendor-neutral contract and local evidence runner for iOS Universal Links, Android App Links, and custom schemes. One `deeplinklab.yml` connects static configuration to lifecycle-aware runtime cases, exact destination assertions, screenshots, relevant logs, and a one-command replay.

It orchestrates platform tools; it is not another device driver. Built-in local lanes use `xcrun simctl` and `adb`. Safari, Chrome, controlled-page taps, and generic visible-text assertions use an optional Maestro adapter.

The committed reference proof observed 20 cases across both local platforms in 90.183 seconds: 8 controls passed, 12 deliberately broken routes failed at the exact destination assertion, and none were unavailable. Six failure-class cases then produced identical classifications over 20 explicit-reset repetitions. These are local Simulator/emulator measurements, not physical-device claims; inspect the [proof ledger](docs/PROOF.md) and [self-contained report](proof/reference/report.html).

## The evidence boundary

| Layer | What DeepLink Lab can establish | What it cannot establish |
| --- | --- | --- |
| Static preflight | AASA, `assetlinks.json`, entitlements, manifests, domains, fingerprints, URL schemes, and common configuration contradictions | Runtime routing or cached OS behavior |
| Local simulated runtime | Observed destination on the selected iOS Simulator or Android emulator, lifecycle/source state, screenshot, logs, duration, and replay | Physical-device equivalence |
| Physical device | Not managed in v1; bring your own established Appium/Maestro/device workflow | Deferred store attribution or authenticated consumer-app behavior |

The report labels these layers separately. A green Simulator or emulator case is never described as real-device proof.

## Install

The npm package supports macOS on both Apple Silicon and Intel. It contains one universal native binary: there are no runtime npm dependencies and no post-install download.

```bash
npm install --global deeplink-lab
deeplink-lab --help
```

Or run it without keeping a global install:

```bash
npx deeplink-lab --help
```

To build from source instead, install Rust 1.88 or newer:

```bash
cargo install --path . --locked
```

## Five-minute static preflight

After either installation path:

```bash
deeplink-lab validate --spec deeplinklab.yml
deeplink-lab doctor
```

The repository fixture intentionally has runtime faults, but its association and app configuration is statically coherent. A deliberately broken static example is included:

```bash
deeplink-lab validate --spec examples/broken-static.yml
```

That command exits non-zero and names the contradictory AASA app ID, Android package, and malformed certificate fingerprint.

## Run the deterministic two-platform fixture

Build the native apps without Gradle, CocoaPods, or third-party mobile dependencies:

```bash
fixtures/ios/build.sh
fixtures/android/build.sh
```

Boot an iOS Simulator and Android emulator, then run the 20-case reference matrix:

```bash
deeplink-lab run \
  --spec deeplinklab.yml \
  --ios-device booted \
  --android-device emulator-5554 \
  --out outputs/reference
```

The fixture contains known-good routes and intentional app-level routing failures, so the command correctly exits `2` after writing `outputs/reference/report.json` and a portable `outputs/reference/report.html`. A failing fixture that exits zero would be fake evidence.

On the documented Mac, `scripts/start-reference-devices.sh` creates dedicated local devices and sets the Android viewport used by the committed timing. It downloads nothing and fails if the required runtimes are absent.

Reproduce one case with the exact same declarative runner:

```bash
deeplink-lab replay \
  --spec deeplinklab.yml \
  --case ios-wrong-destination \
  --ios-device booted
```

## The contract

```yaml
version: 1
apps:
  ios-app:
    platform: ios
    appId: com.example.shop
    artifact: build/Shop.app
    teamId: TEAMID1234
    entitlements: ios/Shop.entitlements
    infoPlist: ios/Info.plist
    probe:
      kind: maestro_visible_text
      template: "Destination: {destination}"
associations:
  - domain: links.example.com
    aasa: fixtures/apple-app-site-association
cases:
  - id: product-cold
    app: ios-app
    link: https://links.example.com/products/42
    expected: {target: app, destination: product/42}
    state:
      install: installed
      launch: cold
      verification: preserve
      userDefault: preserve
    source: {context: direct}
```

The checked-in [JSON Schema](schemas/deeplinklab.schema.json) rejects unknown keys. See [the specification reference](docs/SPEC.md) for all states, source contexts, probes, and boundaries.

## Runtime coverage

| Dimension | iOS Simulator | Android emulator |
| --- | --- | --- |
| Installed / uninstalled | Install/uninstall through `simctl`; unhandled links when observable | Install/uninstall through `adb`; unhandled links when observable |
| Cold / warm / background | Terminate/launch; background by launching Safari | Force-stop/launch; background through `KEYCODE_HOME` |
| Verification reset | Reinstall is only a local approximation; Apple CDN invalidation is not claimed | `pm set-app-links`, user-selection reset, and re-verification |
| Direct link | `simctl openurl` | `am start -a VIEW -c BROWSABLE` |
| Safari / Chrome / controlled page | Optional Maestro adapter | Optional Maestro adapter |
| Exact destination | App-data file, log regex, or Maestro visible text | Debug `run-as` file, log regex, or Maestro visible text |

An app-data probe is intentional test instrumentation for debug/fixture builds. It reports the route the app actually committed, while the screenshot supplies independent visual context. Production apps can instead use stable accessibility text or a log marker.

## Evidence per case

Every case result records:

- normalized classification;
- expected target and destination;
- observed target and destination, or an explicit `unobserved` value;
- screenshot and SHA-256 digest;
- relevant device logs and command transcript;
- duration and exact starting state;
- portable replay command.

DeepLink Lab never converts “app opened” or “probe unavailable” into a destination pass. See [the evidence architecture](docs/ARCHITECTURE.md).

## Hostile proof gates

The CLI encodes the project’s predeclared falsification gates:

```bash
deeplink-lab gates \
  --spec deeplinklab.yml \
  --representative proof/reference/report.json \
  --repeatability proof/repeatability/report.json
```

The evaluator requires:

1. at least five unique, materially different runtime-only failure classes;
2. at least 95% per-case classification agreement over 20 reset runs;
3. at least 20 cases across two local platforms in under 10 minutes;
4. a timed, configuration-only app integration under 30 minutes;
5. complete evidence for every runtime failure.

Missing evidence yields `unverified`, not a pass. Current reference proof and limitations are tracked in [docs/PROOF.md](docs/PROOF.md).

The repeatability proof deliberately covers one Android case for each of the six material failure classes (120 evidence-bearing executions), while the separate speed/evidence proof covers the full two-platform matrix. The configuration-only integration exercise builds a distinct `dev.deeplinklab.candidate` package, validates its new YAML record, reaches `product/7`, and confirms the runner-source hash did not change:

```bash
ANDROID_DEVICE=emulator-5554 scripts/prove-integration.sh
```

## Add an app without runner code

1. Add an `apps` entry with the platform identifier, build artifact, static config paths, and one supported destination probe.
2. Add links and exact destinations under `cases`.
3. Run `deeplink-lab validate` before booting a device.
4. Run one cold direct case, then expand lifecycle and source contexts.

The runner has no fixture bundle IDs, activities, routes, or screenshots compiled into it. Platform behavior is selected entirely by `platform` and the declarative app record. [CONTRIBUTING.md](CONTRIBUTING.md) includes the acceptance checklist for a community fixture.

## Deliberate exclusions

- no authenticated Instagram, TikTok, Facebook, Gmail, WhatsApp, or similar account automation;
- no managed real-device cloud in v1;
- no claim to prove deferred App Store or Play Store attribution;
- no assertion that simulated results equal physical-device behavior;
- no SaaS account, telemetry, hosted control plane, or credential custody.

## Project map

- `src/preflight.rs` — configuration facts and contradictions;
- `src/runtime.rs` — orchestration through platform tools and evidence capture;
- `src/report.rs` — self-contained HTML rendering;
- `src/gates.rs` — hostile proof-gate evaluation;
- `fixtures/` — dependency-light native apps and controlled website;
- `schemas/` — generated contract and report schemas;
- `scripts/build-npm.sh` — reproducible universal macOS npm artifact build;
- `proof/` — sanitized reference evidence, never marketing screenshots presented as certification.

Read [SECURITY.md](SECURITY.md) before testing links from untrusted parties. Contributions are welcome under the [Apache-2.0 license](LICENSE).
