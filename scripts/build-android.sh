#!/usr/bin/env bash
set -euo pipefail
trap 'echo "Android build failed near line $LINENO" >&2' ERR

export JAVA_HOME="${JAVA_HOME:-/home/mohy/Programs/tunestash-jdk17}"
export ANDROID_HOME="${ANDROID_HOME:-/home/mohy/Android/Sdk}"
export NDK_HOME="${NDK_HOME:-$ANDROID_HOME/ndk/30.0.16248370}"
export ANDROID_NDK_HOME="$NDK_HOME"
export PATH="$JAVA_HOME/bin:$ANDROID_HOME/platform-tools:$PATH"



repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [[ ! -f "$repo_root/.env" ]]; then
  echo "Copy .env.example to .env and set your Telegram app credentials." >&2
  exit 1
fi
set -a
source "$repo_root/.env"
set +a
if [[ ! "${TELEGRAM_API_ID:-}" =~ ^[0-9]+$ || -z "${TELEGRAM_API_HASH:-}" || "${TELEGRAM_API_HASH}" == your_api_hash ]]; then
  echo "Set TELEGRAM_API_ID and TELEGRAM_API_HASH in .env." >&2
  exit 1
fi

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

tdlib="$repo_root/target/native/tdlib/arm64-v8a/libtdjson.so"
if [[ ! -s "$tdlib" ]]; then
  "$repo_root/scripts/build-tdlib-android.sh"
fi

cd "$repo_root"
# Dioxus regenerates its WebP launcher icons before our overlay runs. Remove
# the previous overlay first so its intermediate Gradle build has no duplicate
# ic_launcher resources on repeated runs.
for density in mdpi hdpi xhdpi xxhdpi xxxhdpi; do
  rm -f "$repo_root/target/dx/TuneStash/debug/android/app/app/src/main/res/mipmap-$density/ic_launcher.png"
done
dx bundle --android --target aarch64-linux-android --package-types apk --package music_app --locked

# dx owns generated Gradle files, so apply the native resource and library
# overlay after every regeneration, then assemble the final APK.
generated="$repo_root/target/dx/TuneStash/debug/android/app"
main="$generated/app/src/main"
mkdir -p "$main/jniLibs/arm64-v8a" "$main/res/values-v31"
cp "$tdlib" "$main/jniLibs/arm64-v8a/libtdjson.so"
cp "$repo_root/android/res/drawable/"*.xml "$main/res/drawable/"
cp "$repo_root/android/res/values/colors.xml" "$main/res/values/tunestash_colors.xml"
cp "$repo_root/android/res/values/styles.xml" "$main/res/values/tunestash_styles.xml"
cp "$repo_root/android/res/values-v31/styles.xml" "$main/res/values-v31/tunestash_styles.xml"
for density in mdpi hdpi xhdpi xxhdpi xxxhdpi; do
  rm -f "$main/res/mipmap-$density/ic_launcher.webp"
  cp "$repo_root/android/res/mipmap-$density/ic_launcher.png" "$main/res/mipmap-$density/ic_launcher.png"
done
cp "$repo_root/android/res/mipmap-anydpi-v26/ic_launcher.xml" "$main/res/mipmap-anydpi-v26/ic_launcher.xml"
python3 - "$main/AndroidManifest.xml" <<'PY'
from pathlib import Path
import sys
path = Path(sys.argv[1])
manifest = path.read_text()
old = 'android:theme="@style/AppTheme"'
if old not in manifest:
    raise SystemExit('Could not locate generated Android theme')
path.write_text(manifest.replace(old, 'android:theme="@style/TuneStashTheme"', 1))
PY
( cd "$generated" && ./gradlew :app:assembleDebug --offline )

apk="$repo_root/target/dx/TuneStash/debug/android/app/app/build/outputs/apk/debug/app-debug.apk"
if [[ ! -s "$apk" ]]; then
  echo "Android build finished without the expected debug APK: $apk" >&2
  exit 1
fi
python3 - "$apk" <<'PY'
import sys
from zipfile import ZipFile
with ZipFile(sys.argv[1]) as apk:
    if 'lib/arm64-v8a/libtdjson.so' not in apk.namelist():
        raise SystemExit('Final APK does not contain TDLib.')
PY

printf 'Debug APK: %s\n' "$apk"
