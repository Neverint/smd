# Decisions

Defaults chosen during the build. Newest at the bottom of each phase.

## Phase 0: scaffold (2026-11-11)

| # | Decision | Why |
|---|---|---|
| 1 | Name **Scrub My Disks (SMD)**, repo `Neverint/smd`, public. | Chosen by the owner. No winget package uses the name. |
| 2 | License **MIT**. Duplicates are found with our own engine (size, partial hash, full hash); similar images will use `image_hasher` (MIT/Apache). `czkawka_core` (GPL-3.0) is not embedded. | Owner approved. MIT keeps every listed dependency usable. |
| 3 | Code lives in `G:\Documents\Dev\smd`, not in the Obsidian vault. | The vault is its own git repo and is indexed by Obsidian; `node_modules` and `target` are several GB. `G:\` root only lets administrators create folders, so `G:\Documents\Dev` is used instead of `G:\Dev`. |
| 4 | Rust toolchain in `G:\Documents\Dev\.rust` (`RUSTUP_HOME`, `CARGO_HOME` user variables). VS 2022 Build Tools (C++ workload) on C:. | Keeps C: (Windows only, about 21 % free) as light as possible. Build Tools cannot move off C:. |
| 5 | Tauri **2.x** (latest stable), not 3.0 (alpha). | Stability. |
| 6 | npm, not pnpm. | Already installed, fewer moving parts. |
| 7 | App identifier `io.github.neverint.smd`. App data and logs are in `%LOCALAPPDATA%\io.github.neverint.smd\`. | Tauri derives folders from the identifier. The migration journal will live in `...\io.github.neverint.smd\migrations\`. |
| 8 | Cargo workspace at the repo root: `src-tauri` (app) and `crates/core` (pure logic). `crates/win` and `crates/helper` are added when first needed (Phases 1 and 4). | Pure logic is unit-tested without Windows. No empty crates. |
| 9 | Window material: `window-vibrancy` Mica, falling back to acrylic (Windows 10), then solid. The page turns transparent only after the backend confirms an effect. | Never shows a see-through window without a material behind it. |
| 10 | "Acrylic sidebar" is a translucent layer over Mica, not a separate blur. | WebView2 cannot blur what is behind a transparent window. Windows 11's own apps use Mica for the navigation pane. |
| 11 | Custom title bar with Segoe Fluent Icons caption glyphs (Segoe MDL2 Assets on Windows 10). | Matches native Windows 11 buttons without bundling icon assets. |
| 12 | Hash router (`#/settings`, `#/dev/gallery`). | Works the same in the dev server and in the packaged app without server fallbacks. |
| 13 | `motion` package (the current name of Framer Motion), `@fluentui/react-icons`, `react-router`, `i18next`. | All MIT. |
| 14 | UI preferences (theme, language, sidebar) are kept in browser storage for now. Backend-relevant settings (exclusions, schedule, permanent delete, quarantine) will go to a JSON settings file read by Rust in Phase 1. | Phase 0 has no backend settings yet. |
| 15 | Language: "System" follows `navigator.language` (the Windows display language in WebView2); French for any `fr-*`, English otherwise. | Default to system language, as requested. |
| 16 | Sizes use 1024-based units with Explorer's labels (KB/MB/GB in English, Ko/Mo/Go in French). | Numbers match what Explorer shows. |
| 17 | Module accents: decorative gradient (`--m-from`/`--m-to`) plus a contrast-safe fill (`--m-solid`/`--m-on`): 700-level with white text in light, 300-level with black text in dark. Drive Organizer = slate→sky, Toolbox = stone (not in the brief). | WCAG AA for any text on a module color. |
| 18 | Accent rules use CSS `@scope` so the nearest theme wins (the gallery nests a light panel in a dark page). | Plain descendant selectors cannot express "nearest ancestor". WebView2/Chromium supports `@scope`. |
| 19 | Playwright uses the installed Microsoft Edge (`channel: "msedge"`). | No 150 MB browser download; Edge is on every Windows PC and CI runner. |
| 20 | Rotating logs through `tauri-plugin-log`: 5 files of 2 MB max, local only. | "Rotating local log file" requirement. |
| 21 | Installer targets: NSIS + MSI, WebView2 via bootstrapper. Unsigned for now; SignPath Foundation hook documented in `release.yml`. | SmartScreen will warn until signing is set up. |
| 22 | **My Clutter is scheduled in Phase 2** (the delivery plan did not list it). It reuses the parallel disk walker built for Space Lens in Phase 1. | Keeps Phase 1 focused. Owner can move it. |
| 23 | Smart Care in French is "Entretien express"; other module names are translated too (see `fr.json`). | French UI from day one. |
