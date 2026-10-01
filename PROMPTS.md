
# TuneStash — Final Cursor Implementation Prompts

## How to use this document

Run the prompts in this order:

```text
Prompt 0
Prompt 1
Prompt 2
...
Prompt 32
```

Use **one prompt at a time**.

After Cursor finishes a prompt:

1. inspect the diff,
2. run the requested checks,
3. fix important problems,
4. commit the milestone,
5. continue to the next prompt.

Do not give Cursor all implementation prompts at once.

---

# PROMPT 0 — PROJECT CONTRACT

Use this once before beginning implementation.

```text
You are the senior Rust engineer implementing TuneStash.

Before modifying code, read these files from the repository root completely:

PRODUCT.md
SDD.md
DESIGN.md

Also inspect the concept directory:

concept/login.png
concept/home.png
concept/search.png
concept/library.png
concept/main-player.png
concept/mini-player.png
concept/car-mode.png
concept/music-sources.png

These files are authoritative project inputs.

==================================================
SOURCE-OF-TRUTH PRECEDENCE
==================================================

1. PRODUCT.md owns product behavior.

It defines:
- supported features
- supported Telegram content
- product requirements
- user-visible behavior
- non-goals
- acceptance scenarios
- logout/data semantics
- milestone scope

Never add behavior merely because it appears visually plausible.

2. SDD.md owns architecture and technical implementation.

It defines:
- crate boundaries
- runtime ownership
- persistence strategy
- async/concurrency rules
- Telegram integration
- TDLib FFI
- Android media integration
- secure storage
- verification gates
- platform boundaries

Do not violate the SDD merely to simplify implementation.

3. concept/*.png owns screen-level visual design.

For a screen with a corresponding concept image, the PNG is the visual source of truth for:
- overall composition
- section ordering
- visual hierarchy
- component placement
- relative sizing
- artwork proportions
- control grouping
- screen density
- navigation placement
- mini-player placement
- visual emphasis

Do not redesign concept-backed screens.

4. DESIGN.md owns the reusable design system.

It defines:
- palette
- semantic colors
- light/dark themes
- typography
- spacing
- corner radii
- shadows
- component styling
- state presentation
- accessibility
- touch targets
- motion
- iconography

When a concept does not show a detail, use DESIGN.md.

When behavior conflicts with a concept image, PRODUCT.md wins.

When architecture conflicts with a concept image, SDD.md wins.

When a concept clearly defines layout but DESIGN.md is general, follow the concept layout and DESIGN.md tokens.

Record unavoidable deviations in:

docs/IMPLEMENTATION_STATUS.md

==================================================
REPOSITORY ARCHITECTURE
==================================================

Follow SDD.md.

The intended workspace has two main crates:

crates/music_core
crates/music_app

music_core contains platform-independent:

src/domain/
src/application/
src/ports/

music_app contains:

src/ui/
src/runtime/
src/adapters/sqlite/
src/adapters/telegram/
src/adapters/media/
src/adapters/secure_store/

and:

assets/

Documentation belongs under:

docs/

Do not introduce a large framework of unnecessary crates.

==================================================
TECHNOLOGY
==================================================

Use:

- Rust
- Dioxus / RSX
- Dioxus supported WebView mobile path
- CSS
- Tokio
- bounded channels
- rusqlite / SQLite
- TDLib through its C/JSON API
- Media3 / MediaSessionService on Android
- JNI or a minimal Android bridge
- Android Keystore through a secure-store adapter

Do not introduce:

- Flutter
- Tauri
- React
- React Native
- Electron
- a hosted frontend
- a TuneStash backend
- remote TuneStash media hosting

Do not use Dioxus's experimental WGPU renderer.

==================================================
CORE ARCHITECTURAL INVARIANTS
==================================================

1. music_core must compile and test without:
   - Android SDK
   - TDLib installation
   - Dioxus UI runtime
   - JNI
   - Media3

2. Domain types must never contain:
   - Dioxus types
   - TDLib types
   - Android SDK types
   - JNI types
   - rusqlite row types

3. Dioxus components must not directly access:
   - SQLite
   - TDLib FFI
   - Media3
   - secure storage

4. Long-running tasks must not depend on Dioxus component lifetime.

Unmounting a page must not destroy:
- playback
- Telegram sync
- active downloads
- database workers

5. Use bounded channels.

Do not create unbounded progress-event queues.

Progress events may be coalesced.

Terminal transitions must never be silently dropped.

6. Never block the UI thread with:
   - SQLite
   - TDLib receive
   - native media operations

7. SQLite access must use a dedicated worker or controlled blocking execution.

8. TDLib unsafe FFI must be isolated behind a narrow safe adapter.

9. Use TDLib C/JSON rather than C++ ABI bindings.

10. On Android, MediaSessionService owns active background playback.

An Activity or WebView must not be required to keep playback alive.

11. There is exactly one logical playback queue.

Rust owns the canonical queue.

Media3, UI, notification controls, headset controls and lock-screen controls all manipulate that same logical queue.

12. A1 uses complete-file playback.

Do not simulate progressive playback.

Do not implement Telegram range streaming as part of A1.

13. Never report a partial file as offline-ready.

14. Never delete Telegram content as a consequence of:
   - deselecting a source
   - deleting a playlist
   - deleting an offline copy
   - logout

15. Never claim device functionality has been verified unless it was actually tested.

==================================================
DESIGN RULES
==================================================

Use semantic design tokens.

Do not scatter raw hex values through components.

Core palette:

electric-lime   #d7ff3f
pressed-lime    #badd2c
graphite-ink    #171a19
warm-ivory      #f8f7f2
gallery-white   #fffefa
soft-plaster    #e9ebe5
electric-violet #6c5ce7
muted-sage      #a9b39c
quiet-ink       #626864
signal-red      #b42318

Browsing screens normally use the light visual system.

Playback-focused screens normally use the dark visual system.

Preserve:
- English metadata
- Persian metadata
- mixed-direction text

Do not force all text into LTR.

==================================================
CONCEPT IMAGE RULES
==================================================

Before implementing a concept-backed screen:

1. Open the PNG at full resolution.
2. Inspect its composition.
3. Identify reusable components.
4. Implement responsive UI rather than absolute screenshot coordinates.
5. Compare the rendered result against the PNG after implementation.

Concept paths:

Login:
concept/login.png

Home:
concept/home.png

Search:
concept/search.png

Library:
concept/library.png

Mini player:
concept/mini-player.png

Main player:
concept/main-player.png

Car mode:
concept/car-mode.png

Music source selection:
concept/music-sources.png

Do not replace these layouts with generic Spotify, Apple Music, or YouTube Music patterns.

==================================================
DEVELOPMENT PROCESS
==================================================

For every implementation prompt:

1. Re-read the relevant sections of:
   PRODUCT.md
   SDD.md
   DESIGN.md

2. Open the relevant concept PNG if the task is UI-related.

3. Inspect existing implementation before changing it.

4. Briefly state:
   - what already exists
   - what is missing
   - what specifications control this task

5. Implement the smallest complete increment.

6. Do not implement future prompts unless necessary to compile the current step.

7. Run appropriate checks such as:

cargo fmt --check
cargo check
cargo test
cargo clippy

Do not blindly use --all-features where platform features are mutually exclusive.

8. Fix failures introduced by the current change.

9. Update:

docs/IMPLEMENTATION_STATUS.md

Use statuses:

- implemented
- simulated
- blocked
- device-verified

10. End every task with:

Specifications used
Files changed
Architecture decisions
Commands executed
Tests performed
Rust concepts involved
Known limitations
Device validation still required

Then stop.

Do not continue to the next milestone automatically.
```

