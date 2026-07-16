# Runtime boundaries

## What a local result means

An iOS result is an observation from the named Simulator runtime and `simctl` behavior on the host Xcode version. An Android result is an observation from the named emulator image and `adb`/package-manager behavior. Both are valuable regression evidence and neither certifies a physical device.

## Installed and uninstalled

The built-in lanes install local artifacts and can remove them before opening a link. A custom scheme with no installed handler is directly observable. Store fallback and deferred attribution are not: DeepLink Lab does not install from App Store, Play Store, TestFlight, or internal tracks and does not reset advertising/vendor identity.

## Verification caches and user choices

Android exposes package-manager commands for verification reset, re-verification, and user selection. Their result still depends on emulator OS version and live domain reachability.

Apple's association CDN, user choices, and real-device state do not have equivalent public Simulator invalidation commands. Reinstalling an app can isolate local app state but must not be described as clearing Apple's production caches.

## Browsers and controlled pages

Direct platform URL commands are kept separate from browser/page taps. Browser lanes use Maestro because it already owns UI interaction. Chrome must already be installed on the selected device. The project does not automate authenticated social, messaging, or email applications.

## Destination observability

Exact destination claims require a stable observable. The fixture apps write their committed route to a debug-only file and display the same value on screen. Real apps may expose a debug route file, a structured log marker, or a stable accessibility label through Maestro.

If only app foregrounding can be seen, the result is `observation_unavailable`; it is not a destination pass.

## Physical devices

The v1 repository intentionally has no device enrollment, signing custody, USB provisioning, or cloud credentials. A future physical-device adapter must preserve the evidence contract and use an established transport supported on the target platform, such as Appium/XCUITest on iOS. It must be labeled separately from local simulated evidence.

The [physical iPhone guide](PHYSICAL-DEVICE.md) documents the supported BYO procedure and distinguishes direct app payload delivery from a real Universal Link tap.
