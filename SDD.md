# Software Design Document — Rust-First Telegram Music Player

Version: 1.0  
Date: 2026-09-29  
Status: Proposed implementation baseline; native integration remains to be proven.  
Companion specification: [PRODUCT.md](PRODUCT.md).

## 1. Architectural decision

Use standalone Dioxus for UI components and a Rust application core. Choose Dioxus's supported WebView mobile path for the first implementation; do not depend on the experimental WGPU renderer. No Flutter, Tauri wrapper, React, or separately hosted frontend is required.

UI behavior is Rust/RSX and styling is CSS. Application Rust runs natively in the selected mobile path. TDLib remains a C++ dependency accessed through its C/JSON interface. Native media services may require a small Kotlin/Java or Swift bridge. The objective is maximum useful Rust learning, not eliminating every non-Rust dependency.

Android is the first release target. Keep iOS boundaries explicit, but do not label iOS supported or tested merely because a target directory or trait exists. Compilation and physical-device tests require the appropriate Apple tooling.

## 2. Design principles

1. Keep product behavior in Rust and independent of UI and platform SDK types.
2. Prove real background playback and TDLib integration early.
3. Prefer two crates and explicit modules over a large framework of abstractions.
4. Make long-running tasks independent of component and WebView lifetimes.
5. Persist identity and progress; do not treat transient TDLib file handles as durable identities.
6. Report real capabilities. Fakes, unsupported adapters, and untested paths must be explicit.

## 3. Dependencies and boundaries

| Responsibility | Technology | Boundary |
|---|---|---|
| Components, routing, view state | Dioxus / RSX | App crate only |
| Layout, typography, themes | CSS | App assets |
| Domain and use cases | Rust | Core crate; no mobile/UI SDK dependencies |
| Async orchestration | Tokio and bounded channels | Service layer; no blocking database/FFI calls on UI threads |
| JSON/data conversion | Serde / serde_json | DTOs and adapters; not a replacement for domain types |
| Persistence | rusqlite / SQLite | Repository adapter and serialized database worker |
| Telegram protocol client | TDLib through C/JSON FFI | Telegram adapter with a narrow safe Rust API |
| Android playback | Media3 / MediaSessionService | Native adapter controlled through Rust-facing commands/events |
| iOS playback | AVPlayer / AVAudioSession and system media controls | Later native adapter with the same semantic contract |
| System interop | JNI; objc2 or small native bridge | Platform adapters only |
| Sensitive local keys | Android Keystore / iOS Keychain integration | Secure-store adapter |

Select mutually compatible released versions at implementation time, pin the Rust toolchain and TDLib build revision, commit Cargo.lock, and document native SDK/NDK/JDK requirements. A table naming a dependency is not evidence that its current version builds with the others.

## 4. Repository organization

Start with one Cargo workspace and two crates:

| Path | Contents |
|---|---|
| `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml` | Reproducible workspace and toolchain selection |
| `crates/music_core/src/domain/` | IDs, tracks, sources, playlists, queue, and availability types |
| `crates/music_core/src/application/` | Import, search rules, favorites, queue policy, and sync orchestration |
| `crates/music_core/src/ports/` | Repository, Telegram, media, and secure-store contracts |
| `crates/music_app/src/ui/` | Dioxus pages, components, view models, and localization scaffolding |
| `crates/music_app/src/runtime/` | Service composition, commands/events, cancellation, and lifecycle ownership |
| `crates/music_app/src/adapters/sqlite/` | Migrations and repository implementation |
| `crates/music_app/src/adapters/telegram/` | TDLib FFI, request correlation, auth, history, and file resolution |
| `crates/music_app/src/adapters/media/` | Android implementation, explicit iOS boundary, and test fake |
| `crates/music_app/src/adapters/secure_store/` | Platform key storage |
| `crates/music_app/assets/` | CSS, original icons, and redistributable test audio |
| `docs/` | Setup, implementation status, device evidence, and learning notes |

