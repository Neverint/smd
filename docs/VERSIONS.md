# Versions and change names

How versions are numbered, how a version goes from test build to standard release, and how changes are named in the changelog, commits and pull requests.

## Version numbers

`MAJOR.MINOR.PATCH`, numbers only.

| Part | Before 1.0 | Example |
|---|---|---|
| MINOR (`0.x.0`) | One step of the [roadmap](ROADMAP.md): new features or modules. | 0.2.0: code signing and in-app updates |
| PATCH (`0.x.y`) | Fixes and small changes inside a step. No new module. | 0.1.1: fixes from the first real use of 0.1.0 |
| MAJOR (`1.0.0`) | The roadmap's 1.0 criteria are met. | 1.0.0 |

After 1.0, MAJOR goes up only when something cannot carry over on its own (a settings or data format, a removed feature).

No `-beta` or `-rc` suffix: the MSI installer accepts only numbers. Whether a version is in testing or released is shown by its changelog heading and by whether its pull request is merged, not by the number.

## Life of a version

1. **Testing.** The version number is set on its own branch, and the pull request is titled `X.Y.Z: <what it brings>`. The changelog heading is `## X.Y.Z (testing)`. The installer is built locally (`npm run hwmon`, then `npm run tauri build`) and installed on the developer's PC. The pull request is **not merged**.
   - A bug found in testing is fixed on the same branch, with the same number: the version is not out yet. The developer data log tells builds of the same number apart by the exe's date. <!-- DEVLOG -->
2. **Stable.** A test version is stable when all of these are true:
   - it was used normally for a few days, including every action its changelog lists;
   - every change in its changelog was checked once in the installed app;
   - `npm run devlog` shows no panic, no failed action and no session without an end line that is not explained; <!-- DEVLOG -->
   - all checks pass (lint, types, unit, browser and Rust tests, budget).
3. **Released (standard version).** Only once it is stable:
   - merge the pull request into `main`;
   - give the changelog heading its date: `## X.Y.Z (YYYY-MM-DD)`;
   - update the version line in `README.md` and `README.fr.md`;
   - push the tag `vX.Y.Z`: the release workflow builds the installers into a draft GitHub Release, published with the changelog section.

The next version starts from `main` after the release, with the next number.

## Change names

### Changelog

Each version has these headings, in this order, and only the ones with entries:

| Heading | For |
|---|---|
| **Added** | Something new the user can do or see. |
| **Changed** | Something that already existed works or looks different. |
| **Fixed** | A bug. Say what went wrong and when, then what happens now. |
| **Removed** | Something taken out. |
| **Safety** | Anything about what the app removes or moves, admin rights, or going online (the trust contract). |
| **For developers** | No effect on the user: tests, tools, logs, docs. |

One sentence per entry, written for the user, in the same plain words as the app.

### Commits

The first line starts with the kind of change, the same words as the changelog headings:

- `add: <what>` · `change: <what>` · `fix: <what>` · `remove: <what>` · `safety: <what>` · `dev: <what>` (tests, tools, docs)
- `release: X.Y.Z` for the commit that sets a version number or dates its changelog.

Example: `fix: parallel saves of one settings file no longer fail`.

### Pull requests

One pull request per version, titled `X.Y.Z: <what it brings>`. Its description lists the changes under the same headings as the changelog, then the tests run and anything not verified.
