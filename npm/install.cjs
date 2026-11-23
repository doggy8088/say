#!/usr/bin/env node
'use strict';

// Finds or downloads the native `say` binary for the npm package.
//
// The package has no install script: newer npm versions block install scripts unless the
// user allows them. Instead, the launcher (npm/cli.cjs) calls ensureBinary() on its first
// run, which downloads the release archive for this platform from the website, verifies
// its SHA-256 and unpacks it into the package folder, or into a per-user cache when the
// package folder isn't writable (for example after `sudo npm install -g`).
//
// `node npm/install.cjs` installs the binary right away, preferring a local
// `cargo build --release` when one exists (for development).

const { createHash } = require('node:crypto');
const { spawnSync } = require('node:child_process');
const {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
  writeFileSync,
} = require('node:fs');
const { get } = require('node:https');
const { homedir } = require('node:os');
const { join } = require('node:path');
const { URL } = require('node:url');

const PACKAGE_ROOT = join(__dirname, '..');
const BINARY_NAME = 'say';
const GITHUB_OWNER = 'doggy8088';
const GITHUB_REPO = 'say';
const BIN_DIR = join(__dirname, `${BINARY_NAME}-bin`);
const BIN_NAME = process.platform === 'win32' ? `${BINARY_NAME}.exe` : BINARY_NAME;
const DEST = join(BIN_DIR, BIN_NAME);

const TARGETS = {
  'darwin-arm64': 'aarch64-apple-darwin',
  'darwin-x64': 'x86_64-apple-darwin',
  'linux-x64': 'x86_64-unknown-linux-gnu',
  'linux-arm64': 'aarch64-unknown-linux-gnu',
  'win32-x64': 'x86_64-pc-windows-msvc',
  'win32-arm64': 'aarch64-pc-windows-msvc',
};

function platformKey(platform = process.platform, arch = process.arch) {
  return `${platform}-${arch}`;
}

function cargoTarget(platform = process.platform, arch = process.arch) {
  const target = TARGETS[platformKey(platform, arch)];
  if (!target) {
    const supported = Object.keys(TARGETS).join(', ');
    throw new Error(
      `Unsupported platform: ${platform}/${arch}. Supported platforms: ${supported}. ` +
        `You can build "${BINARY_NAME}" from source with Cargo (https://github.com/${GITHUB_OWNER}/${GITHUB_REPO}).`,
    );
  }
  return target;
}

function packageVersion() {
  return require(join(PACKAGE_ROOT, 'package.json')).version;
}

function artifactName(target) {
  const ext = target.includes('windows') || target.includes('pc-windows') ? 'zip' : 'tar.xz';
  return `${BINARY_NAME}-${target}.${ext}`;
}

// The source repository is private, so its GitHub Releases can't be downloaded anonymously.
// The website mirrors every release under the same path layout (see .github/workflows/pages.yml).
const DOWNLOAD_BASE = 'https://say.gh.miniasp.com/releases/download';

function releaseBaseUrl(version = packageVersion()) {
  return `${DOWNLOAD_BASE}/v${version}`;
}

function sha256(path) {
  return createHash('sha256').update(readFileSync(path)).digest('hex');
}

function verifyChecksum(filePath, checksumText) {
  if (typeof checksumText !== 'string') {
    throw new Error('Invalid checksum file format');
  }
  const expected = checksumText.trim().split(/\s+/)[0].toLowerCase();
  if (!/^[a-f0-9]{64}$/.test(expected)) {
    throw new Error('Invalid checksum file format');
  }
  const actual = sha256(filePath);
  if (actual !== expected) {
    throw new Error(`Checksum mismatch for ${filePath}: expected ${expected}, got ${actual}`);
  }
}

function download(url, destination, redirectsRemaining = 5) {
  return new Promise((resolve, reject) => {
    get(url, { headers: { 'User-Agent': '@willh/say' } }, (res) => {
      if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location && redirectsRemaining > 0) {
        res.resume();
        const nextUrl = new URL(res.headers.location, url).toString();
        download(nextUrl, destination, redirectsRemaining - 1).then(resolve, reject);
        return;
      }
      if (res.statusCode !== 200) {
        res.resume();
        reject(new Error(`Download failed ${res.statusCode}: ${url}`));
        return;
      }
      const chunks = [];
      res.on('data', (chunk) => chunks.push(chunk));
      res.on('end', () => {
        writeFileSync(destination, Buffer.concat(chunks));
        resolve();
      });
    }).on('error', reject);
  });
}

function run(command, args) {
  const result = spawnSync(command, args, { stdio: 'inherit' });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`Command failed: ${command}`);
}

function psQuote(value) {
  return `'${String(value).replace(/'/g, "''")}'`;
}

