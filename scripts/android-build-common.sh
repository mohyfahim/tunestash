#!/usr/bin/env bash
# Shared checks and generated-project overlay for debug and release APKs.

android_build_setup() {
  export JAVA_HOME="${JAVA_HOME:-/home/mohy/Programs/tunestash-jdk17}"
  export ANDROID_HOME="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-/home/mohy/Android/Sdk}}"
  export NDK_HOME="${NDK_HOME:-$ANDROID_HOME/ndk/30.0.16248370}"
  export ANDROID_NDK_HOME="$NDK_HOME"
  export PATH="$JAVA_HOME/bin:$ANDROID_HOME/platform-tools:$PATH"

  if [[ ! -f "$repo_root/.env" ]]; then
    echo "Copy .env.example to .env and set your Telegram app credentials." >&2
    return 1
  fi
  set -a
  # shellcheck disable=SC1091
  source "$repo_root/.env"
  set +a
  if [[ ! "${TELEGRAM_API_ID:-}" =~ ^[0-9]+$ || -z "${TELEGRAM_API_HASH:-}" || "$TELEGRAM_API_HASH" == your_api_hash ]]; then
    echo "Set TELEGRAM_API_ID and TELEGRAM_API_HASH in .env." >&2
    return 1
  fi

  if [[ ! -x "$JAVA_HOME/bin/java" ]] || ! "$JAVA_HOME/bin/java" -version 2>&1 | grep -Eq 'version "17(\.|\")'; then
    echo "Set JAVA_HOME to a JDK 17 installation." >&2
    return 1
  fi
  if [[ ! -f "$ANDROID_HOME/platforms/android-34/android.jar" || ! -f "$ANDROID_HOME/platforms/android-35/android.jar" ]]; then
    echo "Set ANDROID_HOME to an Android SDK with platforms 34 and 35." >&2
    return 1
  fi

  local ndk_bin="$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin"
  if [[ ! -x "$ndk_bin/aarch64-linux-android24-clang" || ! -x "$ndk_bin/llvm-ar" ]]; then
    echo "Set NDK_HOME to an Android NDK with ARM64 Clang and llvm-ar." >&2
    return 1
  fi
  export CC_aarch64_linux_android="$ndk_bin/aarch64-linux-android24-clang"
  export AR_aarch64_linux_android="$ndk_bin/llvm-ar"

  if ! command -v dx >/dev/null || [[ "$(dx --version)" != dioxus\ 0.7.10* ]]; then
    echo "Dioxus CLI 0.7.10 is required." >&2
    return 1
  fi
  if ! rustup target list --installed | grep -Fxq aarch64-linux-android; then
    echo "Install the ARM64 Rust target: rustup target add aarch64-linux-android" >&2
    return 1
  fi

  tdlib="$repo_root/target/native/tdlib/arm64-v8a/libtdjson.so"
  if [[ ! -s "$tdlib" ]]; then
    "$repo_root/scripts/build-tdlib-android.sh"
  fi
}

android_prepare_generated() {
  local variant="$1"
  local main="$repo_root/target/dx/TuneStash/$variant/android/app/app/src/main"
  # Dioxus bundles once before our overlay. Remove files that conflict with
  # generated launcher icons or require Media3 before that initial build.
  for density in mdpi hdpi xhdpi xxhdpi xxxhdpi; do
    rm -f "$main/res/mipmap-$density/ic_launcher.png"
  done
  rm -f "$main/kotlin/dev/dioxus/main/PlayerService.kt"
}

