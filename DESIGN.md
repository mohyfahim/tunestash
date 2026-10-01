# Music App Design System

Version: 1.0  
Theme: Light + Dark  
Platform direction: Mobile-first, adaptable to tablet, desktop, and car mode

## 0. Concept

The concepts are designed in [concept dir](concept) as follows:
- login page is in [login](concept/login.png)
- home page is in [home](concept/home.png)
- the search page is in [search](concept/search.png)
- the library page is in [lib](concept/library.png)
- the main player is in [main](concept/main-player.png) and mini player is in [mini](concept/mini-player.png)
- the car mode play is in [car](concept/car-mode.png)
- the source selection page is in [source](concept/music-sources.png)

## 1. Design principles

The visual system should feel modern, calm, tactile, and music-focused. The interface uses high-contrast neutrals, restrained color, soft depth, and a small number of vivid accents.

Primary principles:

- Use **electric lime** as the strongest interactive and playback accent.
- Use **electric violet** as a secondary expressive accent for source identity, selected states, and artwork-related highlights.
- Keep the main interface neutral and quiet so album artwork remains visually dominant.
- Use large rounded containers and soft layered surfaces rather than heavy borders.
- Maintain clear hierarchy with large headings, compact metadata, and generous whitespace.
- Preserve a consistent structure across light and dark themes.
- Design touch targets generously, especially for playback and car-mode controls.
- Support English, Persian, and mixed-direction metadata without changing the visual hierarchy.

---

## 2. Core color palette

```yaml
colors:
  electric-lime: "#d7ff3f"
  pressed-lime: "#badd2c"
  graphite-ink: "#171a19"
  warm-ivory: "#f8f7f2"
  gallery-white: "#fffefa"
  soft-plaster: "#e9ebe5"
  electric-violet: "#6c5ce7"
  muted-sage: "#a9b39c"
  quiet-ink: "#626864"
  signal-red: "#b42318"
```

### Accent roles

| Token | Role |
|---|---|
| `electric-lime` | Primary CTA, active nav, progress, selected chips, main playback emphasis |
| `pressed-lime` | Pressed/active state for lime controls |
| `electric-violet` | Secondary accent, source badges, optional highlights, artwork-linked controls |
| `signal-red` | Destructive actions, serious errors, unavailable states |
| `muted-sage` | Secondary metadata, inactive indicators, subtle status text |

---

## 3. Semantic color tokens

Use semantic tokens in UI code rather than raw hex values.

### Light theme

```yaml
theme-light:
  background: "#f8f7f2"
  background-elevated: "#fffefa"
  surface-primary: "#fffefa"
  surface-secondary: "#e9ebe5"
  surface-tertiary: "rgba(23, 26, 25, 0.05)"

  text-primary: "#171a19"
  text-secondary: "#626864"
  text-tertiary: "#a9b39c"
  text-on-accent: "#171a19"

  border-subtle: "rgba(23, 26, 25, 0.08)"
  border-strong: "rgba(23, 26, 25, 0.18)"

  accent-primary: "#d7ff3f"
  accent-primary-pressed: "#badd2c"
  accent-secondary: "#6c5ce7"

  icon-primary: "#171a19"
  icon-secondary: "#626864"
  icon-muted: "#a9b39c"

  destructive: "#b42318"
  success: "#d7ff3f"

  mini-player-bg: "#171a19"
  mini-player-text: "#fffefa"
  mini-player-muted: "#a9b39c"
```

### Dark theme

```yaml
theme-dark:
  background: "#171a19"
  background-elevated: "#1f2321"
  surface-primary: "#202421"
  surface-secondary: "#292e2a"
  surface-tertiary: "rgba(255, 254, 250, 0.06)"

  text-primary: "#fffefa"
  text-secondary: "#a9b39c"
  text-tertiary: "#626864"
  text-on-accent: "#171a19"

  border-subtle: "rgba(255, 254, 250, 0.08)"
  border-strong: "rgba(255, 254, 250, 0.16)"

  accent-primary: "#d7ff3f"
  accent-primary-pressed: "#badd2c"
  accent-secondary: "#6c5ce7"

  icon-primary: "#fffefa"
  icon-secondary: "#a9b39c"
  icon-muted: "#626864"

  destructive: "#b42318"
  success: "#d7ff3f"

  mini-player-bg: "#202421"
  mini-player-text: "#fffefa"
  mini-player-muted: "#a9b39c"
```

---

## 4. Theme behavior

### Light theme

Use light theme for browsing-heavy screens:

