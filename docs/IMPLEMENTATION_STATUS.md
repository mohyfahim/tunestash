# Implementation status

Updated: 2026-10-09

## Implemented

- One-screen TuneStash welcome page based on `concept/login.png`, with the
  TeleTune name changed and the demo button and pagination removed.
- Phone-number, verification-code, email-code, and Telegram two-step-password
  forms. TDLib drives the form shown for the current authorization state.
- A dark Music Sources screen after authorization. Its loading page reports
  chat discovery and music-check progress. Telegram Audio and Documents
  identified as playable audio are checked throughout each accessible chat's
  searchable history, following TDLib pagination. Voice notes, links, and
  text-only messages do not qualify. Only confirmed music sources appear in
  Default Sources, Selected Chats, or searchable All Chats.
- Main and Archive chat loading discovers candidates, including archived
  chats. The candidate IDs come from TDLib's `getChats` results for those two
  lists; other cached chat objects are excluded. Some accessible channels on
  the test account have empty per-chat list fields, so those fields are not
  used as the membership check. Inaccessible unselected chats stay hidden;
  access failures for saved selections keep their choice intact and can be
  retried.
- Source choices persist by account and chat ID in SQLite. Selecting a regular
  chat moves it from All Chats to Selected Chats; selected default sources stay
  in Default Sources. A complete music check that finds no qualifying file
  disables a previously saved choice; failed checks preserve it and keep the
  chat hidden until verification succeeds. New music messages and relevant
  edits or deletions affect eligibility during an active scan.
- A successful source scan now saves the discovered catalog in SQLite. Later
  launches load that catalog without scanning Telegram; live source updates are
  ignored until the user taps Resync. Resync runs a full scan and replaces the
  saved catalog only after success. A failed resync restores the last saved
  list. Existing installations perform one scan to create this new catalog.
- The Sources heading has compact Resync and Next controls. Next is available
  after a complete scan, including one with failed chat checks or no selected
  sources. It saves an account-scoped setup marker and opens a blank Library
  page. Later launches open Library; Back to Sources opens Sources temporarily.
  Logout removes the marker, and a failed save keeps Sources visible.
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
- Host tests cover the v1-to-v2 setup-marker migration, account isolation,
  completion with partial failures and no selection, persistence after a fresh
  authorization, failed writes, and Library/Back routing.
- The updated APK built and installed on Samsung SM-A256E. On the device, the
  44px-tall Resync and Next buttons aligned with Music Sources without wrapping.
  With no sources selected, Next opened the blank Library page; Back returned
  to Sources, and a cold relaunch opened Library again.
- Host tests cover audio and Document eligibility, sender independence,
  pagination, section placement, account-scoped persistence, and the different
  persistence outcomes for empty and failed scans. The Android target passes
  Clippy with warnings denied and the APK builds successfully.
- An earlier fix kept background checks from reopening the checking-music
  banner. Its APK was installed on Samsung SM-A256E; after the 636-chat scan
  finished, the banner was absent on two device checks. The current build
  instead waits for a manual Resync after a successful scan.
- Host tests cover catalog migration, atomic replacement, cold-launch restore,
  and manual resync. The new APK was installed on Samsung SM-A256E. A cold
  relaunch showed the saved source page and an enabled Resync button immediately,
  with no automatic checking banner. Tapping Resync advanced the on-device
  source database's modification time, confirming the manual scan saved a new
  result. The scan progress itself was too brief to capture in a screenshot.
- Samsung SM-A256E, Android 14: the music-only APK installed and scanned live
  Telegram history progressively. Saved Messages, Gym Musics (a created
  channel), and Spotify Save Bot appeared after music was confirmed. The
  scan's live progress count and three-section page were visible. An initial
  broad scan included over 3,000 cached chat objects; the corrected Main and
  Archive list query reduced candidates to 636 on this account. The corrected
  scan completed without a failure banner, and searching All Chats for
  GeekACK returned no match. Saved Messages and a selected regular chat were
  still selected after reinstalling the corrected APK and rescanning.
