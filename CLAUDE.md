# CLAUDE.md: Scrub My Disks (SMD)

Free, open-source Windows 10/11 maintenance app. Tauri 2 (Rust) + React/TypeScript/Vite.
Layout and scan flow inspired by CleanMyMac, Windows 11 materials. All assets are original.

Repo: `Neverint/smd` (MIT). Local path: `G:\Documents\Dev\smd`.
Commit identity: `Neverint <Neverint@users.noreply.github.com>` (set in this repo's git config).

## The trust contract (non-negotiable)

1. **Look before you touch.** Every module scans read-only first and shows results. Nothing changes until the user reviews and clicks the action button.
2. **Reversible by default.** Deletions go to the Recycle Bin (or `S:\Quarantine` when quarantine mode is on). Permanent delete is an explicit opt-in. Startup items are disabled through `StartupApproved`, never deleted. Every tweak has an undo. A System Restore point is created before any batch of system changes (admin, 1 per 24 h by default: handle and report that case).
3. **Least privilege.** The app runs non-elevated. Admin work goes through a separate elevated helper, started with UAC only for the batch that needs it, running only allow-listed typed operations. The UI says why admin is needed.
4. **No snake oil.** No registry cleaner, no RAM booster, no fake speed claims. Every number shown is measured.
5. **Privacy.** No telemetry, no account. Network only for winget, updates and Toolbox downloads, and the UI says when. Logs stay local.
6. **Free only.** Check every dependency's license before adding it, and record it in `THIRD_PARTY_LICENSES.md`.

Hard "never" list (drive layout): never move or junction `C:\Users`, `C:\ProgramData`, `C:\Program Files*`, `C:\Windows`, `WinSxS`; never change `ProgramFilesDir` or profile paths in the registry; never symlink AppData; never put system TEMP/TMP or the pagefile on S:; never remove the pagefile from C:; never compress the Windows folder; never convert partition styles; never touch Recovery or EFI partitions.

## Layout

```
src/                 React frontend
  app/               shell: App (router), TitleBar, Sidebar, modules.ts (module registry)
  modules/           one folder per page (smart-care, cleanup, space-lens, performance, applications, clutter, activity, settings); ModulePage = placeholder
                     shared/ScanFlow.tsx (progress ring, "done" summary), modules.css
  components/        shared UI (Button, Card, ProgressRing, SideSheet, CheckList, ConfirmDialog, ErrorNotice, UsageBar, ...)
  design/            tokens.css, module-accents.css, base.css (see docs/design-system.md)
  dev/gallery/       /#/dev/gallery: every component in both themes
  i18n/              en.json, fr.json (same keys, enforced by a test)
  stores/            Zustand stores (settings, cleanup, smartCare, spaceLens, clutter)
  lib/               backend.ts (typed `call()` / `stream()` + mocks outside Tauri), types.ts, mocks.ts, mocks-phase2.ts, format.ts, useFormat.ts, theme.ts
e2e/                 Playwright smoke tests (Edge, mocked backend; `__SMD_FAST_MOCKS__` skips mock delays)
crates/core/         pure logic, no OS access: cleanup rules, safety checks, health score, Space Lens tree,
                     command-line parsing, startup values, app-name matching, formatting
crates/win/          Windows access: drives, memory, Recycle Bin, disk walk, cleanup, startup items, processes,
                     tasks/services, installed apps, winget, My Clutter
src-tauri/           Tauri app: one file per module's commands (cleanup, smart_care, space_lens, performance, applications,
                     clutter, activity, settings),
                     state.rs, store.rs (JSON files), error.rs (typed AppError), window_effect.rs
rules/               cleanup.json (declarative cleanup rules); tweaks.json and the drive policy come later
sidecars/hwmon/      .NET 8 LibreHardwareMonitor helper; `npm run hwmon` builds it into src-tauri/resources/hwmon (not committed)
docs/                DECISIONS.md, design-system.md
```

## Commands

Rust lives on G: (not on PATH in older shells). In a fresh PowerShell:

```powershell
$env:Path += ';G:\Documents\Dev\.rust\cargo\bin'; $env:RUSTUP_HOME='G:\Documents\Dev\.rust\rustup'; $env:CARGO_HOME='G:\Documents\Dev\.rust\cargo'
```

| Task | Command |
|---|---|
| Run the desktop app | `npm run tauri dev` |
| Frontend only (mocked backend) | `npm run dev` then http://localhost:1420 |
| Lint / types / unit tests | `npm run lint`, `npm run typecheck`, `npm test` |
| Smoke tests | `npm run e2e` |
| Rust checks | `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` |
| Sensor helper | `npm run hwmon` (needs the .NET 8 SDK; on this PC it is in `G:\Documents\Dev\.dotnet`: add it to PATH first). Required before any Rust build: tauri-build checks the bundled resources. |
| Release build | `npm run hwmon`, then `npm run tauri build` (NSIS + MSI in `target/release/bundle`) |

## Conventions

- Every backend command returns `AppResult<T>`; errors serialize as `{ code, message, details }`. Add each command to `Commands` in `src/lib/backend.ts` and a mock in `src/lib/mocks.ts`.
- Pure logic goes in `crates/core` with unit tests. Windows API and file system code goes in `crates/win`.
- Anything that removes files goes through `smd_win::cleanup::remove_items`, which re-checks `smd_core::safety::check_removal`. Never remove a path the UI sent: act on the backend's own scan, by id.
- Long tasks take a `Channel<Progress>` (`on_progress`) and run in `spawn_blocking`; guard them with `AppState::begin` so two never overlap.
- Debug the real app with Playwright: start it with `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9333`, then `chromium.connectOverCDP("http://127.0.0.1:9333")`. Never click a destructive button on the developer's own PC.
- PowerShell only where it is the cleanest official API (Defender, WinGet, scheduled tasks, Appx): `-NoProfile -NonInteractive -ExecutionPolicy Bypass`, output with `ConvertTo-Json -Depth 4`, parsed into typed structs. Scripts are `&'static str`; values go in as `SMD_*` environment variables (`smd_win::powershell::run_json_with`). Never build commands by concatenating input.
- External programs (uninstallers, msiexec, reg.exe) are started directly or through `ShellExecuteEx`, never through `cmd /c`.
- All UI text goes through i18n (EN + FR). Plain, friendly wording: say what an item is and whether it is safe to remove. No fear-based copy.
- Colors only through tokens. Text on a module color uses `--m-solid` / `--m-on` (AA contrast); `--m-from` / `--m-to` are decorative only.
- Icon-only buttons need a label (`IconButton` enforces it). Respect `prefers-reduced-motion` (`MotionConfig reducedMotion="user"`).
- Record non-obvious choices in `docs/DECISIONS.md`.
- Work phase by phase. At the end of each phase: build, run, test, commit, push, summarize, wait for "go".
