# Graph Report - tunestash  (2026-10-09)

## Corpus Check
- 110 files · ~218,323 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 775 nodes · 1317 edges · 81 communities (48 shown, 18 thin omitted)
- Extraction: 100% EXTRACTED · 0% INFERRED · 0% AMBIGUOUS · INFERRED: 6 edges (avg confidence: 0.85)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `9b6262fb`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- SourceSnapshot
- FontPanel.tsx
- website/package.json
- Software Design Document — Rust-First Telegram Music Player
- _app.tsx
- HELP-fa.md
- dependencies
- DonationView.tsx
- compilerOptions
- Music App Design System
- vazirmatn/package.json
- Vazirmatn Font فونت وزیرمتن
- Vazirmatn Changelog
- make-fonts.sh
- Vazirmatn-Variable-fa.md
- Vazirmatn-Files-fa.md
- 17. Car mode
- 20. States
- Next.js source code for Vazirmatn website
- 10. Buttons
- 18. Login / onboarding
- Vazirmatn Font Builder
- 11. Chips and filters
- 13. Track rows
- 16. Main player / Now Playing
- 23. Accessibility
- 4. Theme behavior
- 8. Elevation and shadows
- fix-features-fea-anchors.py
- set-uiargs.py
- 12. Cards
- 14. Search
- 15. Mini player
- 3. Semantic color tokens
- 5. Typography
- convert-to-rd-font.py
- set-farsi-digits.py
- set-names.py
- unlink-references.py
- TabPanel.tsx
- add-avar.py
- check-anchors.py
- export-glyph-names.py
- export-glyph-unicodes.py
- generate-feature-file.py
- generate-subset-plist.py
- make-package.sh
- merge-glyphs-plist.py
- webcss.py
- next-env.d.ts
- music_app
- Layout.tsx
- constants.ts
- telegram/mod.rs
- SourceChat
- ui/mod.rs
- SourceEngine
- [slug].tsx
- MainActivity
- [lang]/index.tsx
- build-android.sh
- i18n.ts
- Implementation status
- Android setup
- install-android.sh
- build-tdlib-android.sh

## God Nodes (most connected - your core abstractions)
1. `SourceEngine` - 54 edges
2. `Music App Design System` - 28 edges
3. `selected_engine()` - 23 edges
4. `SourceSnapshot` - 21 edges
5. `TdJson` - 16 edges
6. `react` - 16 edges
7. `compilerOptions` - 16 edges
8. `Driver` - 15 edges
9. `AuthService` - 15 edges
10. `SourceChat` - 14 edges

## Surprising Connections (you probably didn't know these)
- `kind_name()` --references--> `SourceKind`  [EXTRACTED]
  crates/music_app/src/adapters/sqlite/mod.rs → crates/music_core/src/domain/mod.rs
- `SourceEngine` --references--> `SourceStore`  [EXTRACTED]
  crates/music_app/src/adapters/telegram/sources.rs → crates/music_app/src/adapters/sqlite/mod.rs
- `command_request()` --references--> `AuthStage`  [EXTRACTED]
  crates/music_app/src/adapters/telegram/mod.rs → crates/music_core/src/domain/mod.rs
- `Driver` --references--> `SourceEngine`  [EXTRACTED]
  crates/music_app/src/adapters/telegram/mod.rs → crates/music_app/src/adapters/telegram/sources.rs
- `Driver` --references--> `AuthSnapshot`  [EXTRACTED]
  crates/music_app/src/adapters/telegram/mod.rs → crates/music_core/src/domain/mod.rs

## Import Cycles
- None detected.

## Communities (81 total, 18 thin omitted)

### Community 0 - "SourceSnapshot"
Cohesion: 0.06
Nodes (39): android_storage(), AndroidStorage, Result, String, Vec, AuthService, Receiver, Result (+31 more)

### Community 1 - "FontPanel.tsx"
Cohesion: 0.09
Nodes (34): SelectButton, fileTypes, FontPanel(), generateId(), Font, fonts, GlyphPanel(), GlyphCollection (+26 more)

