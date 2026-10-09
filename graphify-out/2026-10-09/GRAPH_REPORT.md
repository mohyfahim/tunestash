# Graph Report - tunestash  (2026-10-02)

## Corpus Check
- 101 files · ~197,756 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 529 nodes · 721 edges · 72 communities (35 shown, 16 thin omitted)
- Extraction: 99% EXTRACTED · 1% INFERRED · 0% AMBIGUOUS · INFERRED: 5 edges (avg confidence: 0.85)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `d398d07a`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- [slug].tsx
- FontPanel.tsx
- website/package.json
- Software Design Document — Rust-First Telegram Music Player
- constants.ts
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

## God Nodes (most connected - your core abstractions)
1. `Music App Design System` - 28 edges
2. `react` - 16 edges
3. `compilerOptions` - 16 edges
4. `Software Design Document — Rust-First Telegram Music Player` - 14 edges
5. `react-i18next` - 13 edges
6. `getLanguages()` - 13 edges
7. `Product Brief — Personal Telegram Music Player` - 11 edges
8. `Vazirmatn Changelog` - 10 edges
9. `make-fonts.sh script` - 8 edges
10. `error()` - 8 edges

## Surprising Connections (you probably didn't know these)
- `DonationList()` --calls--> `getLanguageDirection()`  [EXTRACTED]
  fonts/vazirmatn/website/src/components/DonationList.tsx → fonts/vazirmatn/website/src/i18n.ts
- `getLangFromPath()` --calls--> `getLanguages()`  [EXTRACTED]
  fonts/vazirmatn/website/src/pages/_app.tsx → fonts/vazirmatn/website/src/i18n.ts
- `App()` --calls--> `getDefaultLang()`  [EXTRACTED]
  fonts/vazirmatn/website/src/pages/_app.tsx → fonts/vazirmatn/website/src/i18n.ts
- `Index()` --calls--> `getDefaultLang()`  [EXTRACTED]
  fonts/vazirmatn/website/src/pages/index.tsx → fonts/vazirmatn/website/src/i18n.ts
- `Doc()` --calls--> `getDefaultLang()`  [EXTRACTED]
  fonts/vazirmatn/website/src/pages/[lang]/docs/[slug].tsx → fonts/vazirmatn/website/src/i18n.ts

## Import Cycles
- None detected.

## Communities (72 total, 16 thin omitted)

### Community 0 - "[slug].tsx"
Cohesion: 0.07
Nodes (46): Footer(), Header(), HeroTitle(), LanguageMenu(), Layout(), LayoutProps, Meta(), ScrollTop() (+38 more)

### Community 1 - "FontPanel.tsx"
Cohesion: 0.10
Nodes (32): fileTypes, FontPanel(), generateId(), Font, fonts, GlyphPanel(), GlyphCollection, glyphCollections (+24 more)

### Community 2 - "website/package.json"
Cohesion: 0.05
Nodes (36): devDependencies, eslint, eslint-config-next, @types/node, @types/react, @types/react-window, typescript, name (+28 more)

### Community 3 - "Software Design Document — Rust-First Telegram Music Player"
Cohesion: 0.06
Nodes (32): 10. Success and learning evidence, 1. Product intent, 2. Goals and constraints, 3. Target user and jobs to be done, 4. Sources and content rules, 5. Main experience, 6. Product requirements, 7. Important behavior decisions (+24 more)

### Community 4 - "constants.ts"
Cohesion: 0.10
Nodes (23): getLanguageDirection(), BASE_PATH, DOWNLOAD_BASE_URL, DOWNLOAD_URL, TAG_NAME, VAZIRMATN_CDN_URL, VAZIRMATN_CSS_URL, VAZIRMATN_RD_CSS_URL (+15 more)

### Community 5 - "HELP-fa.md"
Cohesion: 0.08
Nodes (24): آدرس صفحه رسمی فونت وزیرمتن چیست؟, آیا وزیرمتن همان وزیر است؟, این فونت از چه زبان‌هایی پشتیبانی می‌کند؟, این فونت چند حالت/وزن دارد؟, برایم مهم نیست که در اصلِ متن، اعداد را به صورت لاتین یا عربی نوشته‌اند. چگونه فونت وزیرمتن را مجبور به نمایش فارسی اعداد کنم؟, فونت وزیرمتن با کدام فونت لاتین ترکیب شده است؟, مجوز استفاده از فونت وزیرمتن چیست؟, نسخه UI چیست؟ (+16 more)

### Community 6 - "dependencies"
Cohesion: 0.08
Nodes (24): dependencies, date-fns, @emotion/cache, @emotion/react, @emotion/server, @emotion/styled, gray-matter, i18next (+16 more)

### Community 7 - "DonationView.tsx"
Cohesion: 0.21
Nodes (11): DonationList(), Props, DonationView(), SelectButton, convertNumberToPersian(), formatNumber(), Size, useWindowSize() (+3 more)

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

## Knowledge Gaps
- **261 isolated node(s):** `music_app`, `music_core`, `name`, `version`, `author` (+256 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 329 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **16 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `react` connect `FontPanel.tsx` to `[slug].tsx`, `website/package.json`, `constants.ts`, `DonationView.tsx`?**
  _High betweenness centrality (0.046) - this node is a cross-community bridge._
- **Why does `dependencies` connect `dependencies` to `website/package.json`?**
  _High betweenness centrality (0.035) - this node is a cross-community bridge._
- **Why does `react-i18next` connect `[slug].tsx` to `FontPanel.tsx`, `website/package.json`, `DonationView.tsx`?**
  _High betweenness centrality (0.026) - this node is a cross-community bridge._
- **What connects `music_app`, `music_core`, `name` to the rest of the system?**
  _261 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `[slug].tsx` be split into smaller, more focused modules?**
  _Cohesion score 0.06634615384615385 - nodes in this community are weakly interconnected._
- **Should `FontPanel.tsx` be split into smaller, more focused modules?**
  _Cohesion score 0.09855072463768116 - nodes in this community are weakly interconnected._
- **Should `website/package.json` be split into smaller, more focused modules?**
  _Cohesion score 0.052564102564102565 - nodes in this community are weakly interconnected._