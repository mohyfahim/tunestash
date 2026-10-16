# Implementation status

Updated: 2026-10-15

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
- Source discovery now shows a lime progress bar during music checks and an
  indeterminate bar while Telegram loads chats. Default Sources shows its actual
  count and every discovered default source, rather than initially hiding all
  but five. Setup can continue as soon as one playable source is found; the
  full scan continues in the service runtime and is still required before its
  completed catalog is saved.
- Private chats are checked for music before looking up their user type, so
  empty chats no longer need that extra Telegram request. Found bot and
  channel chats are classified promptly. Classification failures retry twice,
  then appear as a retryable discovery gap instead of silently saving an
  incomplete source type.
- The updated debug APK installed on Samsung SM-A256E (Android 14). The source
  page displayed all six default sources. A manual resync showed the lime bar
  at 99 of 620 chats checked and completed at 635 of 635 with no failed checks;
  the page again displayed six default sources. This verifies the resync UI and
  classification outcome on that account. A fresh-account first scan has not
  been timed on the device.
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
  sources. It saves an account-scoped setup marker and opens the music shell on
  Home. Later launches open Home; its gear opens Settings.
  Logout removes the marker, and a failed save keeps Sources visible.
- A Change account confirmation starts TDLib logout, clears the account's
  source choices after TDLib closes, and creates a fresh TDLib client for the
  next login. The later Account increment exposes this action from Settings
  and expands cleanup to all connected-account data.
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
  layout from `concept/library.png`. At this increment Search was a black placeholder with
  persistent dark bottom navigation and dark Android system bars.
  Category and sorting controls are displayed without active
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
Settings and storage management were added in the later increment below.

## Home increment

- Home is the default tab after source setup. Its dark layout follows
  `concept/home.png`: recent cover carousel, listening and playlist sections,
  and the three-tab bottom navigation. The gear now opens Settings, which
  links to Music Sources and Storage management.
- Recently Added shows up to 12 real tracks from enabled sources, ordered by
  Telegram message date with stable chat/message tie breaks. It uses TDLib's
  embedded cover minithumbnail when available and the TuneStash mark otherwise.
  See all opens Library in recent order; tapping the Library tab restores title
  order. Sorting and source filtering remain on the service thread.
- Listening history and playlists have truthful empty states. Their actions
  stay hidden until those capabilities exist. No sample tracks are shown.

Host tests cover recent ordering and stable ties, account and enabled-source
filtering, arrival of tracks during indexing, and title/recent sort switching.
Mobile-feature tests and Clippy pass; the Android ARM64 Rust check and debug APK
build pass. The APK installed and launched on Samsung SM-A256E (Android 14).
On that phone, Home displayed recent tracks from the saved Telegram session,
including mixed Persian/English titles and artwork fallbacks. The gear opened
Sources, Next returned to Home, See all opened 383 tracks in recent order, the
Library tab restored title order, and a cold launch returned to Home. Captures
at the device's normal width and a temporary 960px physical width showed no
clipping; the phone's original display size was restored. None of the twelve
recent tracks had an embedded cover, so the Home cover-image path was not
visually checked on this device.

## Main player increment

- Tapping an indexed Library track opens the full-screen player and starts
  Media3 playback. The Song view follows `concept/main-player.png` with cover
  art or the TuneStash fallback, metadata, favorite, seek, transport, shuffle,
  repeat, and offline status. The Queue view lists the full enabled-source
  library in the active title or recent order, including unloaded pages, and
  allows direct selection. Closing the player leaves a persistent mini-player.
  Add to Playlist, Go to Album, Share, and More are visibly unavailable.
- Rust owns queue order, position, repeat mode, and shuffle. Off advances once;
  repeat queue wraps; single play stops at the end of the selected track.
  State survives process restart without autoplay. Android's
  `MediaSessionService` owns ExoPlayer, media session, audio focus, and the
  foreground notification. Its native bootstrap can start the Rust worker
  without an Activity. iOS reports playback unavailable.