---

# PROMPT 1 — REPOSITORY AUDIT

```text
Read:

PRODUCT.md
SDD.md
DESIGN.md

Then inspect the entire repository before implementing functionality.

Determine:

- current Cargo workspace structure
- current crates
- Dioxus setup
- Rust toolchain configuration
- Android configuration
- native Android code
- assets/CSS
- tests
- Telegram/TDLib code
- persistence code
- playback/media code
- secure storage code
- documentation

Compare the repository against the architecture required by SDD.md.

Classify each subsystem as:

- missing
- partial
- implemented
- verified

Cover:

- music_core
- music_app
- domain
- application
- ports
- runtime
- SQLite adapter
- Telegram adapter
- media adapter
- secure-store adapter
- UI
- design system
- Android build
- tests
- documentation

Do not reorganize functioning code merely because directory names differ.

Only recommend changes where current code violates an important SDD boundary.

Create or update:

docs/IMPLEMENTATION_STATUS.md

Record the baseline accurately.

Do not implement major features during this prompt.

Run any harmless existing checks needed to understand repository health.

Stop after the audit.
```

---

# PROMPT 2 — WORKSPACE FOUNDATION

```text
Using SDD.md as the architecture authority, establish or repair the intended two-crate workspace.

Target:

crates/music_core/
crates/music_app/

music_core should contain:

src/domain/
src/application/
src/ports/

music_app should contain:

src/ui/
src/runtime/
src/adapters/sqlite/
src/adapters/telegram/
src/adapters/media/
src/adapters/secure_store/

Assets should live under:

crates/music_app/assets/

Documentation belongs under:

docs/

Important:

music_core must build and test on a host without:
- Android SDK
- TDLib
- Media3
- JNI
- Dioxus mobile runtime

Pin or document:

- Rust toolchain
- Cargo.lock
- relevant native prerequisites

Do not create speculative crates.

Do not implement Telegram or full playback yet.

Run host-side checks proving the core is platform-independent.

Update docs/IMPLEMENTATION_STATUS.md.

Stop.
```

---

# PROMPT 3 — DOMAIN MODEL

```text
Read PRODUCT.md requirements and SDD.md data-model section.

Implement the platform-independent domain model inside:

crates/music_core/src/domain/

Create strongly typed identifiers rather than reusing raw String values for unrelated identities.

Implement the entities required by SDD.md, including:

Account
Source
Track
TrackSource
Favorite
Playlist
PlaylistItem
Download
SyncCheckpoint
PlaybackSnapshot

Source kinds must represent:

SavedMessages
PersonalChannel
BotConversation

Preserve Telegram origin information without putting TDLib-specific types into the domain.

Track identity rules:

- start with one Track per imported message
- retain TrackSource references
- do not merge merely because title/artist/duration match
- TDLib file_id is not permanent application identity

Represent meaningful states with typed enums.

Examples include:

Auth:
- disconnected
- waiting for input
- ready
- expired
- logging out
- failed

Sync:
- idle
- indexing
- up to date
- paused
- retry scheduled
- access lost
- failed

Download:
- remote only
- queued
- downloading
- complete
- canceled
- failed

Playback:
- idle
- preparing
- buffering
- playing
- paused
- ended
- failed

A partial file must not be representable as offline-ready.

Do not add:
- Dioxus
- TDLib
- rusqlite
- Android types

to domain types.

Add unit tests for important invariants.

Stop.
```

---

# PROMPT 4 — CORE PORTS AND APPLICATION SERVICES