The native build tool may require generated/configuration files elsewhere; document their actual paths rather than forcing an incompatible layout. The core must build and test on a host without an Android SDK or TDLib installation.

## 5. Runtime and ownership

The application/service runtime owns the Telegram client, database worker, download scheduler, and playback controller. Dioxus components attach to snapshots and events. Unmounting a page must not destroy a download or playback session.

UI commands include `ConnectAccount`, `SelectSource`, `SyncSource`, `PlayTrack`, `Pause`, `Seek`, `ToggleFavorite`, and `DownloadOffline`. Events include auth transitions, source progress, library changes, download progress, playback snapshots, and structured errors.

Use bounded queues for work. Coalesce frequent progress events instead of allowing an unbounded event backlog. Preserve ordering for protocol/state transitions; progress notifications may be coalesced, but terminal events must not be dropped. Correlate requests and support cancellation and bounded retry.

On Android, the media service is the lifecycle owner for active background playback. It must be able to reach the controller and native player without depending on an Activity or WebView. A Rust runtime started only by the UI is insufficient. Prove the concrete service/runtime ownership in the early device spike.

After process death, restore durable state without automatically resuming audio unexpectedly. Continuous operation after an explicit OS force-stop is not promised.

## 6. Data model

Use application-generated stable IDs for library objects. Account/chat/message identifiers must retain their full numeric precision through serialization. Keep platform paths and transient SDK handles out of domain identity.

| Entity | Key fields and invariants |
|---|---|
| Account | Stable Telegram user identity and local state; one active account in A1 |
| Source | App ID, account ID, chat ID, kind, display name, enabled flag; unique account/chat pair |
| Track | App ID, original title/artist/filename, duration if known, normalized search fields |
| TrackSource | Track ID, source ID, account/chat/message reference, remote metadata, availability; unique account/chat/message |
| Favorite | Account/track relationship; persistent |
| Playlist | App ID, account ID, name, timestamps |
| PlaylistItem | Item ID, playlist ID, track ID, order; ordering changes are atomic |
| Download | Track/file identity, status, bytes, app-private path, explicit-offline flag, error |
| SyncCheckpoint | Source, history cursor, live/reconciliation markers, status, retry time |
| PlaybackSnapshot | Queue track IDs, current index, position, shuffle/repeat; no automatic autoplay on restore |

Start with one Track per imported message. Group proven identical files only when useful and preserve every TrackSource. Same title/artist/duration is not proof of identical audio. TDLib `file_id` values are runtime handles, not cross-device or permanent library IDs. Re-resolve files from durable message references and validate any cached remote identity after session changes.

Metadata overrides in M1 live separately from imported metadata so synchronization cannot overwrite user corrections. Cover art is optional; missing metadata always has a usable fallback.

## 7. Storage and transactions

Run SQLite through a dedicated worker or controlled blocking executor; never synchronously query it in a render path. Enable foreign keys. Migrations have ordered versions and run transactionally when supported. Test upgrading a populated earlier schema.

Commit imported message records and their history checkpoint in the same transaction. A crash before the commit repeats the page safely; a crash after the commit resumes from the recorded cursor. Unique keys and upsert semantics make replay idempotent.

Use paginated repository queries. Normalize Persian/Arabic yeh and kaf, case, and predictable whitespace in search fields while preserving the original display text. Avoid stripping distinctions aggressively. A simple indexed/search implementation is sufficient initially; adopt FTS only when a concrete library-size test justifies it.

## 8. Telegram adapter

### Authentication and lifecycle

Use TDLib's authorization updates as the state machine. Handle requested phone/code/password/email or other supported states without assuming every account follows the same sequence. Present unsupported states as actionable errors, not success. Do not automate account creation in A1.

Supply an application-owned api_id/api_hash through local build/developer configuration excluded from source control. Telegram application credentials embedded in a distributed client are not server secrets; account authorization codes, passwords, session material, and database keys remain sensitive. Never request that users paste login codes into an AI conversation or log them.

Keep TDLib databases in app-private storage. Protect the database encryption key through the secure-store adapter. Do not silently use a plaintext key in a production path when secure storage is unavailable. Development fakes must be clearly labeled.

