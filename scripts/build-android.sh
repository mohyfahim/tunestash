#!/usr/bin/env bash
set -euo pipefail
trap 'echo "Android debug build failed near line $LINENO" >&2' ERR

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=scripts/android-build-common.sh
source "$repo_root/scripts/android-build-common.sh"
android_build_setup

cd "$repo_root"
android_prepare_generated debug
dx bundle --android --target aarch64-linux-android --package-types apk --package music_app --locked
android_overlay_generated debug

generated="$repo_root/target/dx/TuneStash/debug/android/app"
( cd "$generated" && ./gradlew :app:assembleDebug )

apk="$generated/app/build/outputs/apk/debug/app-debug.apk"
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
