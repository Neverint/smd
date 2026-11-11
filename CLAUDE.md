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
  modules/           one folder per page (smart-care, settings, ...); ModulePage = placeholder
  components/        shared UI (Button, Card, ProgressRing, SideSheet, CheckList, ...)
  design/            tokens.css, module-accents.css, base.css (see docs/design-system.md)
  dev/gallery/       /#/dev/gallery: every component in both themes
  i18n/              en.json, fr.json (same keys, enforced by a test)
  stores/            Zustand stores
  lib/               backend.ts (typed `call()` + mocks outside Tauri), format.ts, theme.ts
e2e/                 Playwright smoke tests (Edge, mocked backend)
crates/core/         pure logic, no OS access: health score, formatting, rules
src-tauri/           Tauri app: commands.rs, error.rs (typed AppError), window_effect.rs
rules/, data/        declarative cleanup rules, tweaks.json, drive policy (from Phase 1)
sidecars/hwmon/      .NET LibreHardwareMonitor sidecar (Phase 3)
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
| Release build | `npm run tauri build` (NSIS + MSI in `target/release/bundle`) |

## Conventions

- Every backend command returns `AppResult<T>`; errors serialize as `{ code, message, details }`. Add each command to `Commands` in `src/lib/backend.ts` and a mock in `src/lib/mocks.ts`.
- Pure logic goes in `crates/core` with unit tests. Windows API code goes in a `crates/win` crate (created in Phase 1).
- PowerShell only where it is the cleanest official API (Defender, WinGet, scheduled tasks, Appx): `-NoProfile -NonInteractive -ExecutionPolicy Bypass`, output with `ConvertTo-Json -Depth 4`, parsed into typed structs. Never build commands by concatenating user input.
- All UI text goes through i18n (EN + FR). Plain, friendly wording: say what an item is and whether it is safe to remove. No fear-based copy.
- Colors only through tokens. Text on a module color uses `--m-solid` / `--m-on` (AA contrast); `--m-from` / `--m-to` are decorative only.
- Icon-only buttons need a label (`IconButton` enforces it). Respect `prefers-reduced-motion` (`MotionConfig reducedMotion="user"`).
- Record non-obvious choices in `docs/DECISIONS.md`.
- Work phase by phase. At the end of each phase: build, run, test, commit, push, summarize, wait for "go".
