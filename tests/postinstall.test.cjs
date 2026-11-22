'use strict';

const assert = require('node:assert/strict');
const { mkdirSync, mkdtempSync, rmSync, writeFileSync } = require('node:fs');
const { tmpdir } = require('node:os');
const { join } = require('node:path');
const test = require('node:test');

const {
  TARGETS,
  artifactName,
  cargoTarget,
  findExtractedBinary,
  platformKey,
  releaseBaseUrl,
  sha256,
  verifyChecksum,
} = require('../npm/postinstall.cjs');
const { expectedReleaseUrls, verifyReleaseAssets } = require('../npm/prepublish-check.cjs');

test('maps all 6 supported platforms to Rust targets', () => {
  assert.equal(Object.keys(TARGETS).length, 6);
  assert.equal(platformKey('darwin', 'arm64'), 'darwin-arm64');
  assert.equal(platformKey('darwin', 'x64'), 'darwin-x64');
  assert.equal(platformKey('linux', 'x64'), 'linux-x64');
  assert.equal(platformKey('linux', 'arm64'), 'linux-arm64');
  assert.equal(platformKey('win32', 'x64'), 'win32-x64');
  assert.equal(platformKey('win32', 'arm64'), 'win32-arm64');

  assert.equal(cargoTarget('darwin', 'arm64'), 'aarch64-apple-darwin');
  assert.equal(cargoTarget('darwin', 'x64'), 'x86_64-apple-darwin');
  assert.equal(cargoTarget('linux', 'x64'), 'x86_64-unknown-linux-gnu');
  assert.equal(cargoTarget('linux', 'arm64'), 'aarch64-unknown-linux-gnu');
  assert.equal(cargoTarget('win32', 'x64'), 'x86_64-pc-windows-msvc');
  assert.equal(cargoTarget('win32', 'arm64'), 'aarch64-pc-windows-msvc');
});

test('rejects unsupported platforms with actionable error', () => {
  assert.throws(
    () => cargoTarget('linux', 'arm'),
    /Unsupported platform: linux\/arm.*Supported platforms:.*darwin-arm64.*win32-arm64/,
  );
  assert.throws(() => cargoTarget('freebsd', 'x64'), /Unsupported platform: freebsd\/x64/);
});

test('formats artifact names for all 6 targets and release base URLs', () => {
  assert.equal(artifactName('aarch64-apple-darwin'), 'say-aarch64-apple-darwin.tar.xz');
  assert.equal(artifactName('x86_64-apple-darwin'), 'say-x86_64-apple-darwin.tar.xz');
  assert.equal(artifactName('x86_64-unknown-linux-gnu'), 'say-x86_64-unknown-linux-gnu.tar.xz');
  assert.equal(artifactName('aarch64-unknown-linux-gnu'), 'say-aarch64-unknown-linux-gnu.tar.xz');
  assert.equal(artifactName('x86_64-pc-windows-msvc'), 'say-x86_64-pc-windows-msvc.zip');
  assert.equal(artifactName('aarch64-pc-windows-msvc'), 'say-aarch64-pc-windows-msvc.zip');
  assert.equal(releaseBaseUrl('0.1.0'), 'https://github.com/doggy8088/say/releases/download/v0.1.0');
  assert.equal(releaseBaseUrl('1.2.3'), 'https://github.com/doggy8088/say/releases/download/v1.2.3');
});

test('verifies sha256 checksums for valid, mismatched, and malformed inputs', () => {
  const dir = mkdtempSync(join(tmpdir(), 'say-checksum-'));
  try {
    const file = join(dir, 'sample.txt');
    writeFileSync(file, 'hello');
    const digest = sha256(file);

    verifyChecksum(file, `${digest}  sample.txt\n`);
    verifyChecksum(file, digest.toUpperCase());

    assert.throws(() => verifyChecksum(file, '0'.repeat(64)), /Checksum mismatch/);
    assert.throws(() => verifyChecksum(file, ''), /Invalid checksum file format/);
    assert.throws(() => verifyChecksum(file, 'not-a-valid-sha256'), /Invalid checksum file format/);
    assert.throws(() => verifyChecksum(file, `${'a'.repeat(63)}`), /Invalid checksum file format/);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('finds extracted binary at root, nested directory, or throws when missing', () => {
  const dir = mkdtempSync(join(tmpdir(), 'say-extract-'));
  try {
    const rootDir = join(dir, 'root');
    mkdirSync(rootDir, { recursive: true });
    const directBin = join(rootDir, 'say');
    writeFileSync(directBin, 'synthetic-binary');
    assert.equal(findExtractedBinary(rootDir, 'say'), directBin);

    const nestedDir = join(dir, 'nested');
    const subDir = join(nestedDir, 'say-x86_64-pc-windows-msvc');
    mkdirSync(subDir, { recursive: true });
    const nestedBin = join(subDir, 'say.exe');
    writeFileSync(nestedBin, 'synthetic-binary');
    assert.equal(findExtractedBinary(nestedDir, 'say.exe'), nestedBin);

    const emptyDir = join(dir, 'empty');
    mkdirSync(emptyDir, { recursive: true });
    assert.throws(() => findExtractedBinary(emptyDir, 'say'), /Archive did not contain say/);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('prepublish check builds and validates all 12 release asset URLs', async () => {
  const urls = expectedReleaseUrls('0.1.0');
  assert.equal(urls.length, 12);
  assert.ok(urls.includes('https://github.com/doggy8088/say/releases/download/v0.1.0/say-aarch64-apple-darwin.tar.xz'));
  assert.ok(urls.includes('https://github.com/doggy8088/say/releases/download/v0.1.0/say-aarch64-apple-darwin.tar.xz.sha256'));
  assert.ok(urls.includes('https://github.com/doggy8088/say/releases/download/v0.1.0/say-aarch64-unknown-linux-gnu.tar.xz'));
  assert.ok(urls.includes('https://github.com/doggy8088/say/releases/download/v0.1.0/say-aarch64-unknown-linux-gnu.tar.xz.sha256'));
  assert.ok(urls.includes('https://github.com/doggy8088/say/releases/download/v0.1.0/say-aarch64-pc-windows-msvc.zip'));
  assert.ok(urls.includes('https://github.com/doggy8088/say/releases/download/v0.1.0/say-aarch64-pc-windows-msvc.zip.sha256'));

  const okUrls = await verifyReleaseAssets({
    version: '0.1.0',
    retries: 1,
    retryDelayMs: 0,
    check: async () => ({ ok: true, statusCode: 200 }),
  });
  assert.equal(okUrls.length, 12);

  await assert.rejects(
    () =>
      verifyReleaseAssets({
        version: '0.1.0',
        retries: 2,
        retryDelayMs: 0,
        check: async () => ({ ok: false, statusCode: 404 }),
      }),
    /Missing or unavailable release assets for v0\.1\.0/,
  );
});