```text
Read SDD.md dependency boundaries and runtime design.

Define small, product-oriented ports in:

crates/music_core/src/ports/

Add only interfaces currently required.

Likely responsibilities include:

- library persistence
- sources
- playlists
- Telegram operations
- media playback
- offline downloads
- secure storage

Do not create large generic CRUD abstractions.

Interfaces should describe TuneStash operations.

Implement application-layer use cases in:

crates/music_core/src/application/

Prepare operations such as:

SelectSource
DeselectSource
SyncSource
SearchLibrary
ToggleFavorite
CreatePlaylist
RenamePlaylist
DeletePlaylist
AddTrackToPlaylist
RemoveTrackFromPlaylist
ReorderPlaylist
PlayTrack
Pause
Seek
DownloadOffline

Use structured application/domain errors.

Infrastructure-specific errors must be translated before crossing the adapter boundary.

Add fakes where useful for unit tests.

Do not implement real TDLib/SQLite/Media3 yet.

Stop.
```

---

# PROMPT 5 — LONG-LIVED APPLICATION RUNTIME

```text
Read the runtime and ownership sections of SDD.md.

Implement the service/runtime layer under:

crates/music_app/src/runtime/

The long-lived runtime will eventually own:

- Telegram client
- database worker
- download scheduler
- playback controller

Dioxus components must not own these resources.

Create typed application commands, for example:

ConnectAccount
SubmitAuthInput
SelectSource
DeselectSource
SyncSource
ToggleFavorite
DownloadOffline
PlayTrack
Pause
Seek

Create typed snapshots/events for:

- authentication
- source state
- library changes
- sync progress
- downloads
- playback
- errors

Use bounded channels.

Frequent progress events may be coalesced.

Never drop important terminal transitions.

Implement clean cancellation/shutdown behavior.

Pages must be able to mount/unmount without terminating playback, downloads or indexing.

Use fakes/placeholders where adapters are not implemented.

Add runtime tests where practical.

Stop.
```

---

# PROMPT 6 — SQLITE PERSISTENCE

```text
Read the storage/data sections of SDD.md.

Implement the SQLite adapter under:

crates/music_app/src/adapters/sqlite/

Use rusqlite.

Do not synchronously query SQLite from a Dioxus render path.

Use either:

- a dedicated serialized database worker
or
- a controlled blocking executor consistent with the SDD

Enable foreign keys.

Implement ordered migrations from the beginning.

Persist at least:

Account
Source
Track
TrackSource
Favorite
Playlist
PlaylistItem
Download
SyncCheckpoint
PlaybackSnapshot where applicable

Create appropriate unique constraints and indexes.

Critical import rule:

Imported messages and their synchronization checkpoint must commit atomically.

A crash before the transaction commits should safely replay the page.

A crash after commit must resume using the stored cursor.

Search:

Persist normalized search fields while preserving original display text.

Normalize at least:

- Arabic/Persian yeh variants
- Arabic/Persian kaf variants
- case where applicable
- predictable whitespace

Do not aggressively strip meaningful distinctions.

Tests must cover:

- migrations
- populated-schema upgrade where practical
- idempotent upsert/replay
- checkpoint transaction behavior
- favorite persistence
- playlist persistence
- playlist ordering
- normalization

Update G1-related evidence in IMPLEMENTATION_STATUS.md.

Stop.
```

---

# PROMPT 7 — TDLIB BUILD AND FFI SPIKE

```text
Read the Telegram adapter section of SDD.md.

Implement the smallest real TDLib integration required to prove the architecture.

Location:

crates/music_app/src/adapters/telegram/

Goal:

Prove:

- TDLib build/link
- client creation
- C/JSON boundary
- request/response correlation
- update receive loop
- harmless version/initialization probe
- clean shutdown

Use TDLib C/JSON.

Do not depend on C++ ABI bindings.

Unsafe operations must be isolated.

Document:

- pointer ownership
- string lifetime
- null handling
- threading assumptions
- pinned TDLib revision

Copy returned strings before their lifetime expires.

Blocking receive must run on a dedicated thread.

Use opaque IDs for request correlation.

Keep api_id/api_hash in local developer/build configuration excluded from source control.

Never log:
- auth codes
- passwords
- session data
- secure-store keys

Do not implement the complete Telegram product workflow yet.

Record real native build status in:

docs/IMPLEMENTATION_STATUS.md

Stop.
```

---

# PROMPT 8 — ANDROID MEDIA LIFECYCLE SPIKE

```text
Read the playback/runtime/verification sections of SDD.md.

Implement the narrowest Android media integration that proves the intended lifecycle.

Use:

Media3
MediaSessionService

Location:

crates/music_app/src/adapters/media/

and minimal native Android code where necessary.

Goal:

Prove that background playback can be owned by the Android media service rather than the Dioxus WebView or Activity.

Implement only enough to support:

- prepare a complete local test file
- play
- pause
- stop
- basic native event callback
- release

Expose it through the Rust-facing media port.

Do not create an independent platform queue.

The Rust playback controller will remain the logical queue authority.

If a device/emulator is available, perform an actual playback smoke test.

If not, record:

implemented but not device-verified

Do not claim SDD media verification gate G2 complete from mocks or compilation.

Stop.
```

---

# PROMPT 9 — DESIGN SYSTEM IMPLEMENTATION

```text
Read DESIGN.md completely.

Inspect all concept images:

concept/login.png
concept/home.png
concept/search.png
concept/library.png
concept/main-player.png
concept/mini-player.png
concept/car-mode.png
concept/music-sources.png

Do not implement complete pages yet.

Extract the reusable UI system shared by these screens.

Implement semantic CSS variables.

Support:

[data-theme="light"]
[data-theme="dark"]

Implement semantic tokens rather than raw hex use in individual components.

Include concepts such as:

background
background-elevated
surface-primary
surface-secondary
surface-tertiary

text-primary
text-secondary
text-tertiary
text-on-accent

border-subtle
border-strong

accent-primary
accent-primary-pressed
accent-secondary

destructive
success

mini-player-bg
mini-player-text
mini-player-muted

Implement DESIGN.md:

- spacing scale
- radius scale
- typography hierarchy
- shadows
- buttons
- chips
- track rows
- cards
- accessibility target sizes

Create only reusable components justified by concepts/current requirements.

Likely:

AppScaffold
PageHeader
SectionHeader
BottomNavigation
PrimaryButton
SecondaryButton
IconButton
FilterChip
SourceBadge
Artwork
MediaCard
PlaylistCard
TrackRow
SearchField
Toggle
ProgressBar
PlaybackButton
EmptyState
LoadingState
ErrorState

Components must support Persian and mixed-direction metadata.

Do not create giant screenshot-specific components.

Stop.
```

