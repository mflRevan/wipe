#!/usr/bin/env node
'use strict';

/**
 * postinstall script for the `wipe` npm wrapper package.
 *
 * Downloads the prebuilt `wipe` binary for the current OS/arch from the
 * matching GitHub Release and drops it into ./bin/, where bin/wipe.js
 * (the package's `bin` entry) execs it.
 *
 * IMPORTANT -- asset naming contract:
 *   The archive names built by `assetName()` below (`wipe-<target-triple>.<ext>`)
 *   must exactly match whatever cargo-dist actually produces for this repo
 *   (see ../dist-workspace.toml and ../.github/workflows/release.yml). If
 *   targets, the app/binary name, or the archive extensions configured in
 *   dist-workspace.toml (`unix-archive` / `windows-archive`) ever change,
 *   update TARGETS / the extensions below to match. After the first real
 *   tagged release, double check against the actual filenames attached to
 *   https://github.com/mflRevan/wipe/releases.
 *
 * Only Node built-ins are used: https, fs, path, os, child_process.
 * (zlib is not needed directly -- extraction is delegated to the `tar`
 * binary that ships with the OS, with a couple of documented fallbacks,
 * rather than reimplementing gzip/tar/zip parsing by hand.)
 */

const https = require('https');
const fs = require('fs');
const path = require('path');
const os = require('os');
const { execFileSync } = require('child_process');

const REPO = 'mflRevan/wipe';
const BIN_NAME = 'wipe';

const packageJson = require('./package.json');
const VERSION = packageJson.version;

const BIN_DIR = path.join(__dirname, 'bin');

/**
 * Resolve the current platform/arch to a cargo-dist target triple.
 * Returns null if there's no prebuilt binary published for this host.
 */
function resolveTarget() {
  const { platform, arch } = process;

  if (platform === 'win32' && arch === 'x64') {
    return { triple: 'x86_64-pc-windows-msvc', ext: 'zip', exeName: `${BIN_NAME}.exe` };
  }
  if (platform === 'darwin' && arch === 'x64') {
    return { triple: 'x86_64-apple-darwin', ext: 'tar.gz', exeName: BIN_NAME };
  }
  if (platform === 'darwin' && arch === 'arm64') {
    return { triple: 'aarch64-apple-darwin', ext: 'tar.gz', exeName: BIN_NAME };
  }
  if (platform === 'linux' && arch === 'x64') {
    const triple = isMuslLibc() ? 'x86_64-unknown-linux-musl' : 'x86_64-unknown-linux-gnu';
    return { triple, ext: 'tar.gz', exeName: BIN_NAME };
  }

  return null;
}

/**
 * Cheap, dependency-free musl detection. Alpine -- by far the most common
 * musl-based distro -- always ships /etc/alpine-release. This won't catch
 * every musl system in existence, but it covers the common case without
 * pulling in a package to inspect the ELF interpreter.
 */
function isMuslLibc() {
  try {
    return fs.existsSync('/etc/alpine-release');
  } catch {
    return false;
  }
}

function assetName(target) {
  return `${BIN_NAME}-${target.triple}.${target.ext}`;
}

function download(url, destPath, redirectsLeft) {
  if (redirectsLeft === undefined) redirectsLeft = 5;
  return new Promise((resolve, reject) => {
    const file = fs.createWriteStream(destPath);
    const request = https.get(url, { headers: { 'User-Agent': 'wipe-npm-installer' } }, (res) => {
      const { statusCode, headers } = res;

      if (statusCode >= 300 && statusCode < 400 && headers.location && redirectsLeft > 0) {
        res.resume();
        file.close();
        fs.unlink(destPath, () => {
          download(headers.location, destPath, redirectsLeft - 1).then(resolve, reject);
        });
        return;
      }

      if (statusCode !== 200) {
        res.resume();
        file.close();
        fs.unlink(destPath, () => {
          reject(new Error(`Download failed: HTTP ${statusCode} for ${url}`));
        });
        return;
      }

      res.pipe(file);
      file.on('finish', () => file.close(() => resolve()));
    });

    request.on('error', (err) => {
      file.close();
      fs.unlink(destPath, () => reject(err));
    });
  });
}