- TDLib resolves each track by account, chat, and message ID. Media3 reads
  verified available ranges through a native data source while TDLib continues
  the full download. Explicitly requested offline files are size-checked,
  copied atomically to app-private storage, and used for offline replay.
  Media3's decoded embedded
  cover art replaces a small Telegram thumbnail when available. The database keeps
  favorites, playback state, incomplete download records, and saved-file
  records. Interrupted transfers are retried at runtime startup. Missing
  saved files are invalidated; unavailable messages and buffering failures
  show errors. The active queue removes tracks from sources that become
  unavailable and stops if the selected track is removed.
  Confirmed account logout stops playback and deletes that account's media.

Host tests cover queue ordering beyond the Library page, playback modes,
persistence without autoplay, favorites, download recovery records, source
reconciliation, saved-file validation, range reads, and incomplete-copy
rejection. Formatting, workspace check, workspace Clippy,
workspace tests, and Android ARM64 Clippy pass. Kotlin compilation and a
diagnostic APK assembly passed using the locally installed SDK 34 with the
Media3 SDK metadata check disabled. The tracked build requires SDK 35 for
Media3 1.6.1; SDK 35 is absent on this machine, so the standard final APK
build remains unverified.

On Samsung SM-A256E (Android 14), the diagnostic APK opened and streamed a
Library track, decoded audio before the download finished, saved it, and
displayed all 383 tracks in Queue. A cold relaunch restored the mini-player
without autoplay; Play resumed the saved track at its prior position. The
saved track advanced in airplane mode, and connectivity was restored. A
saved track also advanced with the screen off; a simulated headset media key
paused it. Android's notification card showed title, artwork, seek, and
transport buttons; tapping Next selected the next Rust queue entry. The
player fit the device's normal 1080 px width and a temporary 960 px width
without clipped controls; the original display size was restored. A saved
track's full embedded cover appeared sharply in the player after playback.
The Activity was force-stopped and `PlayerService` started alone; the process
created `tunestash-tdlib` worker threads. Stopping that service, relaunching
the Activity, and pressing Play recreated the session and resumed playback.
Audio interruption handling, source removal while playing, a lock-screen
card, and actual account logout remain unverified on device.

## Rust learning note

`SourceSnapshot` is owned by the service thread and delivered to Dioxus through
a Tokio watch channel. SQLite writes happen on that thread, so UI rendering
only reads immutable snapshots. Account and chat IDs stay as `i64` throughout
the Rust path. TDLib responses are correlated with request IDs; a bounded job
queue checks chats without starting one request per row at once.

## Settings and storage increment

- The Home gear opens Settings with Music Sources and Storage management.
  The latter measures app-private data in the background and shows offline
  downloads, Telegram file cache, and other app data. Saved songs are listed
  by size, 50 at a time, with individual removal and a confirmed bulk clear.
  Sources returns to Settings after setup rather than reopening onboarding.
- Stream only is stored per account and blocks explicit save commands from the
  player and Library. Existing offline files remain playable. Ordinary range
  playback no longer creates a pinned copy; TDLib still uses temporary disk
  bytes, which are requested for cleanup when playback ends or changes and on
  startup. The v6 migration marks ambiguous pre-upgrade pending transfers as
  failed so they cannot silently become offline files.
- Per-song removal stops that song if selected, deletes its pinned file, and
  retains library and queue metadata. Bulk clear stops playback, cancels
  pending work, removes offline files, and requests TDLib cache optimization.
  Generation-tagged copies cannot restore a canceled download. Filesystem and
  TDLib errors remain visible, and figures are rescanned after operations.
- Host tests cover ordinary playback versus explicit saving, Stream only
  persistence and blocking, late copy rejection, bulk clear, sorted storage
  rows, account isolation, and failed file deletion. Workspace tests and
  Clippy, plus an Android ARM64 Rust check, pass. The tracked APK build remains
  blocked by the missing Android SDK 35 platform JAR. A diagnostic APK using
  SDK 34 and skipping Media3 AAR metadata validation built and installed.
