# Product Brief — Personal Telegram Music Player

Version: 1.0  
Date: 2026-09-29  
Status: Implementation specification; no application or device validation is implied.  
Product name: TuneStash.

## 1. Product intent

Turn the music a person already collects in Telegram into an organized, enjoyable music library on their phone. Offer familiar music-player interactions: artwork, search, favorites, playlists, a playback queue, background playback, and offline listening.

The first user's collection is distributed across a personal channel, Saved Messages, and conversations with several bots. All selected sources appear in one library while retaining their origin.

This is also a learning project. Writing and understanding Rust across the application core and user interface is a first-class goal. Android ships first; iOS follows using the same core and UI wherever feasible.

## 2. Goals and constraints

| Priority | Goal |
|---|---|
| 1 | Use Rust for application behavior, domain models, and UI components. |
| 2 | Deliver a usable Android music player connected to the user's existing Telegram collection. |
| 3 | Preserve a credible iOS path through platform-independent models and explicit platform adapters. |
| 4 | Keep implementation understandable enough to learn from and extend incrementally. |

The selected stack is standalone Dioxus with a Rust core. CSS styling, the C++ TDLib dependency, and small native platform bridges are acceptable. “Rust-first” does not require rewriting operating-system services or dependencies in Rust. Flutter and Tauri are not part of the selected implementation.

The application connects directly to Telegram. A product-operated backend, media hosting, and a separate product account are not required for the initial release.

## 3. Target user and jobs to be done

Primary user: someone who regularly saves music in Telegram but wants a dedicated listening experience.

- When I save songs in different chats, I want to find and play them in one place.
- When I discover a song through a bot, I want the received audio file to join my selected library without manually copying it to another chat.
- When I build a playlist, I want to mix songs from different sources.
- When I lock my phone or use headphones, I want playback and controls to keep working.
- When connectivity is unavailable, I want explicitly downloaded songs to remain playable.
- While implementing this product, I want each milestone to teach identifiable Rust concepts and produce a working result.

## 4. Sources and content rules

| Source | Supported behavior |
|---|---|
| Saved Messages | Index accessible audio after the user enables this source. |
| Personal channel | Index accessible history and reconcile changes during synchronization. |
| Existing bot conversation | Index audio files already delivered in that conversation. No bot token is required. |

Support Telegram Audio messages and Document messages that are identified as playable audio. Exclude voice notes by default. External links, buttons, and search-result text are not playable tracks unless an actual accessible audio file is present. Automatic bot commands, button clicks, and external music extraction are outside the initial scope.

Only sources explicitly selected by the user are indexed into the music library. Source selection is an application behavior, not a Telegram-issued music-only authorization scope. Secret-chat history from another device is outside scope.

## 5. Main experience

1. Choose connect a Telegram account.
2. Complete the authentication steps requested by Telegram.
3. Select Saved Messages, a personal channel, and/or bot conversations.
4. See recent tracks appear while older history is indexed incrementally.
5. Search, favorite, create playlists, and play tracks.
6. Download selected tracks and listen offline.

The user does not need to wait for the whole collection to download before browsing it. Track metadata is indexed separately from audio bytes.

### Navigation

| Surface | Main content |
|---|---|
| Home | Recently added tracks, recent listening, and shortcuts to playlists. |
| Search | Search across title, artist, and filename; optional source filtering. |
| Library | Tracks, favorites, playlists, and offline downloads. |
| Sources | Enabled sources, indexing progress, errors, and last successful synchronization. |
| Mini-player | Current track and basic controls, persistent across primary screens. |
| Now Playing | Artwork or fallback, seek, queue, repeat, shuffle, download, and favorite actions. |

Use an original, dark-first visual design. Familiar music-player patterns are welcome; copying another service's branding is not a requirement. Preserve Persian and English metadata, support mixed-direction text, and structure strings for localization. Initial interface copy can be English; full Persian translation is a later localization pass.

## 6. Product requirements

