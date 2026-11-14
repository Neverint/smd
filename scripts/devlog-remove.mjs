// DEVLOG: removes the developer data log from the code base (before 1.0).
// Usage: npm run devlog:remove [-- --dry-run]
//
// 1. Deletes the devlog files.
// 2. Deletes every line marked `// DEVLOG` or `<!-- DEVLOG -->`, and every block between
//    `// DEVLOG start` and `// DEVLOG end` (or the same in <!-- -->), in src, src-tauri, crates,
//    scripts, e2e, docs and the root Markdown files.
// 3. Removes the `devlog` and `devlog:remove` npm scripts.
// 4. Runs `cargo fmt --all` and lists any "devlog" text left, to check by hand.
// It does not touch the logs already written on a PC (%LOCALAPPDATA%\io.github.neverint.smd\devlog).

import { execSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const root = path.resolve(import.meta.dirname, "..");
const dry = process.argv.includes("--dry-run");
const say = (s) => console.log((dry ? "[dry run] " : "") + s);

const FILES = ["src-tauri/src/devlog.rs", "src/dev/devlog.ts", "src/dev/devlog.test.ts","scripts/devlog.mjs", "docs/DEVLOG.md", "scripts/devlog-remove.mjs"];
const DIRS = ["src", "src-tauri/src", "src-tauri/build.rs", "crates", "scripts", "e2e", "docs", "CLAUDE.md", "README.md", "README.fr.md", "CHANGELOG.md", "vite.config.ts"];
const EXT = /\.(rs|ts|tsx|mjs|js|md|css)$/;

const LINE = /(\/\/|<!--)\s*DEVLOG(\s*-->)?\s*$/;
const START = /(\/\/|<!--)\s*DEVLOG start\b/;
const END = /(\/\/|<!--)\s*DEVLOG end\b/;

function walk(p, out) {
  const full = path.join(root, p);
  if (!fs.existsSync(full)) return;
  const st = fs.statSync(full);
  if (st.isFile()) {
    if (EXT.test(p)) out.push(p);
    return;
  }
  for (const name of fs.readdirSync(full)) {
    if (name === "node_modules" || name === "target" || name === "gen") continue;
    walk(path.join(p, name), out);
  }
}

const files = [];
for (const d of DIRS) walk(d, files);
const deleting = new Set(FILES.map((f) => path.normalize(f)));

for (const f of files) {
  if (deleting.has(path.normalize(f))) continue;
  const full = path.join(root, f);
  const text = fs.readFileSync(full, "utf8");
  const lines = text.split("\n");
  const kept = [];
  let inBlock = false;
  let removed = 0;
  for (const l of lines) {
    if (START.test(l)) {
      inBlock = true;
      removed++;
      continue;
    }
    if (inBlock) {
      removed++;
      if (END.test(l)) inBlock = false;
      continue;
    }
    if (LINE.test(l)) {
      removed++;
      continue;
    }
    kept.push(l);
  }
  if (inBlock) throw new Error(`${f}: "DEVLOG start" without "DEVLOG end"`);
  if (removed) {
    say(`${f}: ${removed} line(s) removed`);
    if (!dry) fs.writeFileSync(full, kept.join("\n"));
  }
}

const pkgPath = path.join(root, "package.json");
const pkg = JSON.parse(fs.readFileSync(pkgPath, "utf8"));
for (const s of ["devlog", "devlog:remove"]) {
  if (pkg.scripts?.[s]) {
    say(`package.json: script "${s}" removed`);
    delete pkg.scripts[s];
  }
}
if (!dry) fs.writeFileSync(pkgPath, `${JSON.stringify(pkg, null, 2)}\n`);

for (const f of FILES) {
  if (fs.existsSync(path.join(root, f))) {
    say(`deleted ${f}`);
    if (!dry) fs.rmSync(path.join(root, f));
  }
}

if (!dry) {
  try {
    execSync("cargo fmt --all", { cwd: root, stdio: "inherit" });
  } catch {
    console.log("cargo fmt did not run: run it by hand.");
  }
}

const left = [];
const after = [];
for (const d of DIRS) walk(d, after);
for (const f of after) {
  if (dry && deleting.has(path.normalize(f))) continue;
  const full = path.join(root, f);
  if (!fs.existsSync(full)) continue;
  fs.readFileSync(full, "utf8")
    .split("\n")
    .forEach((l, i) => {
      if (/devlog/i.test(l) && !(dry && LINE.test(l))) left.push(`${f}:${i + 1}: ${l.trim()}`);
    });
}
if (left.length) {
  console.log(`\n"devlog" still appears here (check by hand):`);
  for (const l of left) console.log(`  ${l}`);
}
console.log(`\nNext: npm run lint, npm run typecheck, npm test, npm run check:rust.`);