### FFI

Use the C/JSON API rather than depending on C++ ABI stability. Isolate unsafe operations and document pointer ownership, string lifetime, null handling, and thread assumptions. Copy returned strings before their validity ends. Pin bindings/schema expectations to the shipped TDLib revision.

One receive loop demultiplexes updates and responses; request correlation uses opaque IDs. Blocking receive calls run on a dedicated thread. Route ordered updates into the service layer, and close the client cleanly on shutdown/logout. Provide a harmless initialization/version probe before attempting real authentication.

### Source indexing

Resolve Saved Messages using the authenticated account. List/select accessible channel and bot chats; bot history is read as user chat history, not through the Bot API.

Read newest audio first using supported filtered search/history APIs, and also account for audio sent as Documents. Persist separate cursors when multiple scans are used. Reconcile by message key. Filter voice notes, links, and unsupported content explicitly.

Keep pagination and live updates separate during an active source scan. Persist the completed source catalog and its success marker in one transaction. On later launches, restore that catalog without scanning again. Ignore subsequent source-discovery updates until the user chooses Resync; that command performs a full scan and replaces the saved catalog only on success. Deduplicate overlapping pages and apply edits/deletions observed during an active scan. A short result page alone is not a universal end-of-history signal; follow the pinned API's pagination semantics. Respect rate-limit retry times and use bounded concurrency across sources.

Telegram proxy settings are adapter configuration. The official Telegram app's working connection does not imply that this independent session inherits its proxy configuration.

## 9. Playback and downloads

### A1: complete-file playback

1. Resolve an accessible TrackSource.
2. Reuse a complete validated local file, or request its download.
3. Show truthful download/buffering state.
4. Prepare the platform media engine with the complete local file.
5. Observe actual engine events before reporting playback success.

Define Rust-facing operations for prepare, play, pause, seek, stop, and release, plus events for prepared, position, buffering, ended, interruption, and failure. The Rust queue controller is the logical authority; platform engines and lock-screen actions reflect and update that same controller. Avoid two independently advancing queues.

On Android, implement Media3 playback through MediaSessionService, required foreground-service configuration, audio focus, and system media controls. Do not request broad shared-storage access just to play app-private downloads. Handle headphone removal and audio interruptions. Platform callbacks must respect native thread requirements.

On iOS, use an explicit unavailable adapter until implemented; then integrate AVPlayer, AVAudioSession, Now Playing information, and remote commands. Never use a no-op adapter that reports successful playback.

### M1: progressive playback

TDLib does not supply a reusable public streaming URL for private Telegram audio. Build a byte-source adapter that maps the player's demand to TDLib download ranges and observes actual contiguous available bytes. Do not treat apparent file size as evidence that all bytes exist.

Choose between a platform-native data source and an app-local range bridge after a targeted spike. If using a local HTTP bridge, bind loopback only, use unguessable per-session routes, validate ranges, and end it with the media lifecycle. Keep TDLib identifiers/session data out of URLs. Coordinate seeks, overlapping download requests, cancellation, EOF, and reconnects. Fall back to complete-file playback where progressive decoding is unsupported.

### Cache policy

Explicit offline downloads are pinned; transient playback cache is evictable. The proposed cache default is 2 GiB and is user-adjustable in M1. Pinned downloads are accounted for separately and may exceed that cache allowance; show both totals. Never silently evict pinned content or the currently playing file. Low disk space yields an actionable error.

Store offline files in persistent app-private storage, not an OS-purgeable cache directory. Recognize that clearing app data or uninstalling removes them. Finish writes safely and mark completion only after validating the file state. Keep cache policy coordinated with TDLib storage management so TDLib cannot unexpectedly reclaim app-promised offline files.

## 10. State and recovery contracts

| Area | Representative states |
|---|---|
| Auth | Disconnected, waiting for input, ready, expired, logging out, failed |
| Sync | Idle, indexing, up to date, paused, retry scheduled, access lost |
| Download | Remote only, queued, downloading, complete, canceled, failed |
| Playback | Idle, preparing, buffering, playing, paused, ended, failed |

