# Implementation status

Updated: 2026-10-08

## Implemented

- One-screen TuneStash welcome page based on `concept/login.png`, with the
  TeleTune name changed and the demo button and pagination removed.
- Phone-number, verification-code, email-code, and Telegram two-step-password
  forms. TDLib drives the form shown for the current authorization state.
- An empty `#171a19` canvas after TDLib confirms authorization and `getMe`
  returns the account ID. A saved TDLib session can resume on app launch.
- Native TDLib C/JSON bridge on a dedicated Rust thread. Android Keystore wraps
  the local TDLib database key; app files remain under Android private storage.
- TuneStash launcher icon and Android splash resources, overlaid during the APK
  build. The build reads Telegram app credentials from ignored `.env`.

## Verification

- Host and Android Rust checks, Clippy, and host unit tests pass.
- The final APK builds with `libtdjson.so`, the TuneStash icon, and splash
  resources; archive integrity and APK v2 signature checks pass.
- Samsung SM-A256E, Android 14, ARM64: the final APK installs, launches, and
  shows the welcome and phone-number screens. The phone-number step is supplied
  by TDLib's live authorization state. An initial JNI class-loader crash was
  fixed during device review.
- The owner completed a live Telegram sign-in and reached the empty dark
  canvas. A cold relaunch returned to that canvas from the saved session.
- A cold-launch recording exposed a brief white WebView frame; native dark
  background and inline CSS removed the bright frame in a repeat recording.
- The password branch for accounts with Telegram two-step verification is
  implemented from TDLib's `authorizationStateWaitPassword` state. It was not
  separately exercised with a two-step-enabled account during this check.

Media indexing, playback, offline downloads, and the music-source selection
interface remain future work. The signed-in page is intentionally empty.