- Samsung SM-A256E, Android 14: the final APK installed and resumed the owner's
  existing Telegram session. Live discovery finished, displayed hundreds of
  chats, identified user-created channels and music-sending bots, and showed
  the loading, recovery, and three-section source page. An initial scan left
  nine bot document histories unchecked because pagination supplied an invalid
  message ID. Discovery now uses TDLib's returned `next_from_message_id`;
  a repeat scan on the device finished without failed checks. Failed checks,
  if any occur later, can be expanded to show the chat and Telegram error.
- Source discovery now emits aggregate Android timing logs. Two 636-chat
  resyncs took 80 and 83 seconds with one-item Audio pages; empty paginated
  results caused up to 199 requests for one chat. A 50-item Audio page reduced
  a repeat scan to 20 seconds with the same 123 Audio and 14 Document sources
  found. Measurements and the logcat command are in
  [SOURCE_SCAN_PERFORMANCE.md](SOURCE_SCAN_PERFORMANCE.md).
- On that device, a temporary Saved Messages selection survived a force-stop
  and cold relaunch. A temporary regular chat moved from All Chats to Selected
  Chats on selection. Both test choices were returned to their original off
  state. The Change account confirmation opened and canceled correctly.
  Actual account logout remains unverified to avoid clearing the
  owner's active session and local Telegram data.
- The password branch for accounts with Telegram two-step verification is
  implemented from TDLib's `authorizationStateWaitPassword` state. It was not
  separately exercised with a two-step-enabled account during this check.

## Library increment

- The Library now uses the dark login and Music Sources palette with the mobile
  layout from `concept/library.png`. Home and Search are black placeholders
  with persistent dark bottom navigation and dark Android system bars.
  Category, download, and sorting controls are displayed without active
  behavior. The mini-player stays hidden until playback exists. The icon-only
  Reindex action sits in the Library title row and is disabled without selected
  sources or while indexing.
- Selected Telegram sources receive metadata-only Audio and playable audio
  Document scans. Indexed rows persist by account, chat, and message ID;
  per-filter cursors and each result page commit together. A completed scan
  reconciles removed messages. A completed catalog stays unchanged on launch;
  first scans and interrupted scans run automatically, while Reindex starts a
  new full pass. Live new-message, edit, and deletion updates
  refresh the catalog while the app is open. Source discovery checks Document
  history after an empty Audio search so document-only chats can qualify.
- The list shows the current matching track count, title, artist, source, and
  embedded minithumbnail where available. Tracks are ordered by displayed
  title; the A–Z rail filters case-insensitively by its first letter, and
  tapping the active letter restores all tracks. Persian and numeric titles
  stay in the full list. Missing artwork uses the TuneStash music mark. The
  service thread owns SQLite and publishes paginated UI snapshots.

Host tests cover parsing, account/source filtering, first indexing, cursor
resume, completed-catalog startup, manual Reindex, duplicate prevention,
reconciliation, title ordering, pagination, and active-letter filtering.
Formatting, host tests, host Clippy, Android-target Rust checks, and the APK
build pass. The updated APK installed on Samsung SM-A256E (Android 14). Its
populated dark Library showed 383 tracks in title order and the icon-only
Reindex action. A, B, Q, and X filters showed 15, 22, 1, and 0 matching tracks;
tapping an active letter restored the full list. The rail stayed visible while
scrolling, and its lower letters were reachable by scrolling the rail. A cold
relaunch restored the 383-track catalog. A live Reindex was not exercised.
No private account database was inspected.
Playback, streaming, offline downloads, and the Settings entry point remain
future work.

## Rust learning note

`SourceSnapshot` is owned by the service thread and delivered to Dioxus through
a Tokio watch channel. SQLite writes happen on that thread, so UI rendering
only reads immutable snapshots. Account and chat IDs stay as `i64` throughout
the Rust path. TDLib responses are correlated with request IDs; a bounded job
queue checks chats without starting one request per row at once.