---

# PROMPT 10 — APP ROUTING AND SHELL

```text
Read PRODUCT.md navigation requirements, SDD runtime rules and DESIGN.md navigation section.

Implement typed application routing for the current product surfaces.

Prepare routes for:

Login/Auth
Home
Search
Library
Sources
Settings
MusicSources
NowPlaying
Queue
CarMode

Do not assume every screen appears in bottom navigation.

Implement the shared application shell.

Browsing screens should use the light design system unless the concept or DESIGN.md specifies otherwise.

Playback-focused screens use the dark treatment.

Build one reusable bottom navigation component.

Do not duplicate navigation markup between pages.

Prepare a persistent region where the mini-player can later appear.

Do not fake active playback before playback state exists.

Stop.
```

---

# PROMPT 11 — TELEGRAM AUTHENTICATION

```text
Read:

PRODUCT.md PR-01
SDD.md Telegram authentication section
DESIGN.md login section
concept/login.png

Open concept/login.png at full resolution before coding.

PRODUCT.md owns behavior.

concept/login.png owns the page composition.

DESIGN.md owns styling primitives.

Implement authentication using TDLib authorization updates as the state machine.

Do not assume every account uses:

phone -> code -> success

Handle TDLib-requested states appropriately, potentially including:

phone
code
password
email-related authorization states
waiting
ready
expired
failed

Unsupported states must produce an actionable error.

Security:

Never log:
- authentication codes
- passwords
- Telegram session material
- database encryption keys

Use the secure-store boundary for sensitive local material.

UI:

Match concept/login.png as closely as practical in:

- composition
- typography hierarchy
- CTA placement
- spacing
- visual proportions
- branding treatment
- color balance

Do not add product functionality solely because it appears visually attractive.

If concept/login.png or DESIGN.md contains a demo CTA but PRODUCT.md does not currently require demo mode, do not make demo mode an A1 requirement.

Authentication logic must remain outside Dioxus components.

The UI sends commands and renders auth state.

No TDLib types in UI components.

Perform visual comparison after implementation.

Stop.
```

---

# PROMPT 12 — TELEGRAM SOURCE DISCOVERY

```text
Read PRODUCT.md source rules and SDD.md source-indexing section.

Implement discovery of candidate Telegram music sources.

Support:

Saved Messages
accessible personal channels
existing bot conversations

Important:

Bot conversations are read as normal user chat history.

Do not use the Telegram Bot API.

Do not:
- send bot commands
- click bot buttons
- perform external music extraction
- automatically enable discovered chats

Only explicitly selected sources become TuneStash library sources.

Saved Messages should be resolved using the authenticated account.

Translate Telegram DTOs into internal adapter/application types.

TDLib types must not reach UI/domain code.

Add fake adapter tests for discovery behavior.

Stop.
```

---

# PROMPT 13 — MUSIC SOURCES SELECTION UI

```text
Read:

PRODUCT.md
SDD.md
DESIGN.md

Open:

concept/music-sources.png

at full resolution.

Implement:

Settings -> Music Sources

Treat concept/music-sources.png as the visual source of truth.

Match its:

- page hierarchy
- account card
- source sections
- row layout
- toggle placement
- search placement
- spacing
- surface treatment
- typography
- action placement

Implement the behavior required by PRODUCT.md.

The page should communicate:

Music Sources

Select where TuneStash finds music in Telegram.

Only explicitly selected sources are indexed.

Represent:

Saved Messages
Personal Channels
Bot Conversations

These are application indexing choices.

Do not describe them as Telegram authorization permissions.

Selected source rows should show where data exists:

- avatar/fallback
- name
- username
- source type
- enabled state
- indexed track count
- sync status
- overflow actions

Possible sync states should map from real typed state rather than UI strings.

Available chats section:

- searchable
- only relevant candidate sources
- explicit Add action
- no automatic selection

Deselecting a source must:

- stop/cancel future scanning for that source
- never delete Telegram messages
- not silently delete explicit offline files
- not silently destroy playlist references

Persist selected state.

Use application commands rather than local component-only state.

Add tests for:

- selecting
- deselecting
- duplicate selection prevention
- Saved Messages
- personal channel
- bot conversation

Compare final UI against concept/music-sources.png.

Stop.
```

---

# PROMPT 14 — INCREMENTAL TELEGRAM INDEXER

```text
Read PRODUCT.md content rules and SDD.md Telegram indexing/storage sections.

Implement incremental source indexing.

Pipeline:

Telegram history
-> message classification
-> metadata extraction
-> internal representation
-> transactional persistence
-> library update

Supported playable content:

- Telegram Audio
- Document messages recognized as playable audio

Exclude by default:

- voice notes
- external links
- buttons
- text-only bot/search results
- unsupported documents

unless there is an actual accessible audio file.

Synchronization behavior:

- recent tracks appear early
- older history indexes incrementally
- checkpoints persist per source
- repeated synchronization is idempotent
- interrupted scans resume
- overlapping pages deduplicate
- live updates and historical pagination remain distinct
- edits/deletions can be reconciled
- reconnect can repair/reconcile recent history
- rate limits respect retry timing
- concurrency across sources remains bounded

Do not use TDLib file_id as permanent TuneStash identity.

Commit imported records and checkpoint atomically.

Create fake Telegram tests covering:

- Audio
- playable Document
- voice note
- URL
- bot button
- duplicated message/page
- overlapping pagination
- interrupted indexing
- resume
- edited message
- deleted/inaccessible message

Stop.
```