### Community 2 - "website/package.json"
Cohesion: 0.05
Nodes (37): devDependencies, eslint, eslint-config-next, @types/node, @types/react, @types/react-window, typescript, name (+29 more)

### Community 3 - "Software Design Document — Rust-First Telegram Music Player"
Cohesion: 0.06
Nodes (32): 10. Success and learning evidence, 1. Product intent, 2. Goals and constraints, 3. Target user and jobs to be done, 4. Sources and content rules, 5. Main experience, 6. Product requirements, 7. Important behavior decisions (+24 more)

### Community 4 - "_app.tsx"
Cohesion: 0.14
Nodes (15): getLanguageDirection(), App(), cacheLtr, cacheRtl, getLangFromPath(), MyDocument, ButtonPropsColorOverrides, createCustomMuiTheme() (+7 more)

### Community 5 - "HELP-fa.md"
Cohesion: 0.08
Nodes (24): آدرس صفحه رسمی فونت وزیرمتن چیست؟, آیا وزیرمتن همان وزیر است؟, این فونت از چه زبان‌هایی پشتیبانی می‌کند؟, این فونت چند حالت/وزن دارد؟, برایم مهم نیست که در اصلِ متن، اعداد را به صورت لاتین یا عربی نوشته‌اند. چگونه فونت وزیرمتن را مجبور به نمایش فارسی اعداد کنم؟, فونت وزیرمتن با کدام فونت لاتین ترکیب شده است؟, مجوز استفاده از فونت وزیرمتن چیست؟, نسخه UI چیست؟ (+16 more)

### Community 6 - "dependencies"
Cohesion: 0.08
Nodes (24): dependencies, date-fns, @emotion/cache, @emotion/react, @emotion/server, @emotion/styled, gray-matter, i18next (+16 more)

### Community 7 - "DonationView.tsx"
Cohesion: 0.28
Nodes (9): DonationList(), Props, DonationView(), convertNumberToPersian(), formatNumber(), Size, useWindowSize(), Donation (+1 more)

### Community 8 - "compilerOptions"
Cohesion: 0.11
Nodes (18): compilerOptions, allowJs, esModuleInterop, forceConsistentCasingInFileNames, incremental, isolatedModules, jsx, jsxImportSource (+10 more)

### Community 9 - "Music App Design System"
Cohesion: 0.12
Nodes (16): 0. Concept, 19. Source identity, 1. Design principles, 21. Motion, 22. Iconography, 24. Example CSS variables, 25. Screen theme summary, 26. Overall visual character (+8 more)

### Community 10 - "vazirmatn/package.json"
Cohesion: 0.13
Nodes (14): author, bugs, url, dependencies, description, files, homepage, keywords (+6 more)

