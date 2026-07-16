# Physical iPhone testing

DeepLink Lab v1 does not enroll, provision, or drive physical iPhones. Its built-in
`--ios-device` lane calls `xcrun simctl` and is Simulator-only. Do not pass a physical
device identifier to that option or label a manual device run as a built-in runtime result.

This guide defines two bring-your-own-device lanes:

1. a fast `devicectl` router smoke test against an already signed app;
2. an actual Universal Link test, driven manually or with Appium/XCUITest.

Both are physical-device observations. Neither proves deferred App Store attribution,
Apple CDN invalidation, or behavior on other device and OS combinations.

## Prerequisites

- Xcode must support the iOS version installed on the device. If the phone runs a newer
  iOS release than the installed Xcode/SDK supports, update Xcode or use a compatible phone.
- Connect the unlocked phone to the Mac, accept **Trust This Computer**, and enable
  **Settings > Privacy & Security > Developer Mode**.
- Build the app for `iphoneos`, sign it with a valid Apple Development identity and
  provisioning profile, and include the device in that profile.
- Expose an exact destination through stable accessibility text or a debug log marker.
- For Appium, also enable **Settings > Developer > Enable UI Automation**. Enable Safari
  Web Inspector and Remote Automation only when the test needs a webview.

Confirm the device is visible without copying its private identifier into reports:

```bash
xcrun devicectl list devices
xcrun devicectl device info lockState --device "My iPhone"
```

## Lane 1: signed-app router smoke

This lane proves that a physical build receives a URL and commits the expected in-app
destination. It does **not** exercise iOS Universal Link association because `devicectl`
delivers the payload directly to the named app.

```bash
export DEVICE="My iPhone"
export APP="/absolute/path/to/MyApp.app"
export BUNDLE_ID="com.example.myapp"
export LINK="myapp://products/42"
export OUT="outputs/physical/router-cold"

mkdir -p "$OUT"
xcrun devicectl device install app \
  --device "$DEVICE" \
  --json-output "$OUT/install.json" \
  --log-output "$OUT/install.log" \
  "$APP"

xcrun devicectl device process launch \
  --device "$DEVICE" \
  --terminate-existing \
  --payload-url "$LINK" \
  --json-output "$OUT/launch.json" \
  --log-output "$OUT/launch.log" \
  "$BUNDLE_ID"
```

Read the actual destination from the app's visible debug label or structured log marker.
Capture a device screenshot, the expected and actual destination, duration, starting state,
and the exact command above as the replay path. `launch.log` is a `devicectl` command log;
capture app logs separately with Xcode's debug console, macOS Console, or Appium.

Repeat the payload without `--terminate-existing` for a warm delivery. Put the app in the
background before delivery for a background case. Record those manual state changes; do not
describe them as automated resets.

## Lane 2: actual Universal Link

An actual Universal Link requires a two-way production-shaped association:

1. Serve `https://links.example.com/.well-known/apple-app-site-association` over HTTPS with
   a valid certificate and no redirect. Its app identifier must be the real
   `<TEAM_ID>.<BUNDLE_ID>` and its components must cover the test path.
2. Sign the app with the Associated Domains capability and an entitlement containing
   `applinks:links.example.com`.
3. Implement the app's universal-link continuation path and expose the committed route.
4. Put the AASA file in place before installing the app. Delete and reinstall the signed
   build when changing the entitlement or association.
5. Tap the link from Notes, Messages, or a controlled page on a different origin. Do not use
   `devicectl --payload-url` as association evidence and do not type the URL into Safari's
   address bar as a substitute for a user tap.
6. Assert the exact visible destination, not merely that the app foregrounded.

Run at least these cases:

| Case | Starting state | Expected observation |
| --- | --- | --- |
| Installed, cold | App force-quit | App opens at the exact route |
| Installed, warm | App already running | Continuation reaches the exact route |
| Installed, background | App behind Notes/browser | Continuation reaches the exact route |
| Uninstalled | App removed | Website opens; no app destination is claimed |

Use `xcrun devicectl device uninstall app --device "$DEVICE" "$BUNDLE_ID"` for the
uninstalled case. Reinstalling isolates app state but does not prove that Apple's association
CDN or the user's prior link preference was cleared.

## Automating the physical lane

Use Appium's XCUITest driver when repeatable UI taps, screenshots, logs, and accessibility
assertions are required. A real-device Appium session installs WebDriverAgent, so WDA needs
its own valid signing team, provisioning profile, and unique bundle identifier. Keep one WDA
installation alive across cases where possible instead of reinstalling it for every test.

Local Maestro execution currently supports iOS Simulators, not physical iOS devices. Do not
present a local Maestro flow as physical-iPhone evidence. Appium is the supported established
transport for this lane until DeepLink Lab has an explicit physical-device adapter.

Regardless of transport, every failure record should contain:

- classification and starting state;
- expected and actual destination;
- screenshot and relevant app/device logs;
- duration and device/OS description without publishing a UDID or serial number;
- a one-command or one-flow replay path;
- an explicit `physical_device` evidence label.

Keep this evidence outside the built-in Simulator report unless an adapter validates and
normalizes it. Missing destination observability remains `observation_unavailable`, never a
pass.

## Why the committed fixture is not a physical Universal Link fixture

The checked-in iOS fixture is intentionally compiled for the Simulator and ad-hoc signed.
Its team ID is a placeholder, `links.example.test` is a reserved offline domain, and the app
implements the custom-scheme URL callbacks used by the deterministic suite. It therefore
must not be installed or cited as real Universal Link proof without a separate device build,
real team/bundle identity, live domain, and universal-link continuation handler.

## Primary references

- [Apple: Supporting associated domains](https://developer.apple.com/documentation/xcode/supporting-associated-domains)
- [Apple: Allowing apps and websites to link to your content](https://developer.apple.com/documentation/xcode/allowing-apps-and-websites-to-link-to-your-content/)
- [Appium XCUITest: real-device configuration](https://appium.github.io/appium-xcuitest-driver/latest/preparation/real-device-config/)
- [Maestro UIKit platform support](https://docs.maestro.dev/platform-support/ios-uikit)
