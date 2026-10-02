#!/usr/bin/env bash
set -euo pipefail

export JAVA_HOME=/home/mohy/Programs/tunestash-jdk17
export ANDROID_HOME=/home/mohy/Android/Sdk
export NDK_HOME="$ANDROID_HOME/ndk/30.0.16248370"
export PATH="$JAVA_HOME/bin:$ANDROID_HOME/platform-tools:$PATH"



repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ -z "${JAVA_HOME:-}" || ! -x "$JAVA_HOME/bin/java" ]]; then
  echo "Set JAVA_HOME to a JDK 17 installation." >&2
  exit 1
fi

if ! "$JAVA_HOME/bin/java" -version 2>&1 | grep -Eq 'version "17(\.|\")'; then
  echo "JAVA_HOME must point to JDK 17." >&2
  exit 1
fi

ANDROID_HOME="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-}}"
if [[ -z "$ANDROID_HOME" || ! -f "$ANDROID_HOME/platforms/android-34/android.jar" ]]; then
  echo "Set ANDROID_HOME to an Android SDK with platform 34 installed." >&2
  exit 1
fi

if [[ -z "${NDK_HOME:-}" || ! -d "$NDK_HOME/toolchains/llvm/prebuilt" ]]; then
  echo "Set NDK_HOME to an installed Android NDK." >&2
  exit 1
fi

if ! command -v dx >/dev/null || [[ "$(dx --version)" != dioxus\ 0.7.10* ]]; then
  echo "Dioxus CLI 0.7.10 is required." >&2
  exit 1
fi

if ! rustup target list --installed | grep -Fxq aarch64-linux-android; then
  echo "Install the ARM64 Rust target: rustup target add aarch64-linux-android" >&2
  exit 1
fi

export JAVA_HOME ANDROID_HOME NDK_HOME
export PATH="$JAVA_HOME/bin:$ANDROID_HOME/platform-tools:$PATH"

cd "$repo_root"
dx bundle --android --target aarch64-linux-android --package-types apk --package music_app --locked

apk="$repo_root/target/dx/TuneStash/debug/android/app/app/build/outputs/apk/debug/app-debug.apk"
if [[ ! -s "$apk" ]]; then
  echo "Android build finished without the expected debug APK: $apk" >&2
  exit 1
fi

printf 'Debug APK: %s\n' "$apk"
