# Android setup

TuneStash is a two-crate Rust workspace. `music_core` builds on a host without
Android dependencies. The `music_app` binary is enabled by its `mobile` feature
and uses the Dioxus 0.7.10 Android WebView renderer.

## Prerequisites

- Rust 1.98.1, as selected by `rust-toolchain.toml`.
- Dioxus CLI 0.7.10 (`dx --version`). Install with
  `cargo install dioxus-cli --version 0.7.10 --locked` if absent.
- JDK 17, Android SDK platforms 34 and 35, the NDK, and Android SDK build tools.
- ARM64 Rust target: `rustup target add aarch64-linux-android`.
- CMake, Ninja, Perl, `make`, and `gperf` for the pinned TDLib/OpenSSL build.

## Telegram app credentials

Create a Telegram application at [my.telegram.org](https://my.telegram.org),
then copy `.env.example` to `.env` and fill in `TELEGRAM_API_ID` and
`TELEGRAM_API_HASH`. The build reads this ignored file. Never commit it or
enter authorization codes or passwords outside the Android app. These app
credentials are compiled into the independent Telegram client.

Set these variables to your local installations before running `dx`:

```sh
export JAVA_HOME=/path/to/jdk-17
export ANDROID_HOME=/path/to/Android/Sdk
export NDK_HOME="$ANDROID_HOME/ndk/<installed-version>"
export ANDROID_NDK_HOME="$NDK_HOME"
export PATH="$JAVA_HOME/bin:$ANDROID_HOME/platform-tools:$PATH"
```

From the workspace root, verify the host crates and build a debug ARM64 APK:

```sh
cargo fmt --all -- --check
cargo check --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --all-targets --locked
cargo check -p music_app --target aarch64-linux-android --features mobile --locked
./scripts/build-android.sh
```

The script checks JDK 17, SDK platforms 34 and 35, the NDK, Rust's ARM64 target, and
Dioxus CLI 0.7.10. On the first build it fetches pinned TDLib and OpenSSL
sources into ignored `target/native/`, builds ARM64 `libtdjson.so`, then bundles
the APK. Later builds reuse the native library. It overlays the source-controlled
splash resources, native library, Media3 dependencies, and playback service onto Dioxus's generated Gradle project,
assembles the final APK, and prints its path.

The debug APK is at
`target/dx/TuneStash/debug/android/app/app/build/outputs/apk/debug/app-debug.apk`.
Generated build files and the APK live under the ignored `target/` tree. This
is a development package. To install and test it on a connected Android phone:

```sh
adb devices -l
./scripts/install-android.sh
adb shell pidof com.tunestash.app
adb exec-out screencap -p > tunestash-screen.png
```

The install script selects the only authorized device automatically when no
serial is passed. For multiple devices, pass the serial as its argument or set
`ANDROID_SERIAL` in the environment; set `ANDROID_USER_ID` to install into a
profile other than user 0. The script launches the app after installation. The
smoke test should show `Status: ok`, a live process, and the TuneStash splash
mark followed by the login screen. An already authorized session opens Music
Sources after chat discovery completes.

The login, input, and source screens are in `music_app::ui`; their CSS is in
`crates/music_app/assets/style.css`. TDLib runs on a dedicated Rust thread,
and Dioxus observes authorization snapshots. Android Keystore protects the
local TDLib database key. Host tests do not prove a real account login; verify
that separately on an Android device.
The Cargo binary is named `TuneStash` because Dioxus derives the Android
launcher label from that name.

## Shareable ARM64 release APK

Use the same prerequisites and `.env` as the debug build. Initialize the release
signing key once, then build an APK for direct installation by testers:

```sh
./scripts/build-android-release.sh --init-key
./scripts/build-android-release.sh --version-code 1
```

The first command creates `release.p12` and `release.pass` under
`~/.local/share/tunestash/` (or `$XDG_DATA_HOME/tunestash/`). Both files are
outside this repository, restricted to your user account, and required for
future APK updates. Back up **both** securely before distributing the first
APK. The script refuses to replace an existing key. To use different paths,
set `TUNESTASH_RELEASE_KEYSTORE` and `TUNESTASH_RELEASE_PASSWORD_FILE`.
The password file is kept beside the key for unattended local builds, so
protect the directory and its backups together. Do not share either file with
testers. Only the APK and optional `.sha256` checksum go to them.

The build writes `dist/TuneStash-<version>-<version-code>-arm64-v8a.apk` and a
matching `.sha256` file. It uses the Cargo workspace version as Android
`versionName`. Increase `--version-code` for every subsequent build sent to
testers; keep the package ID and signing key the same so Android accepts
updates. The script checks the APK signature, alignment, non-debuggable
manifest, version, archive integrity, and bundled native libraries before
placing it in `dist/`. The release profile and Android code/resource shrinking
reduce size while preserving JNI entry points and the app's fonts and images.
Verify the checksum from the output directory with
`cd dist && sha256sum -c TuneStash-0.1.0-1-arm64-v8a.apk.sha256`.
The generated Gradle project's release lint task is disabled because its
`lint-gradle:31.7.0` dependency did not resolve in this build environment;
Rust checks and the artifact checks above still run separately.

Testers can open the APK on an ARM64 Android device and permit installation
from their chosen file-sharing app, or install it over USB with:

```sh
adb install -r dist/TuneStash-0.1.0-1-arm64-v8a.apk
```

The release APK has the same `com.tunestash.app` package ID as the debug APK
but a different signing key. Android cannot install it over an existing debug
installation. Uninstalling the debug app first removes its local Telegram
session, library, and downloads; Telegram messages remain on Telegram.
The app currently supports ARM64 devices only. Confirm sign-in, a selected
source, playback, and background media controls on a real device before
sharing a build as device-verified.