Use typed Rust enums and domain errors. A disconnected session must not prevent browsing already indexed data or playing allowed offline files. A logout explicitly clears that account's local data and media after user confirmation, while never deleting Telegram messages. Source deselection cancels that source's scans but does not delete messages or unrelated downloads.

Treat remote access loss, local missing files, rate limits, disk-full failures, expired sessions, and codec errors as distinct conditions. Apply backoff to retryable failures; do not retry authentication failures or unsupported codecs indefinitely.

## 11. Verification and delivery gates

| Gate | Required evidence |
|---|---|
| G0: foundation | Host core tests, and actual Android build result or clearly recorded missing prerequisite |
| G1: persistence | Normalization, repeat import, transactional checkpoints, playlist persistence, and migration tests |
| G2: media | Real Android complete-file playback with locked screen, notification/headset controls, focus handling, and UI detach/reattach |
| G3: Telegram | Pinned TDLib build, real auth through app UI, selected-source discovery and one audio download; fake protocol tests are separate |
| G4: alpha | PRODUCT AC-01 through AC-09, including one real source of each type and offline playback |
| G5: MVP | Progressive seek/reconnect, pinned cache protection, and broader device checks |
| G6: iOS | Actual Apple build plus login/media smoke tests; later device background-playback acceptance |

Use cargo fmt, targeted clippy, core tests, and adapter tests appropriate to the build host. Do not blindly combine mutually exclusive native features with `--all-features`. Test concrete risks: replayed pages, checkpoint crashes, queue reconciliation, missing files, and cancellation. UI fakes do not prove native behavior.

Measure on named device/OS/network conditions before adopting numerical performance targets. Keep progress updates rate-limited, lists paginated, and expensive work off the UI thread.

Maintain `docs/IMPLEMENTATION_STATUS.md` with implemented, simulated, blocked, and device-verified behavior. Keep concise Rust learning notes for each milestone. Initial implementation prompts end at A1/G4; M1 and iOS shipping require separate work.

## 12. Risks and open decisions

- Dioxus mobile/native-service lifecycle integration is an early technical risk, not an assumed solved dependency.
- The concrete TDLib build/link process and secure-store bridge must be proven for each target ABI.
- Supported audio containers/codecs depend on actual playback engines; do not advertise universal format support.
- Native platform shims are allowed when a Rust-only workaround makes the project harder to learn or maintain.
- Product name, public distribution strategy, precise minimum OS versions, and advanced streaming transport remain open.
- Telegram API requirements, including channel-content/sponsored-message behavior, need a public-release review. Build an independent client, not a workaround for content restrictions.

## 13. Primary references

Reviewed during design on 2026-09-29. Recheck version-specific instructions when implementing.

- [Dioxus mobile](https://dioxuslabs.com/learn/0.7/guides/platforms/mobile/)
- [Dioxus RSX](https://dioxuslabs.com/learn/0.7/essentials/ui/rsx/)
- [TDLib overview](https://core.telegram.org/tdlib)
- [TDLib authorization and history](https://core.telegram.org/tdlib/getting-started)
- [TDLib C/JSON interface](https://core.telegram.org/tdlib/docs/td__json__client_8h.html)
- [TDLib ranged download](https://core.telegram.org/tdlib/docs/classtd_1_1td__api_1_1download_file.html)
- [Android background media playback](https://developer.android.com/media/media3/session/background-playback)
- [Apple AVAudioSession](https://developer.apple.com/documentation/AVFAudio/AVAudioSession)
- [JNI Rust crate](https://docs.rs/jni/latest/jni/)
- [objc2 Rust crate](https://docs.rs/objc2/latest/objc2/)
- [Tokio](https://docs.rs/tokio/latest/tokio/)
- [rusqlite](https://docs.rs/rusqlite/latest/rusqlite/)
- [Telegram API terms](https://core.telegram.org/api/terms)