function extract(archive, destDir) {
  mkdirSync(destDir, { recursive: true });
  if (archive.endsWith('.zip')) {
    if (process.platform === 'win32') {
      // Unzip through .NET, not Expand-Archive: Windows PowerShell started from PowerShell 7
      // inherits PowerShell 7's module path, loads an incompatible Expand-Archive that fails,
      // and still exits 0.
      run('powershell', [
        '-NoProfile',
        '-NonInteractive',
        '-Command',
        "$ErrorActionPreference = 'Stop'; Add-Type -AssemblyName System.IO.Compression.FileSystem; " +
          `[IO.Compression.ZipFile]::ExtractToDirectory(${psQuote(archive)}, ${psQuote(destDir)})`,
      ]);
    } else {
      run('unzip', ['-o', archive, '-d', destDir]);
    }
  } else {
    run('tar', ['-xJf', archive, '-C', destDir]);
  }
}

function findExtractedBinary(dir, binName = BIN_NAME) {
  const direct = join(dir, binName);
  if (existsSync(direct)) return direct;

  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    if (!entry.isDirectory()) continue;
    const candidate = join(dir, entry.name, binName);
    if (existsSync(candidate)) return candidate;
  }

  throw new Error(`Archive did not contain ${binName}`);
}

// Per-user cache for installs whose package folder isn't writable.
function cacheRoot(platform = process.platform, env = process.env, home = homedir()) {
  if (platform === 'win32') return join(env.LOCALAPPDATA || join(home, 'AppData', 'Local'), 'willh-say');
  if (platform === 'darwin') return join(home, 'Library', 'Caches', 'willh-say');
  return join(env.XDG_CACHE_HOME || join(home, '.cache'), 'willh-say');
}

// Where the binary may live, in the order the launcher checks.
function binaryCandidates(version = packageVersion()) {
  return [DEST, join(cacheRoot(), version, BIN_NAME)];
}

function findBinary(version = packageVersion()) {
  return binaryCandidates(version).find((path) => existsSync(path)) || null;
}

function installFromLocalBuild() {
  const localRelease = join(PACKAGE_ROOT, 'target', 'release', BIN_NAME);
  if (!existsSync(localRelease)) return false;
  mkdirSync(BIN_DIR, { recursive: true });
  copyFileSync(localRelease, DEST);
  chmodSync(DEST, 0o755);
  return true;
}

// Downloads, verifies and unpacks the release binary into destDir. The work happens in a
// private temporary folder and the binary is renamed into place, so two first runs at the
// same time don't trip over each other.
async function installFromRelease(destDir = BIN_DIR, version = packageVersion()) {
  const target = cargoTarget();
  const archive = artifactName(target);
  const base = releaseBaseUrl(version);

  mkdirSync(destDir, { recursive: true });
  const tmpDir = mkdtempSync(join(destDir, '.tmp-'));
  const archivePath = join(tmpDir, archive);
  const checksumPath = `${archivePath}.sha256`;
  try {
    await download(`${base}/${archive}`, archivePath);
    await download(`${base}/${archive}.sha256`, checksumPath);
    verifyChecksum(archivePath, readFileSync(checksumPath, 'utf8'));
    const unpacked = join(tmpDir, 'unpacked');
    extract(archivePath, unpacked);

    const extracted = findExtractedBinary(unpacked);
    chmodSync(extracted, 0o755);
    const dest = join(destDir, BIN_NAME);
    try {
      renameSync(extracted, dest);
    } catch (error) {
      // Another first run finished first; its binary is just as good.
      if (!existsSync(dest)) throw error;
    }
    return dest;
  } finally {
    rmSync(tmpDir, { recursive: true, force: true });
  }
}

const NOT_WRITABLE = new Set(['EACCES', 'EPERM', 'EROFS']);

// Returns the path of a usable binary, downloading it first if none is installed yet.
async function ensureBinary(version = packageVersion()) {
  const found = findBinary(version);
  if (found) return found;
  try {
    return await installFromRelease(BIN_DIR, version);
  } catch (error) {
    if (!NOT_WRITABLE.has(error.code)) throw error;
    return installFromRelease(join(cacheRoot(), version), version);
  }
}

async function main() {
  if (installFromLocalBuild()) return;
  await installFromRelease();
}

if (require.main === module) {
  main().catch((error) => {
    console.error(error.message);
    process.exit(1);
  });
}

module.exports = {
  TARGETS,
  artifactName,
  binaryCandidates,
  cacheRoot,
  cargoTarget,
  ensureBinary,
  findBinary,
  findExtractedBinary,
  platformKey,
  releaseBaseUrl,
  sha256,
  verifyChecksum,
};
