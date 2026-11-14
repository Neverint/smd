// DEVLOG: reads the developer data log and prints a digest of the last sessions.
// Usage: npm run devlog [-- --last 5] [-- --all] [-- --session <file name>] [-- --json]
// See docs/DEVLOG.md. Removed with `npm run devlog:remove` before 1.0.

import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const dir = path.join(process.env.LOCALAPPDATA ?? "", "io.github.neverint.smd", "devlog");
const argv = process.argv.slice(2);
const opt = (name) => {
  const i = argv.indexOf(`--${name}`);
  return i >= 0 ? (argv[i + 1] ?? true) : undefined;
};

/** Commands that change something on the PC (or the app's own settings). */
const ACTIONS = new Set([
  "cleanup_run", "activity_undo", "startup_set", "processes_stop", "apps_uninstall", "apps_remove_leftovers",
  "updates_apply", "updates_setup", "clutter_remove", "permission_set", "organizer_policy_set", "organizer_run",
  "organizer_resume", "quarantine_restore", "quarantine_purge_consent", "tweaks_apply", "tweak_undo",
  "restart_explorer", "toolbox_install", "schedule_set", "settings_set", "protection_scan",
  "protection_update_signatures", "cleanup_admin_scan", "sensors_admin_poll", "disk_reliability",
]);
const SLOW_MS = 3000;

if (!fs.existsSync(dir)) {
  console.log(`No developer log yet (${dir}). Start the app once.`);
  process.exit(0);
}

let files = fs.readdirSync(dir).filter((f) => f.endsWith(".jsonl")).sort();
const one = opt("session");
if (typeof one === "string") files = files.filter((f) => f.includes(one));
else if (!opt("all")) files = files.slice(-Number(opt("last") ?? 5));

const read = (file) =>
  fs
    .readFileSync(path.join(dir, file), "utf8")
    .split("\n")
    .filter(Boolean)
    .map((l) => {
      try {
        return JSON.parse(l);
      } catch {
        return { k: "badline", raw: l.slice(0, 200) };
      }
    });

const pad = (n) => String(n).padStart(2, "0");
function local(t, tz) {
  const d = new Date(t + (tz ?? 0) * 60000);
  return `${d.getUTCFullYear()}-${pad(d.getUTCMonth() + 1)}-${pad(d.getUTCDate())} ${pad(d.getUTCHours())}:${pad(d.getUTCMinutes())}:${pad(d.getUTCSeconds())}`;
}
const dur = (ms) => (ms >= 60000 ? `${(ms / 60000).toFixed(1)} min` : ms >= 1000 ? `${(ms / 1000).toFixed(1)} s` : `${ms} ms`);
const short = (v, n = 160) => {
  const s = typeof v === "string" ? v : JSON.stringify(v);
  return s && s.length > n ? `${s.slice(0, n)}…` : s;
};

const digests = files.map((file) => {
  const ev = read(file);
  const start = ev.find((e) => e.k === "start") ?? {};
  const tz = start.tzOffsetMin;
  const end = ev.find((e) => e.k === "end");
  const first = ev[0]?.t ?? 0;
  const last = ev.at(-1)?.t ?? first;
  const cmds = ev.filter((e) => e.k === "ui.cmd");
  const pages = [];
  for (const e of ev) {
    if (!e.route || !e.k.startsWith("ui.")) continue;
    const p = e.route.replace(/^#\/?/, "") || "smart-care";
    if (pages.at(-1) !== p) pages.push(p);
  }
  const errors = [
    ...cmds.filter((c) => !c.ok).map((c) => `cmd ${c.cmd} failed after ${dur(c.ms)}: [${c.error?.code}] ${c.error?.message}`),
    ...ev.filter((e) => e.k === "log" && (e.level === "WARN" || e.level === "ERROR")).map((e) => `log ${e.level} ${e.target}: ${e.msg}`),
    ...ev.filter((e) => e.k === "ui.error").map((e) => `ui error: ${e.message} at ${e.at}`),
    ...ev.filter((e) => e.k === "ui.rejection").map((e) => `ui unhandled: ${e.message}`),
    ...ev.filter((e) => e.k === "ui.console").map((e) => `console.${e.level}: ${short(e.msg, 200)}`),
    ...ev.filter((e) => e.k === "ui.polls" && e.errors > 0).map((e) => `poll ${e.cmd}: ${e.errors}/${e.n} failed`),
  ];
  const panics = ev.filter((e) => e.k === "panic");
  return {
    file,
    started: local(first, tz),
    version: start.version,
    build: start.build,
    windows: start.windows ? `${start.windows.product ?? ""} ${start.windows.display ?? ""} (${start.windows.build}.${start.windows.ubr})` : undefined,
    args: start.args,
    length: dur(last - first),
    ended: end ? "clean exit" : panics.length ? "PANIC (crashed)" : "NO END LINE: killed, crashed or still running",
    pages,
    clicks: ev.filter((e) => e.k === "ui.click").length,
    actions: cmds.filter((c) => ACTIONS.has(c.cmd)).map((c) => `${local(c.t, tz).slice(11)} ${c.cmd} ${c.ok ? "ok" : "FAILED"} in ${dur(c.ms)} args=${short(c.args)} result=${short(c.ok ? c.result : c.error)}`),
    activity: ev.filter((e) => e.k === "activity").map((e) => `${e.entry?.kind} bytes=${e.entry?.bytes} items=${e.entry?.items} failed=${e.entry?.failed} undo=${e.entry?.undo}`),
    slow: cmds.filter((c) => c.ms >= SLOW_MS).map((c) => `${c.cmd} ${dur(c.ms)}`),
    longtasks: ev.filter((e) => e.k === "ui.longtask").length,
    errors,
    panics: panics.map((p) => `${p.msg} at ${p.at} (thread ${p.thread})`),
    truncated: ev.some((e) => e.k === "truncated"),
  };
});

if (opt("json")) {
  console.log(JSON.stringify(digests, null, 2));
  process.exit(0);
}

console.log(`Developer log: ${dir} (${fs.readdirSync(dir).filter((f) => f.endsWith(".jsonl")).length} sessions on disk)\n`);
for (const d of digests) {
  console.log(`## ${d.started} · ${d.version} ${d.build} · ${d.length} · ${d.ended}`);
  console.log(`   file ${d.file}${d.windows ? ` · ${d.windows}` : ""}${d.args?.length ? ` · args ${d.args.join(" ")}` : ""}`);
  if (d.pages.length) console.log(`   pages: ${d.pages.join(" → ")}`);
  console.log(`   clicks: ${d.clicks}${d.longtasks ? ` · UI freezes over 200 ms: ${d.longtasks}` : ""}${d.truncated ? " · FILE TRUNCATED" : ""}`);
  const list = (title, items) => {
    if (!items.length) return;
    console.log(`   ${title} (${items.length}):`);
    for (const i of items.slice(0, 40)) console.log(`     - ${i}`);
    if (items.length > 40) console.log(`     … ${items.length - 40} more`);
  };
  list("actions", d.actions);
  list("activity entries saved", d.activity);
  list("slow commands", d.slow);
  list("errors and warnings", d.errors);
  list("PANICS", d.panics);
  console.log("");
}