---

# PROMPT 15 — SOURCES STATUS SCREEN

```text
Read PRODUCT.md Sources navigation requirements and DESIGN.md.

There is no dedicated source-status PNG concept.

Therefore:

- PRODUCT.md owns information architecture
- DESIGN.md owns visual treatment
- reuse visual patterns/components from concept/music-sources.png
- do not invent a visually unrelated design language

Implement the main Sources screen.

For each enabled source show:

- source name
- source kind
- enabled state
- indexed track count
- sync/indexing state
- progress if meaningful
- last successful synchronization
- current error if any

Support meaningful actions where applicable:

Retry
Pause
Resume
Sync
Disable source

Do not expose destructive Telegram operations.

Present typed states such as:

idle
indexing
up to date
paused
retry scheduled
access lost
failed

Use DESIGN.md state colors.

Do not rely on color alone.

Stop.
```

---

# PROMPT 16 — SEARCH ENGINE

```text
Read PRODUCT.md PR-04 and SDD.md storage/search rules.

Implement library search in the application/core layer.

Search fields:

title
artist
filename

Support optional source filtering.

Normalization:

- case-insensitive where applicable
- Persian/Arabic yeh equivalence
- Persian/Arabic kaf equivalence
- predictable whitespace handling

Preserve original metadata for display.

Normalization should be a deterministic, testable function.

Do not put normalization logic inside UI components.

Add tests for:

- English case differences
- Arabic/Persian yeh
- Arabic/Persian kaf
- mixed Persian/English strings
- filename lookup
- source filtering
- empty query
- whitespace normalization

Do not implement global music search.

Only search the indexed TuneStash library.

Stop.
```

---

# PROMPT 17 — SEARCH PAGE UI

```text
Read:

PRODUCT.md
DESIGN.md

Open:

concept/search.png
concept/mini-player.png

Treat concept/search.png as the page-level visual source of truth.

Implement Search using actual application search state.

Match:

- page title
- search-field size
- search-field styling
- filter-chip placement
- source-filter treatment
- result-row hierarchy
- result spacing
- icons/actions
- bottom navigation
- mini-player placement

Use shared components.

Do not hardcode the concept's sample track names.

Implement real states:

- initial/empty query
- searching/loading if needed
- results
- no results
- error

Keep alternate states visually consistent with the base concept.

Search UI must call the application Search use case.

Do not perform business/search normalization in the component.

Integrate the shared mini-player only when playback state exists.

Compare final screen with concept/search.png.

Stop.
```

---

# PROMPT 18 — FAVORITES AND PLAYLIST APPLICATION LOGIC

```text
Read PRODUCT.md PR-05 and SDD.md data/storage rules.

Implement favorites and local playlists.

Favorites must persist.

Playlists must support:

- create
- rename
- delete
- add track
- remove track
- reorder tracks

Playlist ordering changes should be atomic.

A playlist may contain tracks originating from different Telegram sources.

Deleting a playlist must never delete underlying audio or Telegram messages.

Deselecting a source must not destroy retained playlist references.

If a referenced track becomes unavailable, represent availability explicitly.

Add application/persistence tests for:

- favorite persistence
- create playlist
- rename
- delete
- mixed-source entries
- add/remove
- reorder
- retained unavailable entries
- restart persistence

Stop.
```

---

# PROMPT 19 — LIBRARY PAGE UI

```text
Read:

PRODUCT.md
DESIGN.md

Open:

concept/library.png
concept/mini-player.png

Treat concept/library.png as the visual source of truth.

Implement Library using real application state.

Required product views:

Tracks
Favorites
Playlists
Offline

The exact control presentation should remain visually consistent with concept/library.png.

Match the concept's:

- header
- tabs/filter layout
- list hierarchy
- artwork proportions
- metadata spacing
- favorite/offline actions
- overflow actions
- bottom navigation
- mini-player position

Track rows should expose where applicable:

- artwork
- title
- artist
- source identity
- duration
- favorite state
- download/offline state
- overflow action

Use reusable TrackRow and related components.

Do not hardcode sample tracks from the image.

Support:

- loading
- empty library
- no favorites
- no playlists
- no offline tracks
- errors

without fundamentally redesigning the screen.

Compare the rendered page with concept/library.png.

Stop.
```

---

# PROMPT 20 — HOME APPLICATION DATA

```text
Read PRODUCT.md Home requirements.

Implement application selectors/query logic needed by Home.

Provide data for:

Recently Added
Recent Listening / Continue Listening
Playlist shortcuts

Do not fabricate progress.

A track appears in Continue Listening only if real playback progress exists.

Keep query work out of Dioxus render paths.

Use paginated/limited repository queries rather than loading the entire library when unnecessary.

Prepare meaningful empty states.

Stop.
```

---

# PROMPT 21 — HOME PAGE UI

```text
Read:

PRODUCT.md
DESIGN.md

Open:

concept/home.png
concept/mini-player.png

Treat concept/home.png as the visual source of truth.

Implement Home using the real application data prepared by the previous step.

Match:

- header composition
- section ordering
- Recently Added cards
- Continue Listening rows
- playlist cards
- artwork proportions
- section spacing
- horizontal scrolling where shown
- mini-player
- bottom navigation

Use shared components.

Do not hardcode concept metadata.

Follow PRODUCT.md required content even if sample text differs.

Implement useful states for:

- empty library
- no recent listening
- no playlists
- indexing in progress
- error

Do not falsely present playback progress.

Compare final UI with concept/home.png.

Stop.
```

