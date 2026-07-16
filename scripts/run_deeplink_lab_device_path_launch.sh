#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUN_STARTED_EPOCH="$(date +%s)"
RUN_STARTED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
PROJECT_PATH="$ROOT_DIR/fixtures/ios/DeepLinkFixture.xcodeproj"
SCHEME="${SCHEME:-DeepLinkFixture}"
CONFIGURATION="${CONFIGURATION:-Debug}"
APP_NAME="${APP_NAME:-DeepLinkFixture}"
DEVICECTL_TIMEOUT="${DEVICECTL_TIMEOUT:-45}"
DEVICECTL_ATTEMPTS="${DEVICECTL_ATTEMPTS:-2}"
DEVICECTL_RETRY_DELAY="${DEVICECTL_RETRY_DELAY:-5}"
DESTINATION_ATTEMPTS="${DESTINATION_ATTEMPTS:-10}"
DESTINATION_RETRY_DELAY="${DESTINATION_RETRY_DELAY:-1}"
REQUIRE_INSTALL="${REQUIRE_INSTALL:-1}"
VERIFY_DESTINATION="${VERIFY_DESTINATION:-1}"
PAYLOAD_URL="${PAYLOAD_URL:-deeplinklab://good/product/42}"
EXPECTED_DESTINATION="${EXPECTED_DESTINATION:-product/42}"
OUTPUT_DIR="${OUTPUT_DIR:-$ROOT_DIR/work/physical-device}"
DERIVED_DATA_PATH="${DERIVED_DATA_PATH:-$OUTPUT_DIR/DerivedData}"
DEVICE_ID="${1:-${DEVICE_ID:-}}"
STARTING_STATE="preinstall"

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Required command not found: $1" >&2
    exit 1
  fi
}

for command_name in xcodebuild xcrun shasum jq plutil codesign; do
  require_command "$command_name"
done

device_candidates() {
  xcrun devicectl list devices 2>/dev/null |
    awk '
      function trim(value) {
        gsub(/^[ \t]+|[ \t]+$/, "", value)
        return value
      }

      /[[:xdigit:]]{8}-[[:xdigit:]]{4}-[[:xdigit:]]{4}-[[:xdigit:]]{4}-[[:xdigit:]]{12}/ && $0 ~ /(iPhone|iPad)/ {
        line = $0
        match(line, /[[:xdigit:]]{8}-[[:xdigit:]]{4}-[[:xdigit:]]{4}-[[:xdigit:]]{4}-[[:xdigit:]]{12}/)
        id_start = RSTART
        id_length = RLENGTH
        id = substr(line, id_start, id_length)
        name = trim(substr(line, 1, id_start - 1))
        if (match(name, /   +/)) name = trim(substr(name, 1, RSTART - 1))
        rest = trim(substr(line, id_start + id_length))
        state = rest
        if (match(rest, /   +/)) state = trim(substr(rest, 1, RSTART - 1))
        state_l = tolower(state)
        if (state_l == "connected") score = 100
        else if (index(state_l, "available") > 0 && index(state_l, "unavailable") == 0) score = 50
        else next
        printf "%03d\t%s\t%s\t%s\n", score, id, name, state
      }
    ' | sort -r
}

if [[ -z "$DEVICE_ID" ]]; then
  DEVICE_ID="$(device_candidates | awk -F '\t' 'NR == 1 { print $2 }')"
fi

if [[ -z "$DEVICE_ID" ]]; then
  echo "No connected or available paired iPhone was found." >&2
  echo "Connect and unlock the phone, or pass its CoreDevice identifier as the first argument." >&2
  echo "Example: REQUIRE_INSTALL=1 DEVICECTL_TIMEOUT=120 $0 <coredevice-id>" >&2
  exit 1
fi

DEVICE_LABEL="$(device_candidates | awk -F '\t' -v id="$DEVICE_ID" '$2 == id { printf "%s (%s)", $3, $4; exit }')"