- Home
- Search
- Library
- Sources
- Playlists
- Settings

The light theme should feel editorial and spacious. Use `warm-ivory` for the page background and `gallery-white` for elevated cards.

### Dark theme

Use dark theme for immersive playback-focused surfaces:

- Main player / Now Playing
- Car mode
- Full-screen queue
- Optional expanded mini-player

Dark mode should feel cinematic and reduce competition with album artwork. Use `graphite-ink` as the base and let lime accents remain vivid.

### Theme continuity

The same interaction should keep the same semantic color in both themes.

Examples:

- Play/pause primary control → `electric-lime`
- Active tab → `electric-lime`
- Source identity or optional secondary accent → `electric-violet`
- Errors and destructive actions → `signal-red`

---

## 5. Typography

Recommended visual direction: neutral geometric sans-serif with strong legibility.

Suggested families:

- Inter
- SF Pro
- Manrope
- Geist

For Persian text, pair with a high-legibility Persian/Arabic family such as:

- Vazirmatn
- Noto Sans Arabic

### Type scale

| Style | Size | Weight | Use |
|---|---:|---:|---|
| Display | 34–40 px | 700 | Login / hero messaging |
| H1 | 28–32 px | 700 | Page title |
| H2 | 22–24 px | 700 | Section title |
| H3 | 18–20 px | 600 | Card / subsection title |
| Body Large | 16–17 px | 500 | Primary list labels |
| Body | 14–16 px | 400 | Standard text |
| Meta | 12–14 px | 400–500 | Artist, source, duration |
| Caption | 11–12 px | 500 | Tiny labels, counters |

### Typography rules

- Use sentence case for interface labels.
- Avoid all caps except very small tags when necessary.
- Track title is always visually stronger than artist/source metadata.
- Let Persian strings keep natural alignment and direction.
- Mixed-direction metadata should not be force-aligned character-by-character.

---

## 6. Spacing system

Base unit: **4 px**.

Recommended scale:

```text
4   xs
8   sm
12  md
16  lg
20  xl
24  2xl
32  3xl
40  4xl
48  5xl
64  6xl
```

### Screen padding

- Mobile horizontal padding: 20–24 px
- Compact layouts: 16 px
- Large player / tablet: 24–32 px
- Car mode: 28–40 px depending on display size

---

## 7. Corner radii

Use rounded geometry consistently.

```yaml
radius:
  xs: 8px
  sm: 12px
  md: 16px
  lg: 20px
  xl: 24px
  pill: 999px
```

Recommended uses:

- Search field: `16px`
- Track rows: `14–16px`
- Cards: `16–20px`
- Album art: `14–20px`
- Mini-player: `18–22px`
- Primary CTA: pill or `16–18px`
- Car mode buttons: `20–24px`

---

## 8. Elevation and shadows

Use depth sparingly. Prefer tonal contrast before shadows.

### Light theme shadow

```css
box-shadow: 0 8px 24px rgba(23, 26, 25, 0.08);
```

### Floating controls

```css
box-shadow: 0 10px 30px rgba(23, 26, 25, 0.14);
```

### Dark theme

Avoid heavy black shadows. Use surface contrast and subtle borders instead.

---

## 9. Navigation

Primary mobile navigation:

- Home
- Search
- Library

Optional additional destinations:

- Sources
- Settings

### Bottom navigation rules

- Background: theme surface
- Active icon: `electric-lime`
- Active label: `graphite-ink` in light mode, `electric-lime` or `gallery-white` in dark mode
- Inactive icon/text: `quiet-ink` or `muted-sage`
- Use filled icon for selected tab and outlined icon for inactive tabs when available

---

## 10. Buttons

### Primary button

Light and dark themes:

- Fill: `electric-lime`
- Text/icon: `graphite-ink`
- Pressed: `pressed-lime`
- Radius: 16–20 px or pill
- Height: 52–56 px

### Secondary button

Light theme:

- Fill: `graphite-ink`
- Text: `gallery-white`

Dark theme:

- Fill: `surface-secondary`
- Text: `gallery-white`
- Border: subtle

### Tertiary button

- Transparent background
- Icon/text uses `quiet-ink`, `muted-sage`, or theme-primary text

### Destructive

- Text/icon: `signal-red`
- Use filled red only for high-confidence destructive confirmations

---

## 11. Chips and filters

Use rounded pill filters for categories, source filters, and library modes.

### Selected chip

- Background: `electric-lime`
- Text: `graphite-ink`

### Unselected chip

Light:

