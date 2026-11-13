# Roadmap: from 0.1.0 to 1.0

Version 0.1.0 has every module working and tested. Version 1.0 means you can install it on any Windows 10 or 11 PC, trust it with your files, and keep it up to date without thinking about it. The trust contract in [CLAUDE.md](../CLAUDE.md) does not change on the way: read-only first, reversible by default, least privilege, no snake oil, no telemetry, free only.

Each version below lists what it delivers and what must be true before the next one starts. The order matters more than the dates, so there are none.

## 0.1.x: first real-world use

Fixes from installing and using 0.1.0 on real PCs.

- Try every admin action once with a real Windows prompt: Read SMART details (read-only), CPU sensors, Windows cleanup, an all-users startup item, a machine-wide tweak with its restore point, an admin leftover.
- Try a full round on a second PC or a virtual machine: Smart Care, Cleanup, undo from My Activity, a Drive Organizer move on a test folder, scheduled Smart Care, uninstall (the scheduled task must disappear).
- Fix what breaks, with a test for each fix.

**Done when:** every admin operation has worked with a real Windows prompt, and no bug that loses or corrupts data is open.

## 0.2: trusted installs and updates

- **Code signing** through the [SignPath Foundation](https://signpath.org) (free for open-source projects): the installers and `smd.exe` are signed, so SmartScreen stops warning over time. The release workflow already has the hook.
- **In-app updates** with Tauri's updater: a signed update file on GitHub Releases, checked at most once a day, only after the user says yes the first time (the trust contract says the app tells you when it goes online). The update shows its changelog before it installs.
- **winget package** `Neverint.ScrubMyDisks`, so the app can also be installed with `winget install`.
- **First start**: a short welcome screen (what the app does, what it never does, the drive roles to check) instead of landing straight on Smart Care.

**Done when:** a signed 0.2 updates itself to a signed 0.2.1 on a test PC, and `winget install` works.

## 0.3: wider coverage

- **Drive Organizer**: EA app, Ubisoft Connect and Battle.net games (from their own install records, never by guessing).
- **Applications**: registry leftovers under `HKLM\SOFTWARE` (backed up with `reg export` by the admin helper before removal, restored on undo).
- **Cleanup**: more rules, each one reviewed: Discord, Teams, Spotify and Steam caches; npm, pip and NuGet caches (with a note that they refill); old Windows update leftovers through Windows' own `DISM /StartComponentCleanup` (official tool, admin, no restore of old updates afterwards, so it is a "check first" rule).
- **My Clutter**: similar photos (perceptual hash, `image_hasher`), with side-by-side preview.
- **Hardware Health**: history of disk temperatures and SMART errors (kept locally), with a warning when a counter rises.

**Done when:** every new rule and detection has tests, and each one was checked read-only on at least two PCs.

## 0.4: history and gentle automation

- **Smart Care history**: a chart of the score and the space freed over time, from My Activity.
- **Scheduled actions stay scan-only** (trust contract). New: schedule the quarantine purge and the update check, and let notifications say exactly what a fix would do, with a button that opens the review.
- **Notification settings**: choose which ones you get (scan results, finished tasks, wrong-drive guard, updates).
- **Export and import settings** (exclusions, drive roles, schedule) as a JSON file, for a new PC.

**Done when:** a week of use with the schedule on shows nothing ran that the user did not start, apart from scans.

## 0.5: accessibility and languages

- **Manual screen reader pass** with Narrator and NVDA on every page and flow (the automated axe check is already in CI).
- **Windows high contrast themes** (`forced-colors`), keyboard-only use of every flow, focus order checked.
- **Text size**: the layout holds at 200 % text scaling.
- **More languages** through community translations (German, Spanish, Italian, Portuguese first), with a check that every language has every key.

**Done when:** a Narrator user and a keyboard-only user can each complete Smart Care, Cleanup with review, and a tweak with undo.

## 0.6: security and reliability hardening

- **Threat model** written down for the admin helper, the pipe and every typed operation; an outside review if someone offers one.
- **Fuzz tests** for every parser that reads outside data: Steam `vdf`, Epic manifests, battery report XML, winget and PowerShell JSON, tweak and cleanup catalogs.
- **Supply chain**: `cargo deny` and `npm audit` in CI (licenses and known vulnerabilities), a software bill of materials (SBOM) attached to each release, pinned GitHub Actions.
- **Crash safety**: a local crash log (never sent anywhere) and a "copy details" button; interrupted cleanups and moves always resume or roll back.

**Done when:** the fuzzers run in CI without findings for a week, and the threat model has no open "high" item.

## 0.9: public beta

- **Test matrix**: Windows 10 22H2 and Windows 11 (23H2, 24H2, 25H2), x64 and ARM64 builds, a laptop with a battery, a PC with one drive only, a non-admin account, a PC in French.
- **Documentation site** (GitHub Pages): what each module does, what it never does, how undo works, FAQ.
- **Beta channel** in the updater for people who want early builds.
- A month of beta with issue templates for bugs and new cleanup rules.

**Done when:** a month of beta passes with no open critical bug and no data loss reported.

## 1.0

Released when all of this is true:

- Signed installers and signed updates; `winget install` works.
- Tested on the whole 0.9 matrix.
- No known bug that loses data; every destructive action has a review step and an undo or a stated reason why not.
- The performance budgets in [PERFORMANCE.md](PERFORMANCE.md) still hold.
- Accessibility: the automated check in CI, plus the manual Narrator and keyboard passes.
- English and French complete; other languages marked as community translations.
- Threat model and SBOM published with the release.

## Not planned

These stay out because they break the trust contract or add little: a registry cleaner, a "RAM booster" or memory "optimizer", driver updaters, automatic deletion on a schedule, telemetry or an account, a paid tier, bundling third-party tools in the installer (the Toolbox installs them on request instead).
