#!/usr/bin/env node
// Cache busting for the website, run by .github/workflows/pages.yml before upload.
//
// GitHub Pages serves every file with `Cache-Control: max-age=600` and ignores query
// strings. Without this step a browser can pair a freshly deployed index.html with the
// style.css or app.js it cached from the previous deploy, and the page renders broken.
//
// This script appends `?v=<content hash>` to every local `assets/...` URL in the site's
// HTML, web manifest and JavaScript. A file that changes gets a new URL; files that
// don't change keep their URL and stay cached. JavaScript URLs built from a template,
// such as `assets/audio/${clip}.mp3`, get one hash over every file the template matches.
// A reference to a file that doesn't exist fails the deploy.
//
// Usage: node scripts/stamp-assets.mjs [site-dir]   (default: site; edits files in place)

import { createHash } from 'node:crypto';
import { existsSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join, posix } from 'node:path';

const root = process.argv[2] || 'site';
const errors = [];
let stamped = 0;

function digest(paths) {
  const hash = createHash('sha256');
  for (const path of paths) {
    hash.update(path);
    hash.update(readFileSync(join(root, path)));
  }
  return hash.digest('hex').slice(0, 10);
}

// Every file a site-relative path (or a `${...}` template of one) refers to.
function filesFor(path) {
  if (!path.includes('${')) {
    return existsSync(join(root, path)) ? [path] : [];
  }
  const dir = posix.dirname(path);
  const escaped = posix.basename(path).replace(/[.*+?^()|[\]\\]/g, '\\$&');
  const pattern = new RegExp(`^${escaped.replace(/\$\{[^}]*\}/g, '[^/]+')}$`);
  if (!existsSync(join(root, dir))) return [];
  return readdirSync(join(root, dir))
    .filter((name) => pattern.test(name))
    .sort()
    .map((name) => `${dir}/${name}`);
}

function stamp(file, pattern) {
  const text = readFileSync(join(root, file), 'utf8');
  const out = text.replace(pattern, (match, path) => {
    const files = filesFor(path);
    if (files.length === 0) {
      errors.push(`${file}: ${path} does not exist`);
      return match;
    }
    stamped += 1;
    const version = digest(files);
    return match.replace(path, () => `${path}?v=${version}`);
  });
  writeFileSync(join(root, file), out);
}

// Quoted asset paths in JavaScript, including template literals. Stamped first, because
// a changed script needs its new hash before the HTML that loads it is stamped.
const JS_ASSET = /['"`](assets\/[^'"`\s?]+\.[A-Za-z0-9]+)(?=['"`])/g;
// Asset paths in HTML attributes, srcset lists, absolute URLs and the manifest.
const DOC_ASSET = /(?<![\w-])(assets\/[\w./-]+\.[A-Za-z0-9]+)(?![\w./?-])/g;

const scripts = [];
(function walk(dir) {
  for (const entry of readdirSync(join(root, dir), { withFileTypes: true })) {
    const path = dir ? `${dir}/${entry.name}` : entry.name;
    if (entry.isDirectory()) walk(path);
    else if (path.endsWith('.js')) scripts.push(path);
  }
})('assets');

for (const file of scripts) stamp(file, JS_ASSET);
for (const file of ['index.html', 'site.webmanifest']) {
  if (existsSync(join(root, file))) stamp(file, DOC_ASSET);
}

if (errors.length > 0) {
  console.error(`stamp-assets: ${errors.length} broken asset reference(s):\n  ${errors.join('\n  ')}`);
  process.exit(1);
}
console.log(`stamp-assets: versioned ${stamped} asset URL(s) in ${root}`);
