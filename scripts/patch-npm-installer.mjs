#!/usr/bin/env node
// Patch cargo-dist's generated npm installer package for Windows. Three fixes:
//
// 1. tar drive letters. cargo-dist's `binary-install.js` extracts the release
//    tarball with `spawnSync("tar", ["xf", tempFile, ..., "-C", installDir])`
//    where both paths are absolute "C:\...". GNU tar (e.g. Git for Windows' msys
//    tar, often first on PATH) reads "C:\..." as a remote host `C:` and dies
//    ("tar: Cannot connect to C: resolve failed"), so `npm i -g` fails. Windows'
//    built-in bsdtar (%SystemRoot%\System32\tar.exe) handles drive letters, so we
//    pin tar to it on Windows and leave "tar" everywhere else.
//
// 2. Shim marker. The node launcher (`run()`) sets WIPE_NPM_SHIM=1 for the
//    binary, so `wipe doctor` can warn when commands arrive through the shim.
//
// 3. A real wipe.exe on PATH. npm exposes `wipe` on Windows as `wipe.cmd`, which
//    runs through cmd.exe - and cmd.exe silently cuts every argument at the first
//    line break, truncating multi-line `--body` text with exit code 0. After a
//    global install we also place the native binary next to the shims as
//    `wipe.exe`; PATHEXT resolves .EXE before .CMD, so `wipe` from cmd,
//    PowerShell, or Python's subprocess runs the binary directly, no cmd.exe.
//
// Usage: node scripts/patch-npm-installer.mjs <path-to/binary-install.js>
// (install.js is patched next to it).
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";

const file = process.argv[2];
if (!file) {
  console.error("usage: patch-npm-installer.mjs <binary-install.js>");
  process.exit(2);
}

function fail(what, where) {
  console.error(
    `${what} not found in ${where}\n` +
      "the cargo-dist installer template may have changed - update this patch."
  );
  process.exit(1);
}

// --- 1 + 2: binary-install.js ------------------------------------------------
let s = readFileSync(file, "utf8");

// Match the tar-branch spawnSync and capture its argument tail (comments + the
// remaining args) so we only swap the executable, not the arguments.
const tarRe = /spawnSync\(\s*"tar",\s*\[\s*"xf",\s*tempFile,([\s\S]*?)\]\s*\)/;
if (!tarRe.test(s)) fail("the tar spawnSync", file);
// Emitted into binary-install.js; `join` is already in scope there (it builds
// tempFile). "C:\\\\Windows" here becomes the source text "C:\\Windows".
const tarBin =
  'process.platform === "win32" ' +
  '? join(process.env.SystemRoot || "C:\\\\Windows", "System32", "tar.exe") ' +
  ': "tar"';
s = s.replace(tarRe, (_m, rest) => `spawnSync(${tarBin}, ["xf", tempFile,${rest}])`);

const optsRe = /const options = \{ cwd: process\.cwd\(\), stdio: "inherit" \};/;
if (!optsRe.test(s)) fail("the run() spawn options", file);
s = s.replace(
  optsRe,
  'const options = { cwd: process.cwd(), stdio: "inherit", env: { ...process.env, WIPE_NPM_SHIM: "1" } };'
);
writeFileSync(file, s);
console.log(`patched ${file} -> Windows bsdtar extraction + WIPE_NPM_SHIM marker`);

// --- 3: install.js -----------------------------------------------------------
const installJs = join(dirname(file), "install.js");
let inst = readFileSync(installJs, "utf8");
if (!/install\(false\);/.test(inst)) fail("install(false)", installJs);
inst = inst.replace(
  /install\(false\);/,
  `Promise.resolve(install(false)).then(linkWindowsExe);

// Place the native binary beside npm's shims as wipe.exe (global installs on
// Windows only): PATHEXT prefers .EXE over .CMD, so \`wipe\` no longer runs through
// cmd.exe, which cuts multi-line arguments at the first line break. Best-effort:
// any failure leaves the regular wipe.cmd shim working.
function linkWindowsExe() {
  if (process.platform !== "win32" || process.env.npm_config_global !== "true") return;
  const prefix = process.env.npm_config_prefix;
  if (!prefix) return;
  const fs = require("fs");
  const path = require("path");
  const src = path.join(__dirname, "node_modules", ".bin_real", "wipe.exe");
  const dest = path.join(prefix, "wipe.exe");
  if (!fs.existsSync(src)) return;
  try {
    if (fs.existsSync(dest)) {
      // A running wipe.exe (e.g. \`wipe serve\`) can't be overwritten but can be
      // renamed; move it aside so the new version always takes over.
      const old = dest + ".old-" + Date.now();
      fs.renameSync(dest, old);
      try { fs.unlinkSync(old); } catch (_) { /* still running; harmless */ }
    }
    fs.copyFileSync(src, dest);
  } catch (e) {
    console.error("wipe: could not place wipe.exe next to the npm shims (" + e.message + ");");
    console.error("wipe: multi-line arguments via wipe.cmd may be truncated - use --body-file.");
  }
}`
);
writeFileSync(installJs, inst);
console.log(`patched ${installJs} -> native wipe.exe beside the npm shims on Windows`);