- Background: `soft-plaster`
- Text: `quiet-ink`

Dark:

- Background: `surface-secondary`
- Text: `muted-sage`

### Source chip accents

Optional source identity may use `electric-violet` or soft tinted icon backgrounds.

---

## 12. Cards

### Media card

Used for playlists, albums, and recent items.

- Artwork dominates upper section
- 1:1 artwork ratio preferred
- Rounded corners: 16–20 px
- Title below artwork
- Metadata below title
- Avoid unnecessary borders

### Stat card

Used for:

- Favorites
- Downloads
- Track count
- Playlist count

Light:

- `gallery-white` or `soft-plaster`

Dark:

- `surface-primary`

Use a single accent icon rather than a full accent background.

---

## 13. Track rows

Each track row should support:

- Artwork thumbnail
- Track title
- Artist
- Optional Telegram source badge
- Duration
- Favorite state
- Download/offline state
- Overflow menu

### Hierarchy

1. Title
2. Artist / metadata
3. Source / state
4. Actions

### Row height

- Standard: 64–72 px
- Rich row: 76–88 px

### Artwork

- 48–56 px square
- Radius: 10–14 px

---

## 14. Search

Search is a prominent primary interaction.

### Search field

Light theme:

- Background: `soft-plaster`
- Text: `graphite-ink`
- Placeholder: `quiet-ink`

Dark theme:

- Background: `surface-secondary`
- Text: `gallery-white`
- Placeholder: `muted-sage`

### Search structure

Recommended order:

1. Search input
2. Type filters
3. Source filters
4. Results
5. Recent searches / popular library content

---

## 15. Mini player

The mini-player is persistent across Home, Search, and Library.

### Visual treatment

Light theme page:

- Player background: `graphite-ink`
- Primary text: `gallery-white`
- Secondary text: `muted-sage`
- Progress: `electric-lime`

Dark theme page:

- Player background: `surface-primary`
- Border: subtle
- Progress: `electric-lime`

### Contents

- 40–48 px artwork
- Track title
- Artist
- Play/pause
- Queue / next optional
- Thin progress bar

The entire central information area should be tappable to open the main player.

---

## 16. Main player / Now Playing

Use dark theme by default for immersion.

### Structure

1. Header / collapse control
2. Large artwork
3. Title + artist
4. Source badge
5. Favorite + download + more
6. Seek bar
7. Shuffle / previous / play-pause / next / repeat
8. Queue / lyrics / details

### Primary playback control

- Large circular button
- Fill: `electric-lime` or `gallery-white`
- Icon: `graphite-ink`
- Diameter: 72–88 px mobile

### Seek bar

- Active track: `electric-lime`
- Inactive track: muted neutral
- Thumb: `electric-lime`

---

## 17. Car mode

Car mode is an intentionally simplified playback surface.

### Design goals

- Maximum glanceability
- Oversized controls
- Minimal text
- Reduced interaction depth
- Strong contrast
- No dense browsing UI while driving

### Theme

Dark only.

```yaml
car-mode:
  background: "#171a19"
  panel: "#202421"
  text-primary: "#fffefa"
  text-secondary: "#a9b39c"
  accent: "#d7ff3f"
  secondary-accent: "#6c5ce7"
```

### Portrait car-mode layout

Recommended vertical order:

1. Minimal status/header controls
2. Large album artwork
3. Source badge
4. Track title
5. Artist
6. Progress / time
7. Large playback controls
8. 2 × 2 shortcut grid

Suggested shortcuts:

- Queue
- Go to album
- Download
- Shuffle

### Car-mode sizing

- Artwork: 45–55% of usable width
- Main play button: 88–112 px
- Previous / next: 64–80 px
- Shortcut cards: minimum 72 px tall
- Minimum touch target: 56 px

### Driving-safe behavior

- Avoid long scrolling lists
- Avoid text entry
- Avoid multi-step menus
- Keep queue browsing optional and shallow
- Prioritize play/pause, previous, next, queue, and favorite

---

## 18. Login / onboarding

Login should visually introduce the product without feeling like a settings screen.

### Visual direction

- Strong hero statement
- Abstract overlapping discs, waveforms, or album-art-inspired shapes
- Minimal supporting copy
- One primary CTA
- One secondary demo action

### CTA hierarchy

Primary:

`Continue with Telegram`

Secondary:

`Try Demo Library`

### Light version

- Page: `warm-ivory`
- CTA: `graphite-ink` or `electric-lime`
- Decorative accents: lime + violet

### Dark version

