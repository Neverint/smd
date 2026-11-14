# Developer data log (until 1.0)

A local log of every app session, so problems found while using the app can be traced afterwards. It exists only for the road to 1.0 and is removed before 1.0 (a Rust test fails from version 1.0.0 on while it is still in the code).

It fits the trust contract: nothing is sent anywhere, the files stay in the app's own local folder, and old sessions are deleted automatically.

## Where and what

`%LOCALAPPDATA%\io.github.neverint.smd\devlog\<start unix ms>-<pid>.jsonl`, one file per app session, one JSON object per line. Every line has `t` (unix ms, written by the backend) and `k` (kind). The last 100 sessions are kept; a session stops at 20 MB (a `truncated` line says so).

| `k` | Written by | Content |
|---|---|---|
| `start` | backend | version, debug or release, exe size and date, arguments (`--smart-care` for the scheduled run), Windows version, time zone offset |
| `log` | backend | every backend log line (info and above), with level and module |
| `activity` | backend | every My Activity entry, before it is saved (so a lost entry still shows here) |
| `panic` | backend | message, file and line, thread, backtrace (release builds abort on panic, so this is the only trace) |
| `end` | backend | clean exit; **a file without `end` means the app was killed or crashed** |
| `ui.open`, `ui.load` | UI | window (main or tray), size, scale, language, theme, first paint |
| `ui.cmd` | UI | every backend command: name, arguments (shortened), time in ms, result size or error `{code, message, details}` |
| `ui.polls` | UI | polled commands (tray, sensors, processes), counted per flush instead of one line per call |
| `ui.page`, `ui.click` | UI | page changes; clicked buttons, links, tabs and checkboxes with their label |
| `ui.error`, `ui.rejection`, `ui.console` | UI | uncaught errors, unhandled promise rejections, `console.error` and `console.warn` |
| `ui.longtask`, `ui.focus`, `ui.visibility` | UI | UI freezes over 200 ms, window focus, hidden to tray |

UI events carry `route` (the page) and `ut` (the UI's own time).

## Reading it

```powershell
npm run devlog                      # the last 5 sessions
npm run devlog -- --last 20
npm run devlog -- --all
npm run devlog -- --session 1794665670365   # one session
npm run devlog -- --json            # the digest as JSON
```

The digest shows, per session: local start time, version, length, whether it ended cleanly, pages visited, actions (commands that change something), My Activity entries saved, slow commands (3 s or more), errors and warnings, and panics. For details, read the `.jsonl` file itself.

To turn it off for one run, start the app with the environment variable `SMD_DEVLOG=0`.

## Removing it (before 1.0)

```powershell
npm run devlog:remove -- --dry-run   # shows what would change
npm run devlog:remove
```

The script deletes `src-tauri/src/devlog.rs`, `src/dev/devlog.ts`, `src/dev/devlog.test.ts`, `scripts/devlog.mjs`, this file and itself; deletes every line marked `// DEVLOG` or `<!-- DEVLOG -->` (and blocks between `DEVLOG start` and `DEVLOG end`); removes the two npm scripts; runs `cargo fmt`; and lists any "devlog" text left. Then run the usual checks. The logs already on a PC are not touched: delete the `devlog` folder by hand.

Every hook into the rest of the app is one marked line, so the removal is mechanical:

- `src-tauri/src/lib.rs`: `mod devlog`, `devlog::start()`, the log target, the `devlog_write` command, the exit event.
- `src-tauri/src/activity.rs`: the `activity` event.
- `src/main.tsx`: the import that starts the UI side.
- `src/lib/backend.ts`: the import and one `devlogCommand(...)` line in `call` and in `stream`.

New hooks must follow the same rule: one line, ending with `// DEVLOG`, and the code must still compile with that line gone.