- On Samsung SM-A256E (Android 14), Settings, Storage management, both delete
  confirmations, the main-player Saved action sheet, and return navigation
  from Sources were reviewed. Storage showed 86.5 MB total,
  including five offline songs (27.3 MB). No saved songs or cache were deleted
  during device review, so physical-device deletion and cache cleanup remain
  unverified. Layouts fit the normal 1080 px width and temporary 960 px width;
  the original width was restored.

This increment keeps storage policy on the Rust service thread and publishes
immutable snapshots to Dioxus. Filesystem measurement uses a separate worker;
the playback worker coordinates TDLib file IDs, pending transfers, and
generation-tagged file copies before changing offline state.

## Search increment

- Search now uses the dark app palette and the track-focused composition from
  `concept/search.png`. It searches only indexed music from enabled sources,
  with live results, result counts, pagination, artwork fallback, source names,
  and indexing, empty, and read-error states. Its starting view shows up to
  eight account-scoped recent searches and real recently added tracks. The
  unsupported content-type and source filters and fictional popularity list
  from the concept are omitted.
- SQLite v7 adds normalized title, artist, and filename fields and backfills
  existing indexed tracks. Imports and edits maintain them. Queries match
  literal case-insensitive terms across fields after Arabic/Persian kaf and
  yeh normalization and whitespace folding; original metadata stays intact.
  Title matches rank ahead of artist and filename matches. Searching and
  recent-query persistence run on the service thread and publish a separate
  snapshot, leaving Library sorting and letter filtering unchanged.
- A submitted query or played result saves a deduplicated recent search; typing
  alone does not. Clear removes this account's search history, and logout
  removes it with other account data. Playing a result uses the existing full
  library queue in title order. Search shares the dark Android status bar
  appearance with the player and other screens.

Host tests cover normalization, v6 backfill, original metadata, ranking,
pagination, enabled-source and account isolation, live catalog changes,
recent-query bounds and deduplication, and account clearing. All 55 app and
7 core tests pass. Host Clippy passes with the pre-existing
`clippy::manual_checked_ops` warnings in `ui/storage.rs` suppressed; the
unmodified strict workspace command still reports those two warnings. The
SDK 35 Android APK built successfully and installed on Samsung SM-A256E
(Android 14). On that device, `Voice` returned three indexed tracks, submitting
it saved a recent search that survived reinstall, tapping that history entry
restored the results, and tapping a result opened the player with all 383
library tracks in Queue. After the dark-theme update, the Search start and
results views displayed Persian and English titles without clipping at the
phone's normal narrow width, with the keyboard open and closed. The status bar
matched the other dark screens. Playback was paused after
the handoff check. Device checks did not exercise a Persian variant query,
live import during an active search, or the error and empty-library states;
those remain covered by host logic tests or UI code only.

The Rust core owns normalization, while SQLite stores derived search fields
alongside untouched display strings. A pending query deadline coalesces typing
on the service thread; Dioxus consumes immutable search snapshots and never
queries SQLite during rendering. Search currently scans normalized fields
within the selected account rather than using full-text indexing, so very
large-library query latency has not been measured.

## Typography, listening history, and account increment

- JetBrains Mono is bundled for Latin interface text; Vazirmatn supplies
  Persian and Arabic glyphs. The generated lime and graphite Telegram-plane
  and music-note mark appears on welcome, login, Sources, launcher, and splash.
  Track artwork continues to use the music-note fallback.
- The unselected player heart uses theme gray. Song and Queue share a pinned
  seek and transport footer above the Android navigation safe area; artwork,
  metadata, secondary actions, and the queue scroll independently.
- SQLite v8 stores recent plays by account and track identity. A track enters
  history after Media3 reports actual playback; replaying it moves it to the
  front. Home shows up to 12 available tracks from enabled sources, including
  previously completed plays, and tapping one starts it from the beginning.
