#!/usr/bin/env node
'use strict';

/**
 * Launcher shim for the `wipe` npm wrapper package.
 *
 * The real platform binary is downloaded into this same `bin/` directory
 * by `../install.js` (run as the package's `postinstall` script -- see
 * package.json). This shim just execs it, forwarding argv, stdio, and the
 * process exit code.
 */

const fs = require('fs');
const path = require('path');
const { spawnSync } = require('child_process');

const BIN_NAME = process.platform === 'win32' ? 'wipe.exe' : 'wipe';
const binPath = path.join(__dirname, BIN_NAME);

if (!fs.existsSync(binPath)) {
  console.error(
    `[wipe] The "${BIN_NAME}" binary was not found in ${__dirname}.\n` +
      '[wipe] This usually means the postinstall download step failed or was\n' +
      '[wipe] skipped (e.g. installed with --ignore-scripts). Try:\n' +
      '[wipe]   npm rebuild wipe\n' +
      '[wipe] or install wipe another way: https://github.com/mflRevan/wipe#installation'
  );
  process.exit(1);
}

const result = spawnSync(binPath, process.argv.slice(2), { stdio: 'inherit' });

if (result.error) {
  console.error(`[wipe] Failed to launch "${binPath}": ${result.error.message}`);
  process.exit(1);
}

if (result.signal) {
  // Mirror the convention of dying from the same signal that killed the child.
  process.kill(process.pid, result.signal);
}

process.exit(result.status === null ? 1 : result.status);
