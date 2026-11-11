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

## Phase 1: Smart Care, Cleanup, Space Lens, My Activity (2026-11-11)

| # | Decision | Why |
|---|---|---|
| 24 | Windows code lives in a new `crates/win` crate (drives, memory, processes, Recycle Bin, disk walk, cleanup scan). Pure logic stays in `crates/core` (rules, safety checks, health score, Space Lens tree). | Keeps the rules and scoring testable without Windows. |
| 25 | Cleanup rules are our own declarative file, `rules/cleanup.json` (paths with `{VARIABLES}` and `*` segments, item level, age, include patterns, admin, safety, default selection, programs that lock the files). winapp2.ini is not used. | Its license (CC BY-NC-SA) is likely incompatible; our 14 rules cover the spec's list. |
| 26 | Every removal is checked twice: when scanned and again just before removal (`smd_core::safety::check_removal`): absolute drive path, no `..`, at least two folders deep, never `Windows`, `System32`, `WinSxS`, `Program Files*`, `ProgramData`, `Users`, `Recovery`, `Boot`, `EFI`, and inside the rule's own folder. Symlinks and junctions are never listed or removed. | Defense in depth; the `trash` crate resolves links before recycling, so a link must never reach it. |
| 27 | A cleanup only acts on the last scan, by id, and only on items the backend found itself; the UI sends category ids and the paths the user unticked. A used scan is discarded. | The UI can never ask the backend to delete an arbitrary path. |
| 28 | Admin-only categories (Windows temp, Windows Update downloads, Delivery Optimization, system error reports, memory dumps) are shown, measured when readable, and locked until the elevated helper (Phase 4). | Least privilege: no UAC prompt in Phase 1. |
| 29 | Recycle Bin by default, in batches of 100 through `IFileOperation` (`trash` crate: `FOF_ALLOWUNDO`, `FOF_WANTNUKEWARNING`); an item the shell skips (in use) is reported as skipped. "Delete for good" is a setting, off by default, with a warning and a confirmation dialog. | Reversible by default. Recycled junk only frees space once the bin is emptied, and the UI says so. |
| 30 | Emptying the Recycle Bin is its own category, never pre-selected, labelled "Can't be undone", and always confirmed. It runs **before** the other items are recycled. | Space is freed, and what this cleanup moves to the bin can still be restored. |
| 31 | Browser caches: only `Cache_Data`, `Code Cache`, `GPUCache` (Edge, Chrome, Brave, every profile) and Firefox `cache2`. Cookies, passwords and history are not offered at all in Phase 1. If the browser is running, the row says so and locked files are skipped. | Never touch personal browser data; the spec allows it only when ticked explicitly, which can come later. |
| 32 | Smart Care Phase 1 areas: disk health (`Get-PhysicalDisk`, no admin), free space on the system drive, memory (3 samples about 1 s apart; "sustained" = the lowest sample; critical needs commit charge > 90 % too), maintenance from uptime. Protection, startup load and app updates show "Checked from phase N" and are not scored. Battery is hidden on desktops. | Every number shown is measured; unmeasured areas never lower the score. |
| 33 | Smart Care's "Clean" button cleans only the categories ticked by default (no Recycle Bin, no admin). "Review" opens the same scan in Cleanup. | One click stays safe; anything more goes through the review list. |
| 34 | Space Lens walks with our own `rayon` walker instead of `jwalk`: on Windows `std::fs::DirEntry::metadata` reuses the `FindNextFileW` data, so no file is opened. Links/junctions/mount points are skipped; unreadable folders are counted. Each folder keeps its 40 largest files over 1 MB; the rest become one "small files" box. | Speed (all of C:, 589k files, in 62 s in a debug build) and bounded memory. |
| 35 | Sizes in Space Lens are file sizes (as Explorer's "Size"), not size on disk; hard links (WinSxS) may count twice. | Simple and matches Explorer. Size on disk and the MFT fast mode can come later. |
| 36 | Space Lens shows a squarified treemap (`d3-hierarchy`), two levels deep, plus a "largest items" list and a file-type breakdown. No sunburst for now. | The treemap reads better for "what is big"; the list gives full keyboard access. |
| 37 | Space Lens scans have their own busy flag, so Smart Care and Cleanup can run while a drive is being walked. | A 1 TB walk takes minutes; it is read-only. |
| 38 | My Activity: `activity.json` (newest 500 entries) and `undo\<id>.json` (recycled paths) in `%LOCALAPPDATA%\io.github.neverint.smd`. Undo restores from the Recycle Bin items with the same original path, deleted after the cleanup started. | Undo works across restarts; nothing leaves the PC. |
| 39 | Backend settings live in `settings.json` in the same folder: exclusions and delete mode. "Always skip" in a review list adds an exclusion. | Settings the Rust side must enforce cannot live in browser storage (decision 14). |
| 40 | Progress is streamed with Tauri channels (`on_progress`); the frontend `stream()` helper mirrors `call()` and has mocks for the browser and Playwright. | Typed, ordered progress without global events. |
