// Performance budget for the frontend (run after `vite build`): what the window loads at start.
// Sums the gzip size of index.html's entry script and the modules it preloads.
// Budgets are in docs/PERFORMANCE.md; raise them only with a reason recorded there.
import fs from "node:fs";
import path from "node:path";
import zlib from "node:zlib";

const BUDGET = {
  startupJsGzipKb: 240,
  startupCssGzipKb: 12,
  largestPageChunkGzipKb: 12,
};

const dist = path.resolve("dist");
const html = fs.readFileSync(path.join(dist, "index.html"), "utf8");
const gz = (file) => zlib.gzipSync(fs.readFileSync(file)).length / 1024;

// What loads at start: the module entry script and every file it preloads (its static imports).
const entry = html.match(/<script type="module"[^>]+src="\/?([^"]+\.js)"/)?.[1];
if (!entry) throw new Error("no module entry script in dist/index.html");
const seen = new Set([entry, ...[...html.matchAll(/<link rel="modulepreload"[^>]+href="\/?([^"]+\.js)"/g)].map((m) => m[1])]);
const startupJs = [...seen].reduce((s, f) => s + gz(path.join(dist, f)), 0);
const css = [...html.matchAll(/<link[^>]+href="\/?([^"]+\.css)"/g)].map((m) => m[1]);
const startupCss = css.reduce((s, f) => s + gz(path.join(dist, f)), 0);
const assets = fs.readdirSync(path.join(dist, "assets"));
const pages = assets.filter((f) => /Page-.*\.js$/.test(f));
const largestPage = Math.max(0, ...pages.map((f) => gz(path.join(dist, "assets", f))));

const rows = [
  ["Startup JavaScript (gzip)", startupJs, BUDGET.startupJsGzipKb],
  ["Startup CSS (gzip)", startupCss, BUDGET.startupCssGzipKb],
  ["Largest page chunk (gzip)", largestPage, BUDGET.largestPageChunkGzipKb],
];
let ok = true;
for (const [name, value, budget] of rows) {
  const pass = value <= budget;
  ok &&= pass;
  console.log(`${pass ? "ok  " : "OVER"} ${name}: ${value.toFixed(1)} KB (budget ${budget} KB)`);
}
console.log(`     Startup files: ${[...seen].join(", ")}`);
if (!ok) {
  console.error("Performance budget exceeded (see docs/PERFORMANCE.md).");
  process.exit(1);
}
