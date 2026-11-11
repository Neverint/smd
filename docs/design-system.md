# Design system

"CleanMyMac layout × Windows 11 materials." Original artwork only: no MacPaw names, logos or illustrations.
Source of truth: `src/design/tokens.css` and `src/design/module-accents.css`. Live preview: `npm run dev`, then open `/#/dev/gallery`.

## Materials

| Surface | Light | Dark | Notes |
|---|---|---|---|
| Window | Mica | Mica | Acrylic on Windows 10, solid `--bg-solid` if neither works |
| Sidebar + title bar | transparent over Mica | same | |
| Content layer (`--bg-layer`) | `rgba(255,255,255,.5)` on Mica, `#f9f9f9` solid | `rgba(58,58,58,.3)` on Mica, `#272727` solid | 8 px top-left radius, 1 px `--stroke-card` |
| Card (`--bg-card`) | `#ffffff` | `#2b2b2b` | 8 px radius, 1 px border, `--shadow-card` |
| Overlay / side sheet (`--bg-overlay`) | `#f9f9f9` | `#2c2c2c` | 12 px radius, `--shadow-dialog` |

## Color tokens

| Token | Light | Dark | Use |
|---|---|---|---|
| `--text-primary` | `rgba(0,0,0,.896)` | `#ffffff` | Body text |
| `--text-secondary` | `rgba(0,0,0,.606)` | `rgba(255,255,255,.786)` | Hints, taglines |
| `--text-disabled` | `rgba(0,0,0,.361)` | `rgba(255,255,255,.363)` | Disabled controls |
| `--accent` / `--on-accent` | `#005fb8` / white | `#60cdff` / black | System actions, links |
| `--stroke-card` | `rgba(0,0,0,.058)` | `rgba(255,255,255,.07)` | Card borders |
| `--stroke-control` | `rgba(0,0,0,.08)` | `rgba(255,255,255,.07)` | Control borders |
| `--success` | `#0f7b0f` | `#6ccb5f` | 🟢 |
| `--caution` | `#9d5d00` | `#fce100` | 🟡 |
| `--critical` | `#c42b1c` | `#ff99a4` | 🔴 |
| `--focus-ring` / inner | black / white | white / black | Double focus ring, keyboard only |

## Module accents

Each module sets four variables on `[data-module]`:

- `--m-from`, `--m-to`: decorative gradient for the glow, illustration orb and progress ring. **Never behind text.**
- `--m-solid`, `--m-on`: primary button fill and its text. Light theme: Tailwind-700-level fill with white text. Dark theme: 300-level fill with black text. All pairs reach 4.5:1 or better (WCAG AA).

| Module | Gradient (light) | Solid light | Solid dark |
|---|---|---|---|
| Smart Care | teal `#14b8a6` → blue `#2563eb` | `#0f766e` | `#5eead4` |
| Cleanup | green `#22c55e` → lime `#65a30d` | `#15803d` | `#86efac` |
| Protection | red `#ef4444` → pink `#ec4899` | `#b91c1c` | `#fca5a5` |
| Performance | amber `#f59e0b` → orange `#f97316` | `#b45309` | `#fcd34d` |
| Applications | blue `#3b82f6` → `#1d4ed8` | `#1d4ed8` | `#93c5fd` |
| My Clutter | purple `#a855f7` → violet `#7c3aed` | `#6d28d9` | `#d8b4fe` |
| Space Lens | cyan `#06b6d4` → sky `#0ea5e9` | `#0e7490` | `#67e8f9` |
| Hardware | orange `#f97316` → `#ea580c` | `#c2410c` | `#fdba74` |
| Privacy & Tweaks | indigo `#6366f1` → `#4338ca` | `#4338ca` | `#a5b4fc` |
| Drive Organizer | slate `#64748b` → sky `#0ea5e9` | `#475569` | `#cbd5e1` |
| Toolbox | stone `#78716c` → `#a8a29e` | `#57534e` | `#d6d3d1` |

My Activity and Settings use the system accent.

## Typography

| Role | Font | Size / line |
|---|---|---|
| Display (big numbers) | Segoe UI Variable Display, 600 | 68 / 92 |
| Title large | Segoe UI Variable Display, 600 | 40 / 52 |
| Title (page, hero) | Segoe UI Variable Display, 600 | 28 / 36 |
| Subtitle | Segoe UI Variable Text, 600 | 20 / 28 |
| Body / Body strong | Segoe UI Variable Text, 400 / 600 | 14 / 20 |
| Caption | Segoe UI Variable Text | 12 / 16 |

Falls back to Segoe UI on Windows 10.

## Spacing, radii, shadows

- Spacing: 4 px grid: `--space-1` 4, `-2` 8, `-3` 12, `-4` 16, `-5` 24, `-6` 32, `-7` 48, `-8` 64.
- Radii: controls 4, cards 8, overlays 12, pills 999.
- Shadows: `--shadow-card` (subtle), `--shadow-flyout` (menus), `--shadow-dialog` (side sheet). Dark theme uses stronger shadows.

## Motion

| Token | Value | Use |
|---|---|---|
| `--duration-fast` | 150 ms | Hover, press, toggles |
| `--duration-normal` | 250 ms | Side sheet, sidebar collapse |
| `--duration-slow` | 400 ms | Progress ring, hero entrance |
| `--ease-decelerate` | `cubic-bezier(0,0,0,1)` | Things entering |
| `--ease-standard` | `cubic-bezier(.55,.55,0,1)` | Point-to-point |

`prefers-reduced-motion: reduce` turns CSS animations off and makes `motion` skip transforms (`MotionConfig reducedMotion="user"`).

## Layout

- Title bar 40 px, custom, draggable, with native-style caption buttons (46 px wide, close turns red).
- Sidebar 240 px (56 px collapsed): icon, name, pending badge. Active item = subtle fill + 3 px module-colored pill.
- Main canvas: hero (glow + orb + title) at top center, one-line explanation, single primary action at bottom center.

## Components (all in the gallery)

Button (primary / accent / secondary / subtle, md / lg), IconButton (label required), Card (icon tile, big value, actions), Badge (5 tones), ProgressRing (determinate / indeterminate, `role=progressbar`), ScanButton (round, module colors), ModuleHero, SideSheet (modal, Esc, focus trap, focus return), CheckList (select all with indeterminate state), Segmented (radio group, arrow keys), Toggle (`role=switch`), setting row.

## Accessibility checklist

- WCAG AA contrast for all text (tokens above).
- Full keyboard navigation; visible focus ring on `:focus-visible` only.
- Every icon-only button has `aria-label` + tooltip.
- Dialogs: `role=dialog`, `aria-modal`, labelled by their title.
- Reduced motion respected.
