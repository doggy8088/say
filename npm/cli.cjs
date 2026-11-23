#!/usr/bin/env node
'use strict';

const { spawnSync } = require('node:child_process');
const { ensureBinary, findBinary } = require('./install.cjs');

async function main() {
  let bin = findBinary();
  if (!bin) {
    // The package installs without an install script (newer npm versions block them), so
    // the first run fetches the native binary for this computer.
    process.stderr.write('say: downloading the native binary for this computer (first run only)...\n');
    try {
      bin = await ensureBinary();
    } catch (error) {
      console.error(`say: could not download the native binary: ${error.message}`);
      process.exit(1);
    }
  }

  const result = spawnSync(bin, process.argv.slice(2), { stdio: 'inherit' });
  if (result.error) {
    console.error(result.error.message);
    process.exit(1);
  }
  process.exit(result.status ?? 1);
}

main();
