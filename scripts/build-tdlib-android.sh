#!/usr/bin/env bash
set -euo pipefail

# Pinned native inputs. The resulting binary is build output, never committed.
td_commit=42e6a5259551178d1dab54a22ad96d14bd906e20
openssl_tag=openssl-3.0.17
openssl_commit=4d7dfa23be028f11e0f9189a7512a872c05938fa
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
native_root="$repo_root/target/native"
source_root="$native_root/sources"
build_root="$native_root/build"
output="$native_root/tdlib/arm64-v8a/libtdjson.so"
jobs="${NATIVE_BUILD_JOBS:-2}"

: "${NDK_HOME:?Set NDK_HOME to the installed Android NDK}"
command -v cmake >/dev/null
command -v ninja >/dev/null
if ! command -v gperf >/dev/null; then
  echo "Install gperf to generate TDLib sources." >&2; exit 1
fi
command -v perl >/dev/null
mkdir -p "$source_root" "$build_root" "$(dirname "$output")"

if [[ ! -d "$source_root/td/.git" ]]; then
  git init -q "$source_root/td"
  git -C "$source_root/td" fetch --depth 1 https://github.com/tdlib/td.git "$td_commit"
  git -C "$source_root/td" checkout -q --detach FETCH_HEAD
fi
if [[ "$(git -C "$source_root/td" rev-parse HEAD)" != "$td_commit" ]]; then
  echo "TDLib source commit differs from the pinned revision" >&2; exit 1
fi
if [[ ! -d "$source_root/openssl/.git" ]]; then
  git clone -q --depth 1 --branch "$openssl_tag" https://github.com/openssl/openssl.git "$source_root/openssl"
fi
if [[ "$(git -C "$source_root/openssl" rev-parse HEAD)" != "$openssl_commit" ]]; then
  echo "OpenSSL source commit differs from the pinned revision" >&2; exit 1
fi

if [[ ! -f "$build_root/openssl/lib/libcrypto.a" ]]; then
  (
    cd "$source_root/openssl"
    export ANDROID_NDK_ROOT="$NDK_HOME"
    export PATH="$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin:$PATH"
    ./Configure android-arm64 -D__ANDROID_API__=24 no-shared no-tests --prefix="$build_root/openssl"
    make depend -s
    make -j"$jobs" install_sw
  )
fi

cmake -S "$source_root/td" -B "$build_root/td-host" -GNinja \
  -DTD_GENERATE_SOURCE_FILES=ON -DGPERF_EXECUTABLE="$(command -v gperf)" -DCMAKE_BUILD_TYPE=Release
cmake --build "$build_root/td-host" -j"$jobs"
cmake -S "$source_root/td" -B "$build_root/td-arm64" -GNinja \
  -DCMAKE_TOOLCHAIN_FILE="$NDK_HOME/build/cmake/android.toolchain.cmake" \
  -DANDROID_ABI=arm64-v8a -DANDROID_PLATFORM=android-24 -DANDROID_STL=c++_static \
  -DOPENSSL_INCLUDE_DIR="$build_root/openssl/include" \
  -DOPENSSL_CRYPTO_LIBRARY="$build_root/openssl/lib/libcrypto.a" \
  -DOPENSSL_SSL_LIBRARY="$build_root/openssl/lib/libssl.a" \
  -DCMAKE_BUILD_TYPE=Release
cmake --build "$build_root/td-arm64" --target tdjson -j"$jobs"
"$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-strip" \
  --strip-debug --strip-unneeded "$build_root/td-arm64/libtdjson.so" -o "$output"
echo "TDLib Android library: $output"
