# Scrub My Disks (SMD)

[Français](README.fr.md)

A free, open-source maintenance app for Windows 10 and 11. It brings the best free PC-health tools together in one place, with a calm, guided scan flow and Windows 11 materials (Mica, Segoe UI Variable, Fluent icons).

> **Status: Phase 3.** Protection (Microsoft Defender, firewall, app permissions), Hardware Health (live sensors, drives, memory, battery), Smart Care (complete health score), Cleanup (review, then Recycle Bin), Space Lens (drive map), Performance (startup items, live programs, background tasks), Applications (uninstall with leftovers review, app updates), My Clutter (large, old and duplicate files) and My Activity (history with undo) work. The other modules arrive phase by phase; see the roadmap below.

## Safety principles

1. **Look before you touch.** Every module scans read-only first. Nothing changes until you review the results and click the action button.
2. **Reversible by default.** Files go to the Recycle Bin. Startup items are disabled, not deleted. Every tweak has an undo. A System Restore point is created before system changes.
3. **Least privilege.** The app runs without admin rights and asks only when a specific batch needs it, saying why.
4. **No snake oil.** No registry cleaner, no "RAM booster", no fake speed-up numbers. Every number is measured.
5. **Privacy.** No telemetry, no account. The app only goes online for winget, updates and Toolbox downloads, and tells you when.
6. **Free only.** Every dependency is free and license-compatible. See [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md).

## Modules and roadmap

| Module | What it does | Phase |
|---|---|---|
| Smart Care | One scan, a health score out of 100, and the top fixes | 1 |
| Cleanup | Temporary files, caches, crash dumps, browser caches | 1 |
| Space Lens | Interactive map of what fills a drive | 1 |
| My Activity | History of every scan and action, with undo | 1 |
| Applications | Clean uninstaller with leftovers review, winget updates | 2 |
| Performance | Startup apps, scheduled tasks, services, boot time | 2 |
| My Clutter | Large, old and duplicate files | 2 |
| Protection | Microsoft Defender status and scans, firewall, app permissions | 3 |
| Hardware Health | Temperatures, drive health (SMART), battery, memory | 3 |
| Drive Organizer | Keeps each drive for its job and moves things safely | 3b |
| Privacy & Tweaks | Reversible privacy and debloat tweaks | 4 |
| Toolbox | Installs trusted specialist tools through winget | 4 |

## Build from source

Requirements (Windows 10/11):

- [Node.js](https://nodejs.org) 24 or later
- [Rust](https://rustup.rs) (stable, MSVC toolchain)
- Visual Studio 2022 Build Tools with the **Desktop development with C++** workload
- Microsoft Edge WebView2 (preinstalled on Windows 11)
- [.NET 8 SDK](https://dotnet.microsoft.com/download/dotnet/8.0) for the sensor helper (users only need the .NET 8 runtime)

```bash
npm install
npm run hwmon          # build the sensor helper (once, and after changing sidecars/hwmon)
npm run tauri dev      # run the app
npm test               # unit tests
npm run e2e            # smoke tests (uses Microsoft Edge)
npm run tauri build    # NSIS + MSI installers
```

Unsigned builds trigger a Windows SmartScreen warning. Code signing through SignPath Foundation is planned.

## License

MIT, see [LICENSE](LICENSE). Third-party credits: [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md).
Inspired by the layout of CleanMyMac. Scrub My Disks is not affiliated with MacPaw and uses none of its assets.