android_overlay_generated() {
  local variant="$1" version_code="${2:-1}"
  local generated="$repo_root/target/dx/TuneStash/$variant/android/app"
  local main="$generated/app/src/main"
  if [[ ! -f "$generated/app/build.gradle.kts" || ! -f "$main/AndroidManifest.xml" ]]; then
    echo "Dioxus did not generate the expected $variant Android project: $generated" >&2
    return 1
  fi

  mkdir -p "$main/jniLibs/arm64-v8a" "$main/res/values-v31" "$main/res/drawable-nodpi"
  cp "$tdlib" "$main/jniLibs/arm64-v8a/libtdjson.so"
  cp "$repo_root/crates/music_app/android/PlayerService.kt" "$main/kotlin/dev/dioxus/main/PlayerService.kt"
  cp "$repo_root/crates/music_app/android/proguard-tunestash.pro" "$generated/app/proguard-tunestash.pro"
  cp "$repo_root/android/res/drawable/"*.xml "$main/res/drawable/"
  cp "$repo_root/android/res/drawable-nodpi/"*.png "$main/res/drawable-nodpi/"
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
if 'android:theme="@style/AppTheme"' in manifest:
    manifest = manifest.replace('android:theme="@style/AppTheme"', 'android:theme="@style/TuneStashTheme"', 1)
elif 'android:theme="@style/TuneStashTheme"' not in manifest:
    raise SystemExit('Could not locate generated Android theme')
for permission in ('FOREGROUND_SERVICE', 'FOREGROUND_SERVICE_MEDIA_PLAYBACK'):
    declaration = f'<uses-permission android:name="android.permission.{permission}" />'
    if declaration not in manifest:
        manifest = manifest.replace('<application ', declaration + '\n    <application ', 1)
service = '''<service android:name="dev.dioxus.main.PlayerService"
            android:exported="true" android:foregroundServiceType="mediaPlayback">
            <intent-filter>
                <action android:name="androidx.media3.session.MediaSessionService" />
            </intent-filter>
        </service>'''
if 'android:name="dev.dioxus.main.PlayerService"' not in manifest:
    manifest = manifest.replace('</application>', service + '\n    </application>', 1)
path.write_text(manifest)
PY

  python3 - "$generated/app/build.gradle.kts" "$variant" "$version_code" "$repo_root/Cargo.toml" <<'PY'
from pathlib import Path
import re
import sys
import tomllib

path, variant, version_code, cargo_path = map(Path, sys.argv[1:])
gradle = path.read_text()
gradle = re.sub(r'^\s*implementation\("androidx\.media3:media3-(?:exoplayer|session):[^\"]+"\)\s*\n', '', gradle, flags=re.MULTILINE)
gradle = gradle.replace('compileSdk = 34', 'compileSdk = 35', 1)
if 'dependencies {' not in gradle:
    raise SystemExit('Generated Gradle file has no dependencies block')
gradle = gradle.replace('dependencies {', '''dependencies {
    implementation("androidx.media3:media3-exoplayer:1.6.1")
    implementation("androidx.media3:media3-session:1.6.1")''', 1)
if str(variant) == 'release':
    version = tomllib.loads(cargo_path.read_text())['workspace']['package']['version']
    for field, value in [('versionCode', version_code), ('versionName', f'"{version}"')]:
        gradle, count = re.subn(rf'(?m)^\s*{field}\s*=\s*[^\n]+', f'        {field} = {value}', gradle, count=1)
        if count != 1:
            raise SystemExit(f'Generated Gradle file lacks {field}')
    if 'isShrinkResources = true' not in gradle:
        gradle, count = re.subn(r'(getByName\("release"\)\s*\{\s*isMinifyEnabled\s*=\s*true)', r'\1\n            isShrinkResources = true', gradle, count=1)
        if count != 1:
            raise SystemExit('Generated Gradle file lacks minified release build type')
    # The generated AGP version's lint-gradle artifact is unavailable in this
    # environment. Lint is not needed to assemble an APK; keep the build's
    # separate Rust checks and APK verification as explicit gates.
    if 'lint { checkReleaseBuilds = false }' not in gradle:
        gradle = gradle.replace('android {', 'android {\n    lint { checkReleaseBuilds = false }', 1)
path.write_text(gradle)
PY
}