| ID | Requirement | First milestone |
|---|---|---|
| PR-01 | Authenticate one Telegram account and explain the independent session. | A1 |
| PR-02 | Select and deselect personal-channel, Saved Messages, and bot-chat sources. | A1 |
| PR-03 | Build a persistent library incrementally; repeated sync does not duplicate messages. | A1 |
| PR-04 | Search original metadata using normalized Persian/Arabic letters and case-insensitive matching. | A0/A1 |
| PR-05 | Persist favorites and create, rename, reorder, and delete basic local playlists. | A1 |
| PR-06 | Maintain one coherent queue across app, lock-screen, and headphone controls. | A1 |
| PR-07 | Keep playback working while the UI is backgrounded or the screen is locked. | A1 |
| PR-08 | Mark a track offline-ready only when a complete usable local file exists. | A1 |
| PR-09 | Expose indexing, download, connection, expired-session, and inaccessible-content states. | A1 |
| PR-10 | Preserve source references without merging different performances solely by matching names. | A1 |
| PR-11 | Separate evictable cache from explicit offline downloads; show storage usage. | M1 |
| PR-12 | Support progressive playback with full-download fallback for unsupported files. | M1 |
| PR-13 | Allow local metadata corrections without changing Telegram messages. | M1 |

## 7. Important behavior decisions

- Removing a source never deletes Telegram messages. Its tracks disappear from the active source view unless available through another enabled source. Playlist entries and explicit downloads remain identifiable until the user removes them; unavailable entries are labeled.
- Removing a playlist never deletes its underlying audio files.
- Deleting an offline download affects local storage only. It does not delete the Telegram message.
- Exact duplicate-file grouping is conservative. Matching artist/title alone is insufficient, particularly for live performances, remixes, and different quality versions.
- A remote deletion or loss of access can prevent future downloading. The product does not promise permanent availability. Content restrictions and ephemeral-media requirements remain authoritative.
- Auto-sync runs on app activation and through active connections when permitted. Continuous archive synchronization while the app is closed is not promised, especially on iOS.
- Playlists are device-local in phase 1. Signing into the same Telegram account on another device does not automatically synchronize product playlists.
- Logout stops playback and transfers, ends the session, and clears account-scoped local data and media. Confirm the local-data consequence in the UI before logout. Never delete remote content as part of logout.

## 8. Acceptance scenarios

| ID | Scenario and expected result |
|---|---|
| AC-01 | Select one personal channel, Saved Messages, and one bot chat; accessible audio from each appears with its origin. |
| AC-02 | Repeat synchronization or restart it after interruption; imported messages are not duplicated and history scanning resumes. |
| AC-03 | Search Persian text containing Arabic/Persian variants of kaf or yeh; equivalent normalized titles match while original display text is preserved. |
| AC-04 | Favorite a song and create a mixed-source playlist; both survive an app restart. |
| AC-05 | Play a fully downloaded track, lock the screen, and use media controls; playback and queue state stay consistent. |
| AC-06 | Enable airplane mode; a complete offline download plays and a remote-only track explains why it cannot play. |
| AC-07 | Interrupt a download; the track is not labeled offline-ready and retry can recover without creating a duplicate track. |
| AC-08 | Deselect a source; no Telegram message is changed and retained playlist entries have an explicit availability state. |
| AC-09 | Receive an audio Document, voice note, external URL, and bot button; classification follows the content rules. |
| AC-10 | Clear evictable cache; explicit offline downloads remain intact. Required for M1. |

A1 is complete only when AC-01 through AC-09 have meaningful evidence. Tests against fakes validate logic; they do not replace real Android, Telegram, or background-audio validation. Missing device access must be recorded as unverified.

## 9. Non-goals

No global music catalog, automatic bot interaction, message-sending product features, public music redistribution, web release, custom codec implementation, always-running synchronization guarantee, or proprietary recommendation backend in the initial release.

## 10. Success and learning evidence

Success means the first user can listen to their collection without switching among Telegram chats. Record local test evidence for source coverage, sync recovery, background playback, offline availability, and time to a playable track on a documented device/network. Do not invent performance guarantees or install analytics by default.

Each implementation increment should explain the Rust concepts used, the ownership/concurrency choices made, what was tested, and the exact remaining limitations. See [SDD.md](SDD.md) for architecture.
