#!/usr/bin/env node
'use strict';

const { spawnSync } = require('node:child_process');
const { existsSync } = require('node:fs');
const { join } = require('node:path');

const BINARY_NAME = 'say';
const exe = process.platform === 'win32' ? `${BINARY_NAME}.exe` : BINARY_NAME;
const bin = join(__dirname, `${BINARY_NAME}-bin`, exe);

if (!existsSync(bin)) {
  console.error(
    `${BINARY_NAME} native binary was not found at ${bin}. Try reinstalling @willh/say or running "npm rebuild @willh/say".`,
  );
  process.exit(1);
}

const result = spawnSync(bin, process.argv.slice(2), { stdio: 'inherit' });
if (result.error) {
  console.error(result.error.message);
  process.exit(1);
}
process.exit(result.status ?? 1);
