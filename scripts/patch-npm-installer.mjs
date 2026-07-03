#!/usr/bin/env node
// Make cargo-dist's generated npm installer safe on Windows machines where GNU
// tar (e.g. from Git for Windows) sits ahead of the built-in bsdtar on PATH.
//
// cargo-dist's `binary-install.js` extracts the release tarball with:
//   spawnSync("tar", ["xf", <ABSOLUTE tempFile>, "--strip-components", "1",
//                     "-C", this.installDirectory])
// GNU tar reads the "C:\..." archive path as a remote host `C:` and dies with
//   "tar: Cannot connect to C: resolve failed"
// so `npm i -g @mflrevan/wipe` fails to extract and leaves the old version.
//
// Fix: run tar with cwd = the download dir and pass a RELATIVE archive name, so
// there is no drive-letter colon in the archive argument. This works for both
// GNU tar and bsdtar, and is a no-op semantically on Linux/macOS.
//
// Usage: node scripts/patch-npm-installer.mjs <path-to/binary-install.js>
import { readFileSync, writeFileSync } from "node:fs";

const file = process.argv[2];
if (!file) {
  console.error("usage: patch-npm-installer.mjs <binary-install.js>");
  process.exit(2);
}

let s = readFileSync(file, "utf8");
const re = /spawnSync\("tar",\s*\[\s*"xf",\s*tempFile,([\s\S]*?)\]\s*\);/;
if (!re.test(s)) {
  console.error(
    `no matching tar spawnSync found in ${file}\n` +
      "the cargo-dist installer template may have changed - update this patch."
  );
  process.exit(1);
}
s = s.replace(
  re,
  (_m, rest) => `spawnSync("tar", ["xf", this.filename,${rest}], { cwd: directory });`
);
writeFileSync(file, s);
console.log(`patched ${file} -> relative archive + cwd (Windows GNU tar fix)`);