---

# PROMPT 22 — OFFLINE DOWNLOAD MANAGER

```text
Read PRODUCT.md PR-08 and SDD.md A1 playback/download rules.

Implement explicit offline downloads.

A1 requires complete files before playback.

Download states should include:

remote only
queued
downloading
complete
canceled
failed

Persist:

- track/file identity
- status
- byte/progress information where available
- local app-private path
- explicit-offline flag
- error information

Rules:

1. Partial files are never offline-ready.
2. Completion is recorded only after the local file has been validated.
3. Explicit offline content lives in persistent app-private storage.
4. Do not use an OS-purgeable cache directory for promised offline content.
5. Low disk space must produce an actionable error.
6. Interrupted downloads must be recoverable where supported.
7. Retry must not duplicate the library track.

Do not implement M1 progressive playback.

Do not implement the full M1 cache policy.

Add tests for:

- queue/start
- progress
- interruption
- retry
- cancellation
- complete-file validation
- missing local file
- incorrect offline-ready prevention

Stop.
```

---

# PROMPT 23 — CANONICAL PLAYBACK AND QUEUE CONTROLLER

```text
Read PRODUCT.md PR-06/PR-07 and SDD.md playback architecture.

Implement the canonical Rust playback controller.

It is the logical authority for:

- queue
- current queue index
- current track
- playback state
- playback position
- duration
- repeat
- shuffle
- playback errors

Support operations:

PlayTrack
ReplaceQueue
Pause
Resume
Seek
Stop
Next
Previous
AddNext
AddToQueue
RemoveFromQueue
ReorderQueue
ClearUpcoming
SetShuffle
SetRepeat

Repeat modes should include appropriate typed variants such as:

Off
One
All

The platform media engine must not maintain an independently advancing product queue.

System controls must feed commands back into this controller.

Restore durable playback snapshot if appropriate.

Do not automatically resume audio after process restoration.

Add thorough state-machine tests.

Stop.
```

---

# PROMPT 24 — COMPLETE ANDROID MEDIA3 PLAYBACK

```text
Expand the earlier Media3 spike into the complete A1 playback adapter.

Read SDD.md playback section again.

Support:

- prepare validated complete local file
- play
- pause
- seek
- stop
- next/previous integration through canonical controller
- release
- prepared state
- position updates
- ended
- interruption
- failure

Android responsibilities include:

- Media3
- MediaSessionService
- foreground/background playback requirements
- media notification/system controls
- audio focus
- headphone removal
- call/interruption handling
- lock-screen controls

Platform callbacks must respect native thread requirements.

The media service must not depend on an Activity or Dioxus WebView to stay alive.

Only complete validated local files are playable in A1.

Do not pretend progressive playback exists.

If real Android testing is possible, record actual evidence.

If not, explicitly classify relevant behavior as:

implemented but not device-verified

Stop.
```

---

# PROMPT 25 — MINI PLAYER UI

```text
Read DESIGN.md mini-player section.

Open:

concept/mini-player.png

Treat it as the visual source of truth for the persistent mini-player.

Implement one reusable MiniPlayer component.

Match:

- overall container proportion
- surface styling
- corner radius
- artwork size
- title hierarchy
- artist treatment
- progress presentation
- play/pause placement
- secondary action placement
- spacing

On light browsing pages, use the dark mini-player surface shown by the design.

The mini-player must observe canonical playback state.

It must not own local copies of:

- queue
- current track
- playback status
- playback position

The central information area opens Now Playing.

Integrate this same component with:

Home
Search
Library

and other appropriate browsing screens.

Provide accessible labels for controls.

Compare the component against concept/mini-player.png.

Stop.
```

---

# PROMPT 26 — MAIN PLAYER UI

```text
Read:

PRODUCT.md
DESIGN.md

Open:

concept/main-player.png

Treat it as the main visual source of truth.

Implement Now Playing using canonical playback state.

Match the concept's:

- top collapse/back control
- artwork position and scale
- title
- artist
- source identity
- favorite control
- download control
- overflow
- seek bar
- elapsed/remaining time
- playback controls
- secondary action placement
- queue area if represented
- dark background/surface treatment

Use DESIGN.md semantic colors.

The screen should use the dark immersive theme.

Only expose functionality that actually exists.

Do not implement Lyrics merely because music players commonly have lyrics.

All playback controls send commands to the canonical playback controller.

No component-local queue.

No second player.

Use real download/favorite states.

Compare final screen with concept/main-player.png.

Stop.
```

---

# PROMPT 27 — QUEUE UI

```text
Read PRODUCT.md queue requirements and DESIGN.md full-screen playback guidance.

There is no dedicated queue concept PNG.

Use:

- DESIGN.md
- existing Main Player visual language
- reusable components

Do not invent a visually unrelated design.

Implement Queue.

Show:

- current track
- upcoming items
- artwork
- title
- artist
- source identity where useful
- duration where known

Support:

- play item now
- remove item
- reorder
- clear upcoming

Use an accessible alternative to drag-and-drop where necessary.

Queue mutations must immediately update:

- playback controller
- mini-player
- main player
- Media3/system controls

Do not create a second UI-specific queue state.

Stop.
```

---

# PROMPT 28 — PORTRAIT CAR MODE

```text
Read DESIGN.md Car Mode section.

Open:

concept/car-mode.png

Treat concept/car-mode.png as the visual source of truth.

Implement the in-app portrait Car Mode playback surface.

Important:

This is not Android Auto.

Do not describe or implement it as Android Auto or CarPlay.

Match the concept's:

- portrait composition
- background
- minimal header
- artwork scale
- source badge
- title
- artist
- progress/time display
- oversized playback controls
- secondary shortcut cards
- spacing
- high contrast

Use dark theme only.

Primary controls should emphasize:

Previous
Play/Pause
Next

Use supported secondary actions such as:

Favorite
Queue
Shuffle
Repeat
Download

only when consistent with product functionality and the concept.

Car Mode must use the same canonical playback controller.

Do not instantiate another media engine.

Interaction rules:

- minimum touch target approximately 56x56
- no text entry
- no dense settings
- no deep menus
- no long browsing lists
- minimal scrolling

Compare rendered UI with concept/car-mode.png.

Stop.
```