discover_cached_xcode_team() {
  local capabilities_dir latest_file team
  capabilities_dir="$HOME/Library/Developer/Xcode/UserData/Capabilities"
  [[ -d "$capabilities_dir" ]] || return 1

  latest_file="$(
    find "$capabilities_dir" -type f -name 'capabilities-*-bundle.json' -exec stat -f '%m %N' {} + 2>/dev/null |
      sort -rn |
      sed -n '1s/^[0-9][0-9]* //p'
  )"
  [[ -n "$latest_file" ]] || return 1

  team="$(
    basename "$latest_file" |
      sed -nE 's/^capabilities-.*-([A-Z0-9]{10})-bundle\.json$/\1/p'
  )"
  [[ -n "$team" ]] || return 1
  printf '%s\n' "$team"
}

TEAM_FILE="${TEAM_FILE:-$OUTPUT_DIR/development-team.txt}"
DEVELOPMENT_TEAM_VALUE="${DEVELOPMENT_TEAM:-}"
if [[ -n "$DEVELOPMENT_TEAM_VALUE" ]]; then
  echo "Using the Xcode development team supplied through DEVELOPMENT_TEAM."
elif [[ -f "$TEAM_FILE" ]]; then
  DEVELOPMENT_TEAM_VALUE="$(tr -d '[:space:]' <"$TEAM_FILE")"
  echo "Using the development team saved by the last successful device build."
elif DEVELOPMENT_TEAM_VALUE="$(discover_cached_xcode_team)"; then
  echo "Using Xcode's most recently cached development team."
else
  echo "No Xcode development team could be selected automatically." >&2
  echo "Set DEVELOPMENT_TEAM to the intended 10-character team identifier and rerun." >&2
  exit 1
fi

if ! [[ "$DEVELOPMENT_TEAM_VALUE" =~ ^[A-Z0-9]{10}$ ]]; then
  echo "The selected DEVELOPMENT_TEAM is not a 10-character Apple team identifier." >&2
  exit 1
fi

TEAM_HASH="$(printf '%s' "$DEVELOPMENT_TEAM_VALUE" | shasum -a 256 | awk '{ print substr($1, 1, 10) }')"
BUNDLE_ID="${BUNDLE_ID:-dev.deeplinklab.fixture.device.t${TEAM_HASH}}"
APP_PATH="$DERIVED_DATA_PATH/Build/Products/${CONFIGURATION}-iphoneos/${APP_NAME}.app"
RUN_DIR="$OUTPUT_DIR/latest"
BUILD_LOG="$RUN_DIR/xcodebuild.log"
INSTALL_JSON="$RUN_DIR/install.json"
INSTALL_LOG="$RUN_DIR/install.log"
APPS_JSON="$RUN_DIR/apps.json"
APPS_LOG="$RUN_DIR/apps.log"
LAUNCH_JSON="$RUN_DIR/launch.json"
LAUNCH_LOG="$RUN_DIR/launch.log"
PROCESSES_JSON="$RUN_DIR/processes.json"
PROCESSES_LOG="$RUN_DIR/processes.log"
COPY_JSON="$RUN_DIR/copy-destination.json"
COPY_LOG="$RUN_DIR/copy-destination.log"
OBSERVED_FILE="$RUN_DIR/observed-destination.txt"
SUMMARY_FILE="$RUN_DIR/summary.txt"
ENTITLEMENTS_FILE="$RUN_DIR/signed-entitlements.plist"

mkdir -p "$DERIVED_DATA_PATH" "$RUN_DIR"
rm -f "$BUILD_LOG" "$INSTALL_JSON" "$INSTALL_LOG" "$APPS_JSON" "$APPS_LOG" \
  "$LAUNCH_JSON" "$LAUNCH_LOG" "$PROCESSES_JSON" "$PROCESSES_LOG" \
  "$COPY_JSON" "$COPY_LOG" "$OBSERVED_FILE" "$SUMMARY_FILE" "$ENTITLEMENTS_FILE"

