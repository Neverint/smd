# Third-party licenses

Scrub My Disks is MIT-licensed. Every dependency below was checked for compatibility before it was added.
Update this file whenever a crate, npm package, data file or bundled binary is added.

## Shipped in the app

### Rust crates (direct)

| Crate | Version | License | Use |
|---|---|---|---|
| tauri | 2.12 | Apache-2.0 OR MIT | App framework |
| tauri-build | 2.7 | Apache-2.0 OR MIT | Build script |
| tauri-plugin-log | 2.10 | Apache-2.0 OR MIT | Rotating local log file |
| tauri-plugin-dialog | 2.8 | Apache-2.0 OR MIT | Folder picker (Space Lens) |
| tauri-plugin-opener | 2.7 | Apache-2.0 OR MIT | "Show in Explorer", open the log folder |
| window-vibrancy | 0.8 | Apache-2.0 OR MIT | Mica / acrylic window material |
| windows | 0.62 | MIT OR Apache-2.0 | Win32 APIs: drives, memory, processes, Recycle Bin |
| trash | 5.3 | MIT | Move to / restore from the Recycle Bin |
| rayon | 1.12 | MIT OR Apache-2.0 | Parallel disk walk and rule scanning |
| serde, serde_json | 1.0 | MIT OR Apache-2.0 | Serialization |
| thiserror | 2.0 | MIT OR Apache-2.0 | Typed errors |
| log | 0.4 | MIT OR Apache-2.0 | Logging facade |

Transitive crates (Windows target, checked again 2026-11-11 for Phase 1): all MIT, Apache-2.0, BSD, ISC, Zlib, Unicode-3.0, Unlicense, CC0, BSL-1.0 or 0BSD, except these **MPL-2.0** crates pulled in by Tauri: `cssparser`, `cssparser-macros`, `selectors`, `dtoa-short`, `option-ext`. MPL-2.0 is file-level copyleft: it is compatible with an MIT app as long as these crates are used unmodified (their source is on crates.io). We do not modify them.

### npm packages bundled into the frontend

| Package | License | Use |
|---|---|---|
| react, react-dom | MIT | UI |
| react-router | MIT | Routing |
| zustand | MIT | State |
| motion (Framer Motion) | MIT | Animations |
| i18next, react-i18next | MIT | French / English |
| @fluentui/react-icons | MIT | Fluent UI System Icons |
| @tauri-apps/api | Apache-2.0 OR MIT | Backend bridge |
| d3-hierarchy | ISC | Space Lens treemap layout |

### Assets

| Asset | License |
|---|---|
| App icon and module illustrations | Original work, MIT (this repository) |
| Segoe UI Variable, Segoe Fluent Icons | Not bundled: system fonts of Windows |

## Development only (not shipped)

vite (MIT), @vitejs/plugin-react (MIT), typescript (Apache-2.0), vitest (MIT), jsdom (MIT), @testing-library/* (MIT), @playwright/test (Apache-2.0), eslint and plugins (MIT), @tauri-apps/cli (Apache-2.0 OR MIT), @types/d3-hierarchy (MIT), tempfile crate (MIT OR Apache-2.0, Rust tests only). Their own dependencies include a few ISC, BSD, BlueOak-1.0.0, MPL-2.0 and CC-BY-4.0 (browser compatibility data) packages, none of which end up in the installer.

## Planned (to verify when added)

| Item | License | Phase | Note |
|---|---|---|---|
| sysinfo | MIT | 2 | Live CPU / RAM per process (Performance). Phase 1 uses Win32 calls instead |
| image_hasher | MIT OR Apache-2.0 | 2 | Similar images |
| LibreHardwareMonitorLib | MPL-2.0 | 3 | Optional .NET sidecar; license file shipped with it |
| winapp2.ini | CC BY-NC-SA 3.0 (to confirm) | later | Not used in Phase 1 (own rules in `rules/cleanup.json`). Likely **not** bundleable; download at runtime with consent if ever used |
| Toolbox tools (BCUninstaller, Krokiet, WinDirStat, ...) | various | 4 | Not bundled: installed by winget on request |