---

# PROMPT 29 — RECOVERY, LIFECYCLE AND STRUCTURED ERRORS

```text
Read SDD.md state/recovery contracts and PRODUCT.md error requirements.

Implement coherent behavior for:

- Telegram disconnected
- Telegram reconnect
- network loss
- network restoration
- source indexing interruption
- download interruption
- app background
- app foreground
- Dioxus UI detach/reattach
- source access loss
- expired session
- missing local file
- Telegram rate limits
- disk full
- unsupported/failed codec
- process restoration where applicable

Rules:

Already-indexed library data must remain browseable when Telegram is disconnected.

Valid complete offline files should remain playable when allowed.

Do not promise continuous archive synchronization while the application is fully closed.

Do not unexpectedly autoplay after process death.

Retry only failures that are genuinely retryable.

Apply bounded backoff.

Implement user-facing errors that answer:

- what happened?
- what is affected?
- what remains usable?
- can the user retry?
- what action is needed?

Do not display raw:

- TDLib JSON
- SQLite errors
- JNI errors
- Rust debug output

Use structured typed errors.

Use DESIGN.md state presentation.

Do not rely on red/green alone.

Stop.
```

---

# PROMPT 30 — SAFE LOGOUT

```text
Read PRODUCT.md logout behavior and SDD.md state/recovery rules.

Implement safe logout.

Before logout, display an explicit confirmation describing the local consequences.

The user must understand that logout clears TuneStash's account-scoped local data/media but does not delete Telegram content.

Coordinate logout in a safe order:

1. stop playback
2. cancel/settle downloads and transfers
3. stop source synchronization
4. close/end the Telegram session
5. clear account-scoped database data
6. clear account-scoped local media
7. clear sensitive account material as appropriate
8. reset runtime state
9. return to authentication UI

Never delete:

- Telegram messages
- Telegram channels
- Telegram chats
- remote Telegram files

Add orchestration tests using fake adapters.

Stop.
```

---

# PROMPT 31 — ACCESSIBILITY, RTL AND CONCEPT VISUAL REVIEW

```text
Read DESIGN.md accessibility, typography and state sections.

First perform an accessibility pass across all implemented screens.

Verify:

- minimum 44x44 standard targets
- 48x48 preferred primary targets
- 56x56 minimum Car Mode targets
- sufficient contrast
- screen-reader labels
- scalable typography
- long titles
- missing artist
- missing artwork
- Persian RTL metadata
- mixed Persian/English metadata
- state communication that does not rely on color alone

Move UI copy into localization-ready organization.

Do not translate or mutate user music metadata.

Then perform a visual concept review.

Open:

concept/login.png
concept/home.png
concept/search.png
concept/library.png
concept/main-player.png
concept/mini-player.png
concept/car-mode.png
concept/music-sources.png

Compare each implemented concept-backed screen.

Review:

- overall composition
- section ordering
- alignment
- spacing
- typography hierarchy
- artwork proportions
- control placement
- corner radii
- surfaces
- color use
- mini-player integration
- bottom navigation
- selected states
- touch-target sizing

Classify mismatches:

Critical
Major
Minor
Intentional

Intentional deviations require a documented reason such as:

- PRODUCT.md requirement
- SDD.md architecture constraint
- accessibility
- responsive behavior
- platform limitation

Fix Critical and Major discrepancies.

Do not chase meaningless single-pixel differences that reduce accessibility or maintainability.

Document remaining intentional deviations in:

docs/IMPLEMENTATION_STATUS.md

Stop.
```

---

# PROMPT 32 — A1 ACCEPTANCE, VERIFICATION AND FINAL AUDIT