write_summary() {
  local status="$1"
  local actual_destination="${2:-unobserved}"
  local duration_seconds
  duration_seconds="$(($(date +%s) - RUN_STARTED_EPOCH))"
  {
    printf 'status=%s\n' "$status"
    printf 'evidence_kind=physical_device_direct_payload\n'
    printf 'started_at=%s\n' "$RUN_STARTED_AT"
    printf 'duration_seconds=%s\n' "$duration_seconds"
    printf 'starting_state=%s\n' "$STARTING_STATE"
    printf 'device_id=%s\n' "$DEVICE_ID"
    printf 'device_label=%s\n' "$DEVICE_LABEL"
    printf 'bundle_id=%s\n' "$BUNDLE_ID"
    printf 'payload_url=%s\n' "$PAYLOAD_URL"
    printf 'expected_destination=%s\n' "$EXPECTED_DESTINATION"
    printf 'actual_destination=%s\n' "$actual_destination"
    printf 'app_path=%s\n' "$APP_PATH"
    printf 'build_log=%s\n' "$BUILD_LOG"
    printf 'install_json=%s\n' "$INSTALL_JSON"
    printf 'install_log=%s\n' "$INSTALL_LOG"
    printf 'launch_json=%s\n' "$LAUNCH_JSON"
    printf 'launch_log=%s\n' "$LAUNCH_LOG"
    printf 'processes_json=%s\n' "$PROCESSES_JSON"
    printf 'processes_log=%s\n' "$PROCESSES_LOG"
    printf 'destination_copy_json=%s\n' "$COPY_JSON"
    printf 'destination_copy_log=%s\n' "$COPY_LOG"
    printf 'signed_entitlements=%s\n' "$ENTITLEMENTS_FILE"
    printf 'screenshot=not_captured_by_devicectl\n'
    printf 'replay_command=REQUIRE_INSTALL=%q DEVICECTL_TIMEOUT=%q PAYLOAD_URL=%q EXPECTED_DESTINATION=%q %q %q\n' \
      "$REQUIRE_INSTALL" "$DEVICECTL_TIMEOUT" "$PAYLOAD_URL" "$EXPECTED_DESTINATION" "$0" "$DEVICE_ID"
    printf 'claim_boundary=Direct payload delivery proves fixture routing on this physical device; it does not prove Universal Link association.\n'
  } >"$SUMMARY_FILE"
}

classify_build_failure() {
  if grep -Eiq 'doesn.t match .* deployment target|deployment target' "$BUILD_LOG"; then
    printf 'deployment_target_ineligible\n'
  elif grep -Eiq 'provisioning profile.*doesn.t include|requires a provisioning profile|no profiles for|signing.*failed' "$BUILD_LOG"; then
    printf 'signing_provisioning_failed\n'
  elif grep -Eiq 'need to be unlocked|could not be.*unlocked|preparation errors|Timed out waiting for all destinations' "$BUILD_LOG"; then
    printf 'device_preparation_locked\n'
  elif grep -Eiq 'Unable to find a destination matching|ineligible destinations|not available because' "$BUILD_LOG"; then
    printf 'device_destination_unavailable\n'
  else
    printf 'build_failed\n'
  fi
}

summarize_devicectl_error() {
  local json_path="$1"
  [[ -f "$json_path" ]] || return 0
  plutil -extract error.userInfo.NSLocalizedDescription.string raw -o - "$json_path" 2>/dev/null || true
}

run_with_retries() {
  local label="$1"
  local json_path="$2"
  shift 2

  local attempt=1
  while ((attempt <= DEVICECTL_ATTEMPTS)); do
    rm -f "$json_path"
    printf '%s attempt %s/%s...\n' "$label" "$attempt" "$DEVICECTL_ATTEMPTS"
    if "$@"; then
      return 0
    fi
    summarize_devicectl_error "$json_path" >&2
    if [[ -f "$json_path" ]] && grep -Eiq 'Locked|could not be.*unlocked' "$json_path"; then
      echo "$label failed because the phone is locked. Unlock it before rerunning." >&2
      return 1
    fi
    if ((attempt < DEVICECTL_ATTEMPTS)); then
      sleep "$DEVICECTL_RETRY_DELAY"
    fi
    attempt=$((attempt + 1))
  done
  return 1
}

echo "Building $SCHEME for ${DEVICE_LABEL:-the selected iPhone}..."
if ! xcodebuild \
  -project "$PROJECT_PATH" \
  -scheme "$SCHEME" \
  -configuration "$CONFIGURATION" \
  -destination "id=$DEVICE_ID" \
  -derivedDataPath "$DERIVED_DATA_PATH" \
  -allowProvisioningUpdates \
  -allowProvisioningDeviceRegistration \
  DEVELOPMENT_TEAM="$DEVELOPMENT_TEAM_VALUE" \
  PRODUCT_BUNDLE_IDENTIFIER="$BUNDLE_ID" \
  CODE_SIGN_STYLE=Automatic \
  ENABLE_DEBUG_DYLIB=NO \
  build 2>&1 | tee "$BUILD_LOG"; then
  build_status="$(classify_build_failure)"
  write_summary "$build_status"
  echo "Device build failed ($build_status). See $BUILD_LOG" >&2
  exit 1
fi

if [[ ! -d "$APP_PATH" ]]; then
  write_summary "artifact_missing"
  echo "Built app not found at $APP_PATH" >&2
  exit 1
fi

codesign --verify --deep --strict "$APP_PATH"
codesign -d --entitlements :- "$APP_PATH" >"$ENTITLEMENTS_FILE" 2>/dev/null || true
umask 077
printf '%s\n' "$DEVELOPMENT_TEAM_VALUE" >"$TEAM_FILE"

echo "Installing $APP_PATH..."
if run_with_retries "install app" "$INSTALL_JSON" \
  xcrun devicectl device install app \
  --device "$DEVICE_ID" \
  --timeout "$DEVICECTL_TIMEOUT" \
  --json-output "$INSTALL_JSON" \
  --log-output "$INSTALL_LOG" \
  "$APP_PATH"; then
  STARTING_STATE="installed_fresh,cold,direct_payload"
elif [[ "$REQUIRE_INSTALL" == "1" ]]; then
  write_summary "install_failed"
  echo "Install did not complete; refusing to launch a stale build because REQUIRE_INSTALL=1." >&2
  exit 1
else
  STARTING_STATE="existing_installation,cold,direct_payload"
  echo "Install failed; REQUIRE_INSTALL=0 permits checking an existing installation." >&2
fi

if ! run_with_retries "installed app lookup" "$APPS_JSON" \
  xcrun devicectl device info apps \
  --device "$DEVICE_ID" \
  --bundle-id "$BUNDLE_ID" \
  --timeout "$DEVICECTL_TIMEOUT" \
  --json-output "$APPS_JSON" \
  --log-output "$APPS_LOG"; then
  write_summary "installed_app_unavailable"
  echo "The installed app could not be found for bundle $BUNDLE_ID." >&2
  exit 1
fi

if ! jq -e '.result.apps[0] // .result.installedApplications[0]' "$APPS_JSON" >/dev/null 2>&1; then
  write_summary "installed_app_unavailable"
  echo "The app lookup returned no installed application for bundle $BUNDLE_ID." >&2
  exit 1
fi

echo "Launching $PAYLOAD_URL as a cold direct payload..."
launch_succeeded=0
if run_with_retries "launch app" "$LAUNCH_JSON" \
  xcrun devicectl device process launch \
  --device "$DEVICE_ID" \
  --terminate-existing \
  --payload-url "$PAYLOAD_URL" \
  --timeout "$DEVICECTL_TIMEOUT" \
  --json-output "$LAUNCH_JSON" \
  --log-output "$LAUNCH_LOG" \
  "$BUNDLE_ID"; then
  launch_succeeded=1
