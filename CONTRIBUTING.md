# Contributing

DeepLink Lab values reproducible evidence over broad feature claims. Small, inspectable changes are preferred.

## Development setup

```bash
cargo test --all-targets
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
scripts/check-public.sh
scripts/check-npm-metadata.sh
```

On macOS, verify the exact npm tarball and a clean install of its universal binary:

```bash
npm run test:package
```

Build fixture apps on a Mac with Xcode and Android SDK installed:

```bash
fixtures/ios/build.sh
fixtures/android/build.sh
```

## Pull request checklist

- add unit/regression coverage for classification or preflight changes;
- regenerate schemas with `scripts/update-schemas.sh`;
- preserve JSON compatibility or document an intentional version change;
- include exact proof commands and bounded claims;
- do not add private apps, credentials, absolute workstation paths, third-party account automation, or copied assets;
- do not add app-specific branches to the runtime runner.

## New fixture acceptance

A new app should be integrated only by adding an app record, supported probe, and cases. If platform runner code must recognize its bundle ID, activity, route, or UI, the design is not generic enough.

Fixtures must contain one known-good control, an intentionally broken minimal case, deterministic reset instructions, and a short explanation of why static association validation cannot see the runtime fault.

## Architecture changes

Open an issue before adding a device provider, source application, arbitrary command probe, remote service, or evidence schema version. Those changes alter trust boundaries and need explicit review.
