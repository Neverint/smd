# Performance budget

Scrub My Disks should feel instant and cost nothing while it waits. These budgets are checked
before each release; the frontend ones also run in CI (`npm run budget`).

| What | Budget | Measured (v0.1.0) | How it is checked |
|---|---|---|---|
| Startup JavaScript (gzip) | ≤ 240 KB | 213 KB | `npm run budget` (CI) |
| Startup CSS (gzip) | ≤ 12 KB | 7.8 KB | `npm run budget` (CI) |
| Largest page chunk (gzip) | ≤ 12 KB | 5.0 KB | `npm run budget` (CI) |
| Time to first paint | ≤ 1.5 s | 0.5 s (0.7 s first start) | release build, 3 runs, `first-contentful-paint` after process start |
| Idle CPU (window open, nothing running) | ≤ 0.5 % | 0.02 % | release build, CPU time of the app and its WebView2 processes over 30 s |
| Idle memory, app process | ≤ 50 MB private | 9 MB | release build, after 20 s |
| Idle memory, app + WebView2 | ≤ 250 MB private | 210 MB | same, all child processes |
| NSIS installer | ≤ 10 MB | 4.2 MB | `npm run tauri build` |
| MSI installer | ≤ 12 MB | 5.7 MB | same |

Measured on 2026-11-12 on the developer's PC (Windows 11, 12 logical processors, WebView2
runtime from Microsoft Edge). WebView2 (the Edge engine Windows provides) is most of the memory;
the app's own process stays small.

## What keeps it light

- Only Smart Care (the home page) loads at start. Every other page, and the fake backend used
  for browser development and tests, is loaded when first needed.
- Nothing polls while idle. Live numbers (Performance's running programs, Hardware Health's
  sensors, the tray popover) are sampled only while their page or popover is open, and stop when
  it closes.
- Long scans run on background threads with progress events; the window never waits on them.
- The sensor helper (.NET) runs only while Hardware Health is open.
- The wrong-drive guard waits on Windows folder-change events (`ReadDirectoryChangesW`), not on a
  timer that walks the disks; it audits once, 10 minutes after the last change.

## Measuring again

1. `npm run budget` for the frontend numbers.
2. `npm run tauri build`, then start `target\release\smd.exe` with
   `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9334`, read
   `performance.getEntriesByName("first-contentful-paint")` over the debug port, and sum
   `PrivateMemorySize64` and `TotalProcessorTime` of the process and its children.