fi

if [[ "$launch_succeeded" != "1" ]] && [[ -f "$LAUNCH_JSON" ]] && \
  grep -Eiq 'Locked|could not be.*unlocked' "$LAUNCH_JSON"; then
  write_summary "locked_launch_denied"
  echo "Launch was denied because the phone is locked. Unlock it and rerun." >&2
  exit 1
fi

sleep 1
if ! run_with_retries "process verification" "$PROCESSES_JSON" \
  xcrun devicectl device info processes \
  --device "$DEVICE_ID" \
  --timeout "$DEVICECTL_TIMEOUT" \
  --quiet \
  --json-output "$PROCESSES_JSON" \
  --log-output "$PROCESSES_LOG"; then
  write_summary "launch_unverified"
  echo "Could not read the post-launch process list." >&2
  exit 1
fi

APP_PID="$(
  jq -r --arg suffix "/${APP_NAME}.app/${APP_NAME}" '
    .result.runningProcesses[]? |
      select(.executable | type == "string" and endswith($suffix)) |
      .processIdentifier
  ' "$PROCESSES_JSON" 2>/dev/null | head -n 1
)"

if [[ -z "$APP_PID" ]]; then
  write_summary "not_running"
  echo "$APP_NAME was not found in the device process list." >&2
  exit 1
fi

if [[ "$launch_succeeded" != "1" ]]; then
  echo "The launch command reported an error, but the app is running; destination evidence will decide the result." >&2
fi

if [[ "$VERIFY_DESTINATION" != "1" ]]; then
  write_summary "launched_unobserved"
  echo "DeepLinkFixture is running as PID $APP_PID. Destination verification was disabled."
  echo "Evidence: $SUMMARY_FILE"
  exit 0
fi

actual_destination=""
attempt=1
while ((attempt <= DESTINATION_ATTEMPTS)); do
  rm -f "$OBSERVED_FILE" "$COPY_JSON"
  if xcrun devicectl device copy from \
    --device "$DEVICE_ID" \
    --domain-type appDataContainer \
    --domain-identifier "$BUNDLE_ID" \
    --source Documents/destination.txt \
    --destination "$OBSERVED_FILE" \
    --timeout "$DEVICECTL_TIMEOUT" \
    --json-output "$COPY_JSON" \
    --log-output "$COPY_LOG" >/dev/null 2>&1; then
    if [[ -f "$OBSERVED_FILE" ]]; then
      actual_destination="$(tr -d '\r\n' <"$OBSERVED_FILE")"
      [[ -n "$actual_destination" ]] && break
    fi
  fi
  if ((attempt < DESTINATION_ATTEMPTS)); then
    sleep "$DESTINATION_RETRY_DELAY"
  fi
  attempt=$((attempt + 1))
done

if [[ -z "$actual_destination" ]]; then
  write_summary "observation_unavailable"
  echo "The app is running, but its destination evidence could not be read from the app container." >&2
  echo "This is not counted as a pass. Evidence: $SUMMARY_FILE" >&2
  exit 1
fi

if [[ "$actual_destination" != "$EXPECTED_DESTINATION" ]]; then
  write_summary "wrong_destination" "$actual_destination"
  echo "Destination mismatch: expected '$EXPECTED_DESTINATION', observed '$actual_destination'." >&2
  echo "Evidence: $SUMMARY_FILE" >&2
  exit 2
fi

write_summary "passed" "$actual_destination"
echo "PASS: the physical iPhone committed destination '$actual_destination'."
echo "DeepLinkFixture is running as PID $APP_PID."
echo "Evidence: $SUMMARY_FILE"
echo "Boundary: this is direct payload routing evidence, not Universal Link association proof."
