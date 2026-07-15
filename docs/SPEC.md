# `deeplinklab.yml` specification

The current contract version is `1`. Unknown keys are rejected. Paths are resolved relative to the specification file.

Generate the authoritative JSON Schema with:

```bash
deeplink-lab schema spec
```

## `apps`

Each map key is a local stable name used by cases.

Required fields:

- `platform`: `ios` or `android`;
- `appId`: bundle ID or Android package;
- `artifact`: `.app` bundle or `.apk` path;
- `probe`: one exact-destination observation strategy.

iOS static fields are `teamId`, `entitlements`, and `infoPlist`. Android static fields are `manifest` and `certFingerprints`. Warm/background Android cases also require `launchActivity`. `logProcess` narrows platform log capture.

## `associations`

An association has a bare `domain` and optionally local `aasa` and `assetlinks` files. HTTPS cases must have a matching domain entry. DeepLink Lab checks:

- JSON structure and Apple's uncompressed 128 KB AASA limit;
- AASA team/bundle app IDs and associated-domain entitlements;
- Android relation, package, and uppercase SHA-256 fingerprint format;
- Android browsable `VIEW` filters, unresolved placeholders, and declared schemes;
- contradictions across the files and declared app.

The local file check does not establish response headers, redirects, TLS, CDN state, or live propagation. Those remain remote and runtime facts.

## `cases`

`id` is a lowercase stable identifier. `app`, `link`, and `expected` are required.

`expected.target` is:

- `app`: requires an exact `destination`;
- `browser`: requires an observable browser result to pass;
- `unhandled`: passes only when no handler is observed.

`runtimeFailureClass` is fixture/proof metadata. It names the material behavior a deliberately broken route represents; it is only counted when static preflight passed and runtime actually failed.

## Starting state

Defaults are installed, cold, preserve verification, and preserve user default.

```yaml
state:
  install: installed       # installed | uninstalled
  launch: cold             # cold | warm | background
  verification: preserve   # preserve | reset | reverify
  userDefault: preserve    # preserve | reset
```

Android exposes public `pm` reset and re-verification commands. iOS does not expose equivalent Apple CDN or physical-device user-choice invalidation; the report records the limitation and treats reinstall as a Simulator-only approximation.

## Source context

`direct` uses `simctl openurl` or Android `VIEW`/`BROWSABLE`. `safari`, `chrome`, and `controlled_page` require `pageUrl`, `tapText`, and Maestro. The page tap is preserved as a generated Maestro flow in case evidence.

```yaml
source:
  context: controlled_page
  pageUrl: http://127.0.0.1:8765/
  tapText: Open product 42
```

Synthetic `openurl` and `am start` commands are not described as human taps. Source context is therefore explicit rather than inferred.

## Gates

Gate thresholds default to the hostile project contract and can only be made stricter in accepted project fixtures:

```yaml
gates:
  novelFailureClasses: 5
  repeatRuns: 20
  repeatabilityPercent: 95
  representativeCaseCount: 20
  representativePlatformCount: 2
  maxDurationMs: 600000
  maxIntegrationSeconds: 1800
```

An absent repeated run or timed integration yields `unverified`.
