#!/usr/bin/env bash
set -euo pipefail
trap 'echo "Android release build failed near line $LINENO" >&2' ERR

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=scripts/android-build-common.sh
source "$repo_root/scripts/android-build-common.sh"

usage() {
  cat <<'EOF'
Usage: scripts/build-android-release.sh [--init-key] [--version-code N]

  --init-key         Create the signing key and password file once; do not build.
  --version-code N   Positive Android update number (default: 1).

The key defaults to ~/.local/share/tunestash/release.p12 (or XDG_DATA_HOME).
Set TUNESTASH_RELEASE_KEYSTORE and TUNESTASH_RELEASE_PASSWORD_FILE to use
different paths. Back up both files: later APK updates require the same key.
EOF
}

init_key=false
version_code=1
while (( $# )); do
  case "$1" in
    --init-key) init_key=true; shift ;;
    --version-code)
      if (( $# < 2 )); then usage >&2; exit 2; fi
      version_code="$2"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    *) usage >&2; exit 2 ;;
  esac
done
if [[ ! "$version_code" =~ ^[1-9][0-9]*$ ]] || (( ${#version_code} > 9 )); then
  echo "--version-code must be a positive Android integer." >&2
  exit 2
fi

signing_dir="${XDG_DATA_HOME:-$HOME/.local/share}/tunestash"
keystore="${TUNESTASH_RELEASE_KEYSTORE:-$signing_dir/release.p12}"
password_file="${TUNESTASH_RELEASE_PASSWORD_FILE:-$signing_dir/release.pass}"
if [[ "$keystore" == "$password_file" ]]; then
  echo "The keystore and password file must have different paths." >&2
  exit 2
fi
python3 - "$repo_root" "$keystore" "$password_file" <<'PY'
from pathlib import Path
import sys
repo = Path(sys.argv[1]).resolve()
for name in sys.argv[2:]:
    path = Path(name).expanduser().resolve()
    if path.is_relative_to(repo):
        raise SystemExit(f'Signing material must be outside the repository: {path}')
PY

export JAVA_HOME="${JAVA_HOME:-/home/mohy/Programs/tunestash-jdk17}"
export PATH="$JAVA_HOME/bin:$PATH"
if [[ ! -x "$JAVA_HOME/bin/keytool" ]]; then
  echo "Set JAVA_HOME to a JDK 17 installation with keytool." >&2
  exit 1
fi

if "$init_key"; then
  if [[ -e "$keystore" || -e "$password_file" ]]; then
    echo "Signing material already exists. Refusing to replace the release key." >&2
    exit 1
  fi
  umask 077
  mkdir -p "$(dirname "$keystore")" "$(dirname "$password_file")"
  python3 - "$password_file" <<'PY'
from pathlib import Path
import secrets
import sys
path = Path(sys.argv[1])
with path.open('x') as output:
    output.write(secrets.token_urlsafe(48) + '\n')
PY
  if ! "$JAVA_HOME/bin/keytool" -genkeypair -storetype PKCS12 \
      -keystore "$keystore" -alias tunestash -keyalg RSA -keysize 3072 \
      -validity 10000 -dname 'CN=TuneStash Test Distribution' \
      -storepass:file "$password_file" -keypass:file "$password_file"; then
    rm -f "$password_file" "$keystore"
    exit 1
  fi
  chmod 600 "$keystore" "$password_file"
  printf 'Created release key: %s\nPassword file: %s\nBack up both files securely before sharing APKs.\n' "$keystore" "$password_file"
  exit 0
fi

if [[ ! -s "$keystore" || ! -s "$password_file" ]]; then
  echo "Release signing key is missing. Run $0 --init-key first." >&2
  exit 1
fi
android_build_setup

sdk_build_tools="$(find "$ANDROID_HOME/build-tools" -mindepth 1 -maxdepth 1 -type d | sort -V | tail -n 1)"
for tool in zipalign apksigner aapt; do
  if [[ ! -x "$sdk_build_tools/$tool" ]]; then
    echo "Android SDK build-tools need $tool." >&2
    exit 1
  fi
done

cd "$repo_root"
android_prepare_generated release
dx bundle --android --release --target aarch64-linux-android --package-types apk --package music_app --locked
android_overlay_generated release "$version_code"

generated="$repo_root/target/dx/TuneStash/release/android/app"
( cd "$generated" && ./gradlew :app:assembleRelease )
unsigned="$generated/app/build/outputs/apk/release/app-release-unsigned.apk"
if [[ ! -s "$unsigned" ]]; then
  echo "Gradle did not produce the expected unsigned release APK: $unsigned" >&2
  exit 1
fi

version="$(python3 - "$repo_root/Cargo.toml" <<'PY'
from pathlib import Path
import sys
import tomllib
print(tomllib.loads(Path(sys.argv[1]).read_text())['workspace']['package']['version'])
PY
)"
dist="$repo_root/dist"
mkdir -p "$dist"
output="$dist/TuneStash-$version-$version_code-arm64-v8a.apk"
workdir="$(mktemp -d "$repo_root/target/tunestash-release.XXXXXX")"
trap 'rm -rf "$workdir"' EXIT
"$sdk_build_tools/zipalign" -P 16 -f 4 "$unsigned" "$workdir/aligned.apk"
"$sdk_build_tools/apksigner" sign --ks "$keystore" --ks-key-alias tunestash \
  --ks-pass "file:$password_file" --debuggable-apk-permitted false \
  --out "$workdir/signed.apk" "$workdir/aligned.apk"
"$sdk_build_tools/zipalign" -c -P 16 4 "$workdir/signed.apk"
"$sdk_build_tools/apksigner" verify --verbose --print-certs "$workdir/signed.apk"

python3 - "$workdir/signed.apk" "$sdk_build_tools/aapt" "$version" "$version_code" <<'PY'
import subprocess
import sys
from zipfile import ZipFile

apk_path, aapt, version, version_code = sys.argv[1:]
with ZipFile(apk_path) as apk:
    bad = apk.testzip()
    if bad:
        raise SystemExit(f'APK archive is corrupt at {bad}')
    libs = {name for name in apk.namelist() if name.startswith('lib/') and name.endswith('.so')}
    expected = {'lib/arm64-v8a/libmain.so', 'lib/arm64-v8a/libtdjson.so'}
    if libs != expected:
        raise SystemExit(f'Unexpected native libraries: {sorted(libs)}')
badging = subprocess.check_output([aapt, 'dump', 'badging', apk_path], text=True)
package = f"package: name='com.tunestash.app' versionCode='{version_code}' versionName='{version}'"
if package not in badging:
    raise SystemExit('Release APK package ID or version differs from the requested values')
if 'application-debuggable' in badging:
    raise SystemExit('Release APK is debuggable')
PY

mv -f "$workdir/signed.apk" "$output"
( cd "$dist" && sha256sum "$(basename "$output")" > "$(basename "$output").sha256" )
printf 'Release APK: %s\nSHA-256: %s\n' "$output" "$output.sha256"