### Community 11 - "Vazirmatn Font فونت وزیرمتن"
Cohesion: 0.15
Nodes (12): Arch Linux ([AUR](https://aur.archlinux.org/packages/vazirmatn-fonts)), Authors, Build, CDN, Donation, Download, Fedora Linux, Install (+4 more)

### Community 12 - "Vazirmatn Changelog"
Cohesion: 0.18
Nodes (10): 32.0.0, 32.1, 32.101, 32.102, 33.000, 33.001, 33.002, 33.003 (+2 more)

### Community 13 - "make-fonts.sh"
Cohesion: 0.64
Nodes (8): create_instance(), error(), fixAnchors(), fixTypeMetricsSelectionBit(), generateMiscTTFs(), log(), make-fonts.sh script, subset()

### Community 14 - "Vazirmatn-Variable-fa.md"
Cohesion: 0.25
Nodes (7): فایل‌های فونت وزیرمتن متغیر, فونت متغیر چیست؟, نحوه استفاده از فونت وزیرمتنِ متغیر در برنامه‌های گوناگون, نحوه استفاده از فونت وزیرمتنِ متغیر در وب, نسخه بدون لاتین:, نسخه معمولی:, نسخه نقطه‌گرد:

### Community 15 - "Vazirmatn-Files-fa.md"
Cohesion: 0.29
Nodes (6): UI, اصلی, بدون لاتین Non-Latin, ترکیبی, تمام ارقام فارسی Farsi-Digits, نقطه‌گرد

### Community 16 - "17. Car mode"
Cohesion: 0.33
Nodes (6): 17. Car mode, Car-mode sizing, Design goals, Driving-safe behavior, Portrait car-mode layout, Theme

### Community 17 - "20. States"
Cohesion: 0.33
Nodes (6): 20. States, Disabled, Downloading, Loading, Offline ready, Unavailable

### Community 18 - "Next.js source code for Vazirmatn website"
Cohesion: 0.33
Nodes (5): Evironmental variables, Install the dependencies, Next.js source code for Vazirmatn website, Run, Translation

### Community 19 - "10. Buttons"
Cohesion: 0.40
Nodes (5): 10. Buttons, Destructive, Primary button, Secondary button, Tertiary button

### Community 20 - "18. Login / onboarding"
Cohesion: 0.40
Nodes (5): 18. Login / onboarding, CTA hierarchy, Dark version, Light version, Visual direction

### Community 21 - "Vazirmatn Font Builder"
Cohesion: 0.40
Nodes (4): How it works, Notes, Requirements, Vazirmatn Font Builder

### Community 23 - "11. Chips and filters"
Cohesion: 0.50
Nodes (4): 11. Chips and filters, Selected chip, Source chip accents, Unselected chip

### Community 24 - "13. Track rows"
Cohesion: 0.50
Nodes (4): 13. Track rows, Artwork, Hierarchy, Row height

### Community 25 - "16. Main player / Now Playing"
Cohesion: 0.50
Nodes (4): 16. Main player / Now Playing, Primary playback control, Seek bar, Structure

### Community 26 - "23. Accessibility"
Cohesion: 0.50
Nodes (4): 23. Accessibility, Contrast, State communication, Touch targets

### Community 27 - "4. Theme behavior"
Cohesion: 0.50
Nodes (4): 4. Theme behavior, Dark theme, Light theme, Theme continuity

### Community 28 - "8. Elevation and shadows"
Cohesion: 0.50
Nodes (4): 8. Elevation and shadows, Dark theme, Floating controls, Light theme shadow

### Community 29 - "fix-features-fea-anchors.py"
Cohesion: 0.83
Nodes (3): find_poses(), fix_pos(), fix_pos2()

### Community 30 - "set-uiargs.py"
Cohesion: 0.67
Nodes (3): set ascender and descender of the font, setUIArgs(), update_attribs()

### Community 32 - "12. Cards"
Cohesion: 0.67
Nodes (3): 12. Cards, Media card, Stat card

### Community 33 - "14. Search"
Cohesion: 0.67
Nodes (3): 14. Search, Search field, Search structure

### Community 34 - "15. Mini player"
Cohesion: 0.67
Nodes (3): 15. Mini player, Contents, Visual treatment

### Community 35 - "3. Semantic color tokens"
Cohesion: 0.67
Nodes (3): 3. Semantic color tokens, Dark theme, Light theme

### Community 36 - "5. Typography"
Cohesion: 0.67
Nodes (3): 5. Typography, Type scale, Typography rules

### Community 61 - "Layout.tsx"
Cohesion: 0.24
Nodes (7): Footer(), Header(), LayoutProps, Meta(), SITE_NAME, ToggleThemeContext, useToggleTheme()

### Community 62 - "constants.ts"
Cohesion: 0.18
Nodes (12): getDefaultLang(), BASE_PATH, DOWNLOAD_BASE_URL, DOWNLOAD_URL, SITE_BASE_PATH, TAG_NAME, VAZIRMATN_CSS_URL, VAZIRMATN_RD_CSS_URL (+4 more)

### Community 63 - "telegram/mod.rs"
Cohesion: 0.12
Nodes (21): auth_error(), command_request(), Driver, email_code_can_be_resent_at_telegram_request(), music_probe_page(), MusicProbePage, phone_is_valid(), playable_music_message() (+13 more)

### Community 64 - "SourceChat"
Cohesion: 0.14
Nodes (20): Connection, choices_survive_reopen_and_are_scoped_to_account(), kind_name(), music_name(), parse_kind(), parse_music(), populated_choice_database_upgrades_and_discovery_is_atomic(), Option (+12 more)

### Community 65 - "ui/mod.rs"
Cohesion: 0.20
Nodes (10): App(), LibraryPlaceholder(), Element, EventHandler, Option, Signal, String, send() (+2 more)

### Community 67 - "SourceEngine"
Cohesion: 0.08
Nodes (40): BTreeMap, BTreeSet, Send, TdJson, a_new_scan_starts_progress_at_zero(), cache_only_delete_does_not_recheck_a_finished_chat(), channels_defer_labeling_until_music_is_found(), ChatMeta (+32 more)

### Community 72 - "[slug].tsx"
Cohesion: 0.17
Nodes (14): ScrollTop(), docsDirectory, getAllDocs(), getDocBySlug(), getDocSlugs(), markdownToHtml(), getStaticProps(), Props (+6 more)

### Community 74 - "MainActivity"
Cohesion: 0.31
Nodes (5): Bundle, Context, MainActivity, WebView, WryActivity

### Community 75 - "[lang]/index.tsx"
Cohesion: 0.39
Nodes (4): HeroTitle(), Layout(), getStaticPaths(), getStaticProps

### Community 76 - "build-android.sh"
Cohesion: 0.22
Nodes (8): ANDROID_HOME, ANDROID_NDK_HOME, AR_aarch64_linux_android, CC_aarch64_linux_android, JAVA_HOME, NDK_HOME, PATH, build-android.sh script

### Community 77 - "i18n.ts"
Cohesion: 0.16
Nodes (16): LanguageMenu(), getLanguages(), getLocalCaption(), Resource, resources, Anchor, Link, LinkProps (+8 more)

### Community 78 - "Implementation status"
Cohesion: 0.40
Nodes (4): Implementation status, Implemented, Rust learning note, Verification

### Community 79 - "Android setup"
Cohesion: 0.50
Nodes (3): Android setup, Prerequisites, Telegram app credentials

## Knowledge Gaps
- **278 isolated node(s):** `music_app`, `Step`, `music_core`, `name`, `version` (+273 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 382 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **18 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `SourceEngine` connect `SourceEngine` to `SourceChat`, `SourceSnapshot`, `telegram/mod.rs`?**
  _High betweenness centrality (0.032) - this node is a cross-community bridge._
- **Why does `SourceSnapshot` connect `SourceSnapshot` to `SourceChat`, `ui/mod.rs`, `SourceEngine`, `telegram/mod.rs`?**
  _High betweenness centrality (0.027) - this node is a cross-community bridge._
- **Why does `react` connect `DonationView.tsx` to `FontPanel.tsx`, `website/package.json`, `_app.tsx`, `[lang]/index.tsx`, `i18n.ts`, `Layout.tsx`, `constants.ts`?**
  _High betweenness centrality (0.021) - this node is a cross-community bridge._
- **What connects `music_app`, `Step`, `music_core` to the rest of the system?**
  _278 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `SourceSnapshot` be split into smaller, more focused modules?**
  _Cohesion score 0.05844155844155844 - nodes in this community are weakly interconnected._
- **Should `FontPanel.tsx` be split into smaller, more focused modules?**
  _Cohesion score 0.09013605442176871 - nodes in this community are weakly interconnected._
- **Should `website/package.json` be split into smaller, more focused modules?**
  _Cohesion score 0.05121951219512195 - nodes in this community are weakly interconnected._