- Page: `graphite-ink`
- CTA: `electric-lime`
- Text: `gallery-white`

---

## 19. Source identity

Telegram sources should be visible without dominating the interface.

Recommended source types:

- Saved Messages
- Personal Channel
- Bot Chat

Use small pill badges with icon + label.

Suggested treatment:

- Saved Messages: blue-toned icon or neutral badge
- Personal Channel: violet accent
- Bot Chat: green accent

These source colors should remain secondary to the main product palette.

---

## 20. States

### Loading

- Skeleton surfaces use muted neutral fills
- Do not use spinning loaders for long indexing operations when progress can be shown

### Downloading

- Use circular or linear progress
- Accent: `electric-lime`

### Offline ready

- Small lime indicator or download-complete icon

### Unavailable

- Use `signal-red` for icon or state label
- Keep title readable
- Explain why playback is unavailable

### Disabled

- Reduce opacity
- Never rely on color alone

---

## 21. Motion

Motion should be subtle and functional.

Recommended durations:

```yaml
motion:
  fast: 120ms
  standard: 180ms
  emphasized: 260ms
```

Use motion for:

- Button press
- Tab selection
- Mini-player expansion
- Player artwork transition
- Sheet / modal presentation

Avoid decorative continuous animation in core playback UI.

---

## 22. Iconography

Use simple rounded outline icons with consistent stroke weight.

Suggested stroke:

- 1.75–2 px mobile
- 2–2.5 px large-screen/car mode

Key icons:

- Home
- Search
- Library
- Play / pause
- Previous / next
- Shuffle
- Repeat
- Favorite
- Download
- Queue
- More
- Telegram/source

---

## 23. Accessibility

### Contrast

- `graphite-ink` on `warm-ivory` / `gallery-white` should be the default high-contrast text pairing.
- `gallery-white` on `graphite-ink` is the preferred dark-theme text pairing.
- Do not use `muted-sage` for critical text at small sizes.
- Lime text on ivory backgrounds should be avoided; use lime as fill/accent instead.

### Touch targets

- Minimum standard target: 44 × 44 px
- Recommended primary target: 48 × 48 px or larger
- Car mode: 56 × 56 px minimum

### State communication

Never use color alone for:

- Download complete
- Playback error
- Favorite state
- Offline availability
- Source sync errors

Pair color with icons or labels.

---

## 24. Example CSS variables

```css
:root {
  --electric-lime: #d7ff3f;
  --pressed-lime: #badd2c;
  --graphite-ink: #171a19;
  --warm-ivory: #f8f7f2;
  --gallery-white: #fffefa;
  --soft-plaster: #e9ebe5;
  --electric-violet: #6c5ce7;
  --muted-sage: #a9b39c;
  --quiet-ink: #626864;
  --signal-red: #b42318;
}

[data-theme="light"] {
  --bg: var(--warm-ivory);
  --surface: var(--gallery-white);
  --surface-secondary: var(--soft-plaster);
  --text-primary: var(--graphite-ink);
  --text-secondary: var(--quiet-ink);
  --text-muted: var(--muted-sage);
  --accent: var(--electric-lime);
  --accent-pressed: var(--pressed-lime);
  --accent-secondary: var(--electric-violet);
  --danger: var(--signal-red);
}

[data-theme="dark"] {
  --bg: var(--graphite-ink);
  --surface: #202421;
  --surface-secondary: #292e2a;
  --text-primary: var(--gallery-white);
  --text-secondary: var(--muted-sage);
  --text-muted: var(--quiet-ink);
  --accent: var(--electric-lime);
  --accent-pressed: var(--pressed-lime);
  --accent-secondary: var(--electric-violet);
  --danger: var(--signal-red);
}
```

---

## 25. Screen theme summary

| Screen | Default Theme | Primary Accent |
|---|---|---|
| Login | Light or Dark | Lime + Violet |
| Home | Light | Lime |
| Search | Light | Lime |
| Library | Light | Lime |
| Sources | Light | Violet secondary |
| Mini Player | Dark surface | Lime |
| Main Player | Dark | Lime |
| Queue | Dark | Lime |
| Car Mode | Dark only | Lime |
| Errors / destructive confirmations | Current theme | Red |

---

## 26. Overall visual character

The final product should feel like a personal, premium music utility rather than a streaming catalog. The interface should be clean enough for large libraries, expressive enough for music, and restrained enough that the user's artwork and Telegram collection stay at the center of the experience.

The defining visual signature is:

**warm ivory + graphite + electric lime + restrained violet + large rounded media surfaces.**
