#!/usr/bin/env node
// Make cargo-dist's generated npm installer work on Windows machines where GNU
// tar (e.g. from Git for Windows / msys) is on PATH ahead of the built-in
// bsdtar.
//
// cargo-dist's `binary-install.js` extracts the release tarball with:
//   spawnSync("tar", ["xf", tempFile, "--strip-components", "1",
//                     "-C", this.installDirectory])
// where both paths are absolute "C:\...". GNU tar reads a "C:\..." argument as a
// remote host `C:` and dies:
//   "tar: Cannot connect to C: resolve failed"
// so `npm i -g @mflrevan/wipe` fails to extract and leaves the old version.
// Windows' built-in bsdtar (%SystemRoot%\System32\tar.exe) handles drive-letter
// paths natively (verified), so we pin tar to it on Windows and leave "tar"
// everywhere else. No-op on Linux/macOS.
//
// Usage: node scripts/patch-npm-installer.mjs <path-to/binary-install.js>
import { readFileSync, writeFileSync } from "node:fs";

const file = process.argv[2];
if (!file) {
  console.error("usage: patch-npm-installer.mjs <binary-install.js>");
  process.exit(2);
}

let s = readFileSync(file, "utf8");

// Match the tar-branch spawnSync and capture its argument tail (comments + the
// remaining args) so we only swap the executable, not the arguments.
const re = /spawnSync\(\s*"tar",\s*\[\s*"xf",\s*tempFile,([\s\S]*?)\]\s*\)/;
if (!re.test(s)) {
  console.error(
    `no matching tar spawnSync found in ${file}\n` +
      "the cargo-dist installer template may have changed - update this patch."
  );
  process.exit(1);
}

// Emitted into binary-install.js; `join` is already in scope there (it builds
// tempFile). "C:\\\\Windows" here becomes the source text "C:\\Windows".
const tarBin =
  'process.platform === "win32" ' +
  '? join(process.env.SystemRoot || "C:\\\\Windows", "System32", "tar.exe") ' +
  ': "tar"';

s = s.replace(re, (_m, rest) => `spawnSync(${tarBin}, ["xf", tempFile,${rest}])`);
writeFileSync(file, s);
console.log(`patched ${file} -> use Windows bsdtar for drive-letter-safe extraction`);