function extractTarGz(archivePath, destDir) {
  // GNU tar, BSD tar (macOS), and the bsdtar-based tar.exe that has shipped
  // with Windows since 10 (1803+) all understand `-xzf` for .tar.gz.
  execFileSync('tar', ['-xzf', archivePath, '-C', destDir], { stdio: 'inherit' });
}

function extractZip(archivePath, destDir) {
  if (process.platform === 'win32') {
    try {
      // Windows' built-in tar.exe (bsdtar) also handles .zip.
      execFileSync('tar', ['-xf', archivePath, '-C', destDir], { stdio: 'inherit' });
      return;
    } catch {
      // Fall back to PowerShell's Expand-Archive, present on every
      // supported Windows version, in case tar.exe is unavailable/blocked.
      execFileSync(
        'powershell.exe',
        [
          '-NoProfile',
          '-NonInteractive',
          '-Command',
          `Expand-Archive -LiteralPath '${archivePath}' -DestinationPath '${destDir}' -Force`,
        ],
        { stdio: 'inherit' }
      );
      return;
    }
  }

  // Non-Windows fallback (only reachable if a future target ships a .zip
  // asset on unix): prefer `unzip`, fall back to bsdtar-style `tar`.
  try {
    execFileSync('unzip', ['-o', archivePath, '-d', destDir], { stdio: 'inherit' });
  } catch {
    execFileSync('tar', ['-xf', archivePath, '-C', destDir], { stdio: 'inherit' });
  }
}

/** Archives may unpack into a subdirectory; walk the tree for the binary. */
function findExtractedBinary(rootDir, exeName) {
  const stack = [rootDir];
  while (stack.length) {
    const dir = stack.pop();
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        stack.push(full);
      } else if (entry.name === exeName) {
        return full;
      }
    }
  }
  return null;
}

function unsupportedPlatformMessage() {
  return (
    `[wipe] No prebuilt binary is published for ${process.platform}/${process.arch}.\n` +
    '[wipe] Supported platforms: linux-x64 (glibc or musl), macOS x64/arm64, Windows x64.\n' +
    '[wipe] Install from source instead, e.g.:\n' +
    '[wipe]   cargo install --git https://github.com/mflRevan/wipe wipe-cli\n' +
    '[wipe] (or `cargo install wipe-cli` once/if published to crates.io)'
  );
}

async function main() {
  const target = resolveTarget();
  if (!target) {
    console.error(unsupportedPlatformMessage());
    process.exit(1);
    return;
  }

  fs.mkdirSync(BIN_DIR, { recursive: true });

  const asset = assetName(target);
  const url = `https://github.com/${REPO}/releases/download/v${VERSION}/${asset}`;
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'wipe-install-'));
  const archivePath = path.join(tmpDir, asset);

  try {
    console.log(`[wipe] Downloading ${url}`);
    await download(url, archivePath);

    if (target.ext === 'zip') {
      extractZip(archivePath, tmpDir);
    } else {
      extractTarGz(archivePath, tmpDir);
    }

    const extracted = findExtractedBinary(tmpDir, target.exeName);
    if (!extracted) {
      throw new Error(`Could not find "${target.exeName}" inside ${asset}`);
    }

    const finalPath = path.join(BIN_DIR, target.exeName);
    fs.copyFileSync(extracted, finalPath);
    if (process.platform !== 'win32') {
      fs.chmodSync(finalPath, 0o755);
    }

    console.log(`[wipe] Installed ${target.exeName} (${target.triple}) -> ${finalPath}`);
  } finally {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  }
}

main().catch((err) => {
  console.error(`[wipe] postinstall failed: ${err && err.message ? err.message : err}`);
  console.error(
    '[wipe] You can still use wipe by installing it another way:\n' +
      '[wipe]   cargo install wipe-cli\n' +
      `[wipe] or download a binary manually from https://github.com/${REPO}/releases`
  );
  process.exit(1);
});
