#!/usr/bin/env bash
set -euo pipefail

export ANDROID_HOME=/home/mohy/Android/Sdk

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
apk="$repo_root/target/dx/TuneStash/debug/android/app/app/build/outputs/apk/debug/app-debug.apk"
sdk_root="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}"

if [[ -n "$sdk_root" && -x "$sdk_root/platform-tools/adb" ]]; then
  adb_bin="$sdk_root/platform-tools/adb"
elif command -v adb >/dev/null; then
  adb_bin="$(command -v adb)"
else
  echo "ADB not found. Set ANDROID_HOME or add platform-tools to PATH." >&2
  exit 1
fi

if [[ ! -s "$apk" ]]; then
  echo "Debug APK not found. Run ./scripts/build-android.sh first." >&2
  exit 1
fi

if (( $# > 1 )); then
  echo "Usage: $0 [device-serial]" >&2
  exit 2
fi

device_serial="${1:-${ANDROID_SERIAL:-}}"
if [[ -z "$device_serial" ]]; then
  mapfile -t connected_devices < <("$adb_bin" devices | awk 'NR > 1 && $2 == "device" { print $1 }')
  if (( ${#connected_devices[@]} == 0 )); then
    echo "No authorized ADB device found. Connect a device and enable USB debugging." >&2
    exit 1
  elif (( ${#connected_devices[@]} > 1 )); then
    echo "Multiple ADB devices found. Pass the target serial or set ANDROID_SERIAL:" >&2
    printf '  %s\n' "${connected_devices[@]}" >&2
    exit 1
  fi
  device_serial="${connected_devices[0]}"
fi

device_state="$("$adb_bin" -s "$device_serial" get-state 2>/dev/null || true)"
if [[ "$device_state" != "device" ]]; then
  echo "ADB device '$device_serial' is not connected and authorized (state: ${device_state:-unavailable})." >&2
  exit 1
fi

android_user_id="${ANDROID_USER_ID:-0}"
"$adb_bin" -s "$device_serial" install -r --user "$android_user_id" "$apk"
"$adb_bin" -s "$device_serial" shell am start --user "$android_user_id" -W -n com.tunestash.app/dev.dioxus.main.MainActivity
printf 'Installed TuneStash on %s (Android user %s).\n' "$device_serial" "$android_user_id"
printf 'Launched TuneStash.\n'
