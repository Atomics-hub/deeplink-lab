# Deterministic fixtures

The fixture corpus makes every public behavior reproducible without private apps or third-party assets.

## Native apps

The iOS fixture is one UIKit source file compiled directly with the installed Simulator SDK. The Android fixture is one Java activity compiled with the installed Android SDK build tools. Neither app has network dependencies or analytics.

The iOS build validates the checked-in associated-domains entitlement as static input but does not embed that privileged entitlement in its ad-hoc Simulator signature. Newer runtimes reject an entitlement that no Apple development team granted; the custom-scheme runtime cases therefore remain deterministic without pretending to prove a signed production Universal Link.

Both apps:

- display the committed destination;
- persist it in a debug-only app data file;
- emit `DEEPLINKLAB_DESTINATION=...` to platform logs;
- register the `deeplinklab://` custom scheme for deterministic local routing;
- include static Universal Link/App Link configuration for offline preflight.

The runtime corpus uses a custom scheme because a public repository cannot honestly provide a universally trusted local HTTPS origin, Apple CDN state, or contributor signing identity. The controlled website and association files exercise the configuration and browser-lane contract without pretending localhost proves production association.

## Intentional runtime failures

| Family | Material failure missed by association-file validators |
| --- | --- |
| `wrong` | App opens the home screen instead of the product |
| `query-loss` | Router drops a required query value |
| `fragment-loss` | Router drops an in-page destination fragment |
| `route-collision` | Generic user route wins over a privileged/static route |
| `warm-stale` | Continuation URL is ignored while the process is warm |
| `background-ignore` | URL is ignored when returning from background |

These failures are outside the scope of AASA/assetlinks syntax and ownership checks. They are not presented as undiscovered OS bugs.

## Configuration-only candidate

`fixtures/android-candidate` builds a second package identity from the same auditable activity source. `examples/integration-candidate.yml` integrates that package only through the public contract. `scripts/prove-integration.sh` hashes `src/runtime.rs` before and after the build, validation, install, link dispatch, and exact destination assertion; any runner change or destination mismatch fails the exercise.

## Website

Serve `fixtures/site` for optional controlled-page Maestro cases:

```bash
python3 -m http.server 8765 --directory fixtures/site
```

The reserved `example.test` domain appears only in offline configuration fixtures. Do not deploy it or infer live association behavior from it.

## Contributing a fixture

A fixture must be deterministic, license-compatible, free of credentials, minimal enough to audit, and accompanied by one known-good case plus the smallest case that demonstrates the fault. Framework reproductions are welcome when they add a behavior not already represented.
