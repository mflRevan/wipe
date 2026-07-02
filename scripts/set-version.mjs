#!/usr/bin/env node
// Set the release version everywhere it lives, from a single source of truth.
//
//   node scripts/set-version.mjs 0.1.2
//
// Updates the root Cargo.toml (workspace.package version + the internal
// wipe-core/wipe-daemon dependency versions), the standalone Tauri crate, and
// the frontend package.json files, then refreshes Cargo.lock. crates.io, the npm
// package (cargo-dist reads the crate version), and the binaries all derive their
// version from the root Cargo.toml, so this is the only place you set it.
//
// After running, commit and tag:
//   git commit -am "chore(release): v0.1.2" && git tag v0.1.2 && git push origin main v0.1.2

import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { execSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const version = process.argv[2];
if (!version || !/^\d+\.\d+\.\d+(-[0-9A-Za-z.]+)?$/.test(version)) {
  console.error("Usage: node scripts/set-version.mjs <x.y.z[-pre]>");
  process.exit(1);
}

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

function edit(rel, transform) {
  const path = join(root, rel);
  if (!existsSync(path)) return;
  const before = readFileSync(path, "utf8");
  const after = transform(before);
  if (after !== before) {
    writeFileSync(path, after);
    console.log("  updated", rel);
  }
}

// Root Cargo.toml: workspace version + the two internal path-dependency versions.
edit("Cargo.toml", (s) =>
  s
    .replace(/(\[workspace\.package\][\s\S]*?\nversion = ")[^"]*(")/, `$1${version}$2`)
    .replace(/(wipe-core = \{ path = "crates\/wipe-core", version = ")[^"]*(")/, `$1${version}$2`)
    .replace(/(wipe-daemon = \{ path = "crates\/wipe-daemon", version = ")[^"]*(")/, `$1${version}$2`)
);

// Standalone Tauri wrapper crate.
edit("apps/desktop/src-tauri/Cargo.toml", (s) =>
  s.replace(/(\nversion = ")[^"]*(")/, `$1${version}$2`)
);

// Frontend package.json files (private, but keep them in lockstep).
for (const pkg of ["apps/web/package.json", "apps/desktop/package.json"]) {
  edit(pkg, (s) => s.replace(/("version"\s*:\s*")[^"]*(")/, `$1${version}$2`));
}

// Refresh Cargo.lock for the workspace members.
try {
  execSync("cargo update --workspace", { cwd: root, stdio: "inherit" });
} catch {
  try {
    execSync("cargo build --workspace --quiet", { cwd: root, stdio: "inherit" });
  } catch {
    console.warn("  (couldn't refresh Cargo.lock automatically — run `cargo build`)");
  }
}

console.log(`\nVersion set to ${version}. Next:`);
console.log(`  git commit -am "chore(release): v${version}"`);
console.log(`  git tag v${version} && git push origin main v${version}`);