- Settings has an Account page with Telegram name, username, and profile photo
  or initials. Its confirmation explains the local data removed by sign-out.
  Playback and transfers stop before TDLib logout. After TDLib closes, the app
  removes account SQLite rows, offline media, TDLib files and cache, and the
  old encrypted database key. A persistent pending marker supports cleanup
  after interruption; cleanup failure blocks sign-in and offers Retry.

Formatting, 68 workspace tests, host Clippy, Android ARM64 Rust check, and the
SDK 35 APK build pass. The APK contains both fonts, the new logo, launcher and
splash resources, and TDLib. Host tests cover history order, replay, account
and source filtering, migration, profile-photo fallback, and an offline cleanup
failure followed by retry.

Samsung SM-A256E, Android 14: the updated APK installed over the existing
session. JetBrains Mono and mixed Persian/English track text rendered without
clipping at the phone's normal 1080 px width. The Song and Queue footer stayed
at the bottom while the Queue scrolled; the inactive heart was gray. Playing
“505” for several seconds put it in Continue Listening, and a cold relaunch
kept the entry. Account displayed the live profile name, username, and photo;
the sign-out confirmation fit the screen. Sign-out returned to welcome, and
`files/tdlib` fell from 61,099 KiB to 366 KiB for the new empty session. The
offline directory was empty. Welcome and login with the new mark fit at a
temporary 960 px width; the original display size was restored.

A direct post-logout SQLite row count on the phone was not run: automatic
approval review rejected exporting the private app database to the host for
that check. Account-row deletion is covered by host tests; the device check
used directory size and offline-folder contents instead.

The following checks remain unverified on a device: the new player footer at
960 px width, an actual completed track in history, profile initials when no
photo exists, the Android splash appearance, and the cleanup failure/Retry
screen. At that checkpoint, the device's TuneStash
account was signed out during the approved cleanup check; using the app again
requires a new Telegram sign-in. No phone number or verification code was
entered after sign-out.

## Launcher, source control, and library artwork increment

- The adaptive launcher uses an opaque lime background and a separate dark
  mark sized for Android's safe area. All legacy density icons have opaque lime
  backgrounds. The selected-source remove button shows a cross while the
  unselected add button keeps its plus and the remove accessibility label.
- SQLite v9 tracks artwork quality and whether a message has been checked for a
  thumbnail. Migration preserves existing cover data, and reindexing cannot
  overwrite a downloaded thumbnail or embedded cover with a smaller preview.
  Account cleanup removes these rows with the rest of the library.
- Selected audio and document messages supply small previews and Telegram
  thumbnail file IDs. At most two cover downloads run at once. Completed image
  files are size and format checked, stored with the library record, and
  published to Library, Home, Search, and the playback queue. Existing indexed
  tracks without a checked thumbnail, including tracks with a migrated small
  preview, are revisited through paged message lookups. Playback and explicit
  audio downloads also save embedded cover art at the highest quality level.
  Full audio files are never downloaded solely to look for artwork.

Formatting, all 73 workspace tests, strict host Clippy, the Android ARM64 Rust
check, and the SDK 35 APK build pass. Host tests cover audio/document thumbnail
parsing, download completion, bounded backfill beyond 200 tracks, v8 migration,
and cover preservation after reindex and restart. The APK contains the new
adaptive foreground glyph and all legacy icon densities with opaque lime
corners.

On Samsung SM-A256E (Android 14), the installed app drawer icon showed no dark
gap. Sources showed a cross on selected rows and a plus on unselected rows.
Before playback, “505” used the fallback in Library; after playback, its
embedded cover appeared in the Library row and mini-player. Both still showed
the cover after force-stop and cold launch. A track without a Telegram preview
continues to use the fallback until playback or an explicit download reveals
embedded art. Device checks did not exercise a Telegram cover-thumbnail
download or every library artwork source; those paths have host coverage.
