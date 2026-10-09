# Implementation status

Updated: 2026-10-09

## Implemented

- One-screen TuneStash welcome page based on `concept/login.png`, with the
  TeleTune name changed and the demo button and pagination removed.
- Phone-number, verification-code, email-code, and Telegram two-step-password
  forms. TDLib drives the form shown for the current authorization state.
- A dark Music Sources screen after authorization. Its loading page reports
  chat discovery and bot-check progress; the source screen shows Saved
  Messages, created channels, confirmed music bots, selected chats, and
  searchable unselected chats from Main and Archive.
- Source choices persist by account and chat ID in SQLite. Selecting a regular
  chat moves it from All Chats to Selected Chats; selected default sources stay
  in Default Sources. Stored names and kinds remain visible while Telegram
  details are loading.
- A Change account confirmation starts TDLib logout, clears the account's
  source choices after TDLib closes, and creates a fresh TDLib client for the
  next login. It is only exposed during onboarding.
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
- Host source-choice and classification tests pass. Android-target Clippy and
  the final APK build pass with bundled SQLite.
- Samsung SM-A256E, Android 14: the final APK installed and resumed the owner's
  existing Telegram session. Live discovery finished, displayed hundreds of
  chats, identified user-created channels and music-sending bots, and showed
  the loading, recovery, and three-section source page. Nine chats could not
  be checked on this account; the page reports the gap and offers Retry.
- On that device, a temporary Saved Messages selection survived a force-stop
  and cold relaunch. A temporary regular chat moved from All Chats to Selected
  Chats on selection. Both test choices were returned to their original off
  state. The Change account confirmation opened and canceled correctly.
  Actual account logout remains unverified to avoid clearing the
  owner's active session and local Telegram data.
- The password branch for accounts with Telegram two-step verification is
  implemented from TDLib's `authorizationStateWaitPassword` state. It was not
  separately exercised with a two-step-enabled account during this check.

Media indexing, playback, offline downloads, the Settings entry point, and a
light theme remain future work. Source discovery checks message metadata only;
it does not add tracks to the library.

## Rust learning note

`SourceSnapshot` is owned by the service thread and delivered to Dioxus through
a Tokio watch channel. SQLite writes happen on that thread, so UI rendering
only reads immutable snapshots. Account and chat IDs stay as `i64` throughout
the Rust path. TDLib responses are correlated with request IDs; a bounded job
queue checks chats without starting one request per row at once.