```text
Read PRODUCT.md and SDD.md completely again.

This is an audit.

Do not add new product features unless required to fix a confirmed A1 gap.

==================================================
PRODUCT REQUIREMENTS
==================================================

Evaluate PRODUCT.md A1 requirements.

Pay particular attention to:

PR-01
Telegram authentication

PR-02
Select/deselect Saved Messages, personal channels and bot chats

PR-03
Persistent incremental library with idempotent sync

PR-04
Persian/Arabic normalized search

PR-05
Persistent favorites and playlists

PR-06
One coherent queue

PR-07
Background/locked-screen playback

PR-08
Truthful offline-ready state

PR-09
User-visible indexing/download/connection/session/access errors

PR-10
Preserved source references without unsafe title-based merging

Do not pull M1-only work into A1.

Specifically do not make A1 completion depend on:

- M1 cache accounting
- progressive playback
- local metadata corrections

==================================================
ACCEPTANCE SCENARIOS
==================================================

Evaluate AC-01 through AC-09 exactly as written in PRODUCT.md.

Create/confirm automated evidence where possible.

AC-01:
one personal channel + Saved Messages + one bot chat, preserving origin

AC-02:
interrupted/repeated synchronization resumes without duplicate imports

AC-03:
Persian/Arabic kaf/yeh normalization works while preserving original text

AC-04:
favorite + mixed-source playlist survive restart

AC-05:
complete-file playback and queue stay coherent across media commands

AC-06:
complete offline file plays without connectivity; remote-only track explains why it cannot

AC-07:
interrupted download is not offline-ready; retry recovers without duplicate track

AC-08:
source deselection leaves Telegram untouched and retained playlist references expose availability

AC-09:
Audio Document / voice note / external URL / bot button classification follows product rules

Do not treat AC-10 as an A1 requirement if PRODUCT.md marks it as M1.

==================================================
SDD ARCHITECTURE AUDIT
==================================================

Verify:

- two-crate boundary
- platform-independent music_core
- long-lived runtime ownership
- bounded channels
- controlled SQLite worker/blocking execution
- transactional checkpoints
- safe TDLib C/JSON boundary
- isolated unsafe code
- secure storage boundary
- single canonical Rust queue
- MediaSessionService ownership
- complete-file A1 playback
- no accidental Activity/WebView playback dependency

==================================================
SDD VERIFICATION GATES
==================================================

Evaluate G0 through G4 using real evidence.

G0 Foundation:
- host core tests
- actual Android build or clearly recorded missing prerequisite

G1 Persistence:
- normalization
- repeat import
- transactional checkpoints
- playlist persistence
- migration testing

G2 Media:
requires real Android evidence for:
- complete-file playback
- screen locked/background
- notification/media controls
- headset controls
- audio focus
- UI detach/reattach

Do not mark G2 verified from mocks.

G3 Telegram:
requires:
- pinned TDLib build
- real auth through the application UI
- real selected-source discovery
- real audio download

G4 Alpha:
requires meaningful PRODUCT AC-01 through AC-09 evidence, including appropriate real-device/Telegram checks.

==================================================
RUN CHECKS
==================================================

Run applicable commands.

At minimum where valid:

cargo fmt --check
cargo check
cargo test
cargo clippy

Run the repository's documented Android/native build commands.

Do not blindly combine mutually exclusive platform features.

Fix legitimate issues.

==================================================
FINAL IMPLEMENTATION STATUS
==================================================

Update:

docs/IMPLEMENTATION_STATUS.md

Use statuses:

verified
implemented but unverified
simulated only
blocked
not implemented

Include evidence.

==================================================
FINAL REPORT
==================================================

Produce:

1. Architecture overview
2. PRODUCT requirement matrix
3. AC-01 through AC-09 matrix
4. SDD G0-G4 verification matrix
5. Automated test evidence
6. Real Android evidence
7. Real Telegram evidence
8. Implemented-but-unverified behavior
9. Blocked behavior
10. Known limitations
11. Technical debt
12. Security/privacy notes
13. Exact work still required before A1/G4 can legitimately be called complete

Never claim verification that did not actually occur.
```

---

# REUSABLE PROMPT — REVIEW EACH MILESTONE

Use this after a significant Cursor implementation before committing.

```text
Read:

PRODUCT.md
SDD.md
DESIGN.md

If the changed screen has a concept PNG, inspect that PNG too.

Review the previous implementation as a senior Rust engineer.

Do not modify code initially.

Report findings under:

Critical
High
Medium
Low

Check specifically for:

- PRODUCT.md scope violations
- SDD.md architecture violations
- concept visual drift
- DESIGN.md token violations
- domain/UI coupling
- TDLib types leaking outside adapter
- Android/JNI types leaking into core
- direct SQLite access from UI
- blocking work on async/UI threads
- unsafe FFI scope
- unnecessary cloning
- unnecessary Arc usage
- Mutex/RwLock misuse
- locks held across await
- unbounded channels
- task leaks
- missing cancellation
- duplicate queue ownership
- false playback state
- partial file marked offline-ready
- credential/session logging
- insecure sensitive storage
- unverified behavior reported as verified
- accessibility regressions

Then fix clearly actionable Critical and High findings.

Do not perform unrelated aesthetic refactoring.

Add regression tests where practical.

Run relevant checks.

Update docs/IMPLEMENTATION_STATUS.md.

Report:

- findings
- fixes
- files changed
- tests
- remaining concerns
```

---

# REUSABLE PROMPT — BUG FIX

```text
Before diagnosing this bug, read:

PRODUCT.md
SDD.md
DESIGN.md

If the bug involves a concept-backed UI, also open the corresponding concept PNG.

Problem:

[PASTE BUG HERE]

Do not immediately patch the symptom.

First determine:

1. how the issue can be reproduced
2. expected behavior
3. actual behavior
4. owning subsystem
5. root cause
6. whether a specification invariant is violated

Classify ownership as:

music_core/domain
music_core/application
music_core/ports
music_app/runtime
SQLite adapter
Telegram adapter
media adapter
secure-store adapter
UI
CSS/design
native Android

Make the smallest correct fix.

Do not refactor unrelated code.

Add a regression test where practical.

Run appropriate formatting/check/test commands.

Report:

Root cause
Specification involved
Files changed
Why the fix works
Tests executed
Remaining limitation
Device validation required
```

---

# RECOMMENDED COMMIT SEQUENCE

Use approximately these commit boundaries:

```text
01 repository-audit
02 workspace-foundation
03 domain-model
04 core-ports
05 runtime
06 sqlite-persistence
07 tdlib-spike
08 android-media-spike
09 design-system
10 app-shell
11 telegram-auth
12 source-discovery
13 music-sources-ui
14 incremental-indexing
15 sources-status
16 search-core
17 search-ui
18 favorites-playlists
19 library-ui
20 home-data
21 home-ui
22 offline-downloads
23 playback-controller
24 android-media3
25 mini-player
26 main-player
27 queue
28 car-mode
29 lifecycle-errors
30 logout
31 accessibility-visual-review
32 a1-final-audit
```

Recommended rhythm:

```text
Implementation prompt
↓
Inspect diff
↓
Run milestone review prompt
↓
Fix Critical / High findings
↓
Run tests
↓
Update IMPLEMENTATION_STATUS.md
↓
Commit
↓
Next prompt
```

This sequence intentionally keeps each Cursor task bounded.

Do not ask Cursor to implement TuneStash end-to-end in one pass.