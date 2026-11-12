# Changelog

## 0.1.0 (2026-11-12)

First release. Windows 10 and 11, x64.

### Modules

- **Smart Care**: one read-only scan and a health score out of 100 (disk health, free space, memory, uptime and updates, startup load, protection, battery, drive layout) with the top fixes. Optional daily or weekly run from Task Scheduler: scan only, the score in a notification.
- **Cleanup**: 14 declarative rules (temp files, thumbnail cache, crash dumps, error reports, browser caches, Recycle Bin), review lists, "always skip", Recycle Bin or quarantine, undo. Windows files (system temp, update cache, Delivery Optimization, memory dumps) with admin rights.
- **Space Lens**: a treemap of any drive or folder, largest items, file types.
- **Protection**: Microsoft Defender status, quick and full scans, signature updates, firewall profiles, camera, microphone and location permissions.
- **Performance**: startup items (turned off through `StartupApproved`, like Task Manager; all-users items with admin rights), programs running now, scheduled tasks, services, boot time.
- **Applications**: uninstall with a leftovers review (folders, registry keys, and Program Files / ProgramData folders with admin rights), app updates through winget, Windows updates waiting.
- **My Clutter**: large, old and duplicate files (BLAKE3), Downloads review.
- **Hardware Health**: live sensors (LibreHardwareMonitor helper), CPU temperature and fans with admin rights and PawnIO, SMART details, battery wear, memory modules.
- **Drive Organizer**: drive roles bound to volume IDs, an audit of games and files on the wrong drive, dry-run plans with checks, journaled copy-verify moves with resume and undo, quarantine and scratch folders on a spare drive.
- **Privacy & Tweaks**: 38 reversible changes (privacy, ads and suggestions, AI, File Explorer, built-in Store apps) with their live state, an exact undo each, and a restore point before machine-wide batches.
- **Toolbox**: 13 trusted free tools installed, updated and opened through winget, with their licenses.
- **My Activity**: every scan and action, with undo where possible.
- **Tray**: a mini monitor (processor, memory, disk, network, free space, last score) with quick actions; the app keeps running while a task finishes and notifies when it is done; optional notifications when a game lands on the wrong drive.

### Safety

- Read-only scans first; nothing changes before you review and click.
- Recycle Bin or quarantine by default; permanent delete is an explicit opt-in.
- An elevated helper runs only a fixed list of typed operations, each checked again against the built-in catalogs; one Windows prompt per session.
- No telemetry, no account; network only for winget, updates and Toolbox installs.

### Quality

- 120+ Rust tests, unit tests, smoke tests, and an axe accessibility check (WCAG 2.1 A/AA) of every page in light and dark.
- Performance budget in CI (startup JavaScript and CSS); release numbers in `docs/PERFORMANCE.md`.
- English and French.

### Known limits

- The installers are not code-signed yet (Windows SmartScreen warns).
- Live sensors need the .NET 8 Desktop Runtime.
- EA app, Ubisoft Connect and Battle.net games are not detected by the Drive Organizer.
