#!/usr/bin/env node
// Pack each plugin folder into a release zip, compute sha256, update
// registry.json. Run from repo root: `node scripts/build-zips.mjs`.
//
// Files included in each zip: plugin.toml (required), README.md (if present),
// assets/** (if present).
// Excluded: src/, Cargo.toml, Cargo.lock, target/, *.rs (the runtime code
// is v0.4-pending — it ships in a follow-up zip when the host lands).
//
// Pure-JS implementation. No PowerShell / tar / zip / 7z dependency — uses
// node's built-in `zlib.deflateRawSync` + manual ZIP container assembly. The
// zip format is documented in APPNOTE.TXT §4; this covers the subset we need
// (STORE / DEFLATE entries, no encryption, no zip64).

import { deflateRawSync } from "node:zlib";
import { createHash } from "node:crypto";
import {
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "..");
const distDir = join(root, "dist");
const registryPath = join(root, "registry.json");

// CRC-32 / ISO-3309, table-driven so each zip build doesn't recompute the
// polynomial reflections. Defined at module-init time so the helpers below
// can reference it.
const CRC_TABLE = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) {
      c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    }
    t[n] = c >>> 0;
  }
  return t;
})();

const registry = JSON.parse(readFileSync(registryPath, "utf8"));

if (existsSync(distDir)) {
  rmSync(distDir, { recursive: true, force: true });
}
mkdirSync(distDir, { recursive: true });

const INCLUDE_FILES = ["plugin.toml", "README.md"];

let registryDirty = false;

for (const entry of registry.plugins) {
  const pluginDir = join(root, entry.id);
  if (!existsSync(pluginDir)) {
    console.warn(`[skip] ${entry.id}: directory missing`);
    continue;
  }
  if (!existsSync(join(pluginDir, "plugin.toml"))) {
    console.warn(`[skip] ${entry.id}: plugin.toml missing`);
    continue;
  }

  const files = [];
  for (const name of INCLUDE_FILES) {
    const p = join(pluginDir, name);
    if (existsSync(p)) {
      files.push({ name, body: readFileSync(p) });
    }
  }
  const assetsDir = join(pluginDir, "assets");
  if (existsSync(assetsDir) && statSync(assetsDir).isDirectory()) {
    for (const rel of walk(assetsDir, "")) {
      const abs = join(assetsDir, rel);
      files.push({
        name: `assets/${rel.split(sep).join("/")}`,
        body: readFileSync(abs),
      });
    }
  }

  const zipBytes = buildZip(files);
  const zipPath = join(distDir, `${entry.id}-${entry.version}.zip`);
  writeFileSync(zipPath, zipBytes);

  const sha = createHash("sha256").update(zipBytes).digest("hex");
  const size = zipBytes.length;

  if (entry.sha256 !== sha || entry.size_bytes !== size) {
    entry.sha256 = sha;
    entry.size_bytes = size;
    registryDirty = true;
  }

  console.log(
    `[ok]   ${entry.id}-${entry.version}.zip · ${size} bytes · ${sha.slice(0, 12)}…`,
  );
}

if (registryDirty) {
  registry.updated_unix = Math.floor(Date.now() / 1000);
  writeFileSync(registryPath, JSON.stringify(registry, null, 2) + "\n");
  console.log("[ok]   registry.json updated");
} else {
  console.log("[ok]   registry.json already in sync");
}

// --- pure-JS zip writer ---------------------------------------------------

function buildZip(files) {
  const parts = [];
  const central = [];
  let offset = 0;
  for (const f of files) {
    const nameBytes = Buffer.from(f.name, "utf8");
    const compressed = deflateRawSync(f.body);
    const crc = crc32(f.body);
    const useDeflate = compressed.length < f.body.length;
    const method = useDeflate ? 8 : 0;
    const payload = useDeflate ? compressed : f.body;

    // local file header
    const lfh = Buffer.alloc(30);
    lfh.writeUInt32LE(0x04034b50, 0); // signature
    lfh.writeUInt16LE(20, 4);          // version needed
    lfh.writeUInt16LE(0, 6);           // gp bit flag
    lfh.writeUInt16LE(method, 8);
    lfh.writeUInt16LE(0, 10);          // mod time
    lfh.writeUInt16LE(0, 12);          // mod date
    lfh.writeUInt32LE(crc, 14);
    lfh.writeUInt32LE(payload.length, 18); // compressed size
    lfh.writeUInt32LE(f.body.length, 22);  // uncompressed size
    lfh.writeUInt16LE(nameBytes.length, 26);
    lfh.writeUInt16LE(0, 28);          // extra field length
    parts.push(lfh, nameBytes, payload);

    // central directory header — we'll concat after all locals
    const cdh = Buffer.alloc(46);
    cdh.writeUInt32LE(0x02014b50, 0);
    cdh.writeUInt16LE(20, 4);          // version made by
    cdh.writeUInt16LE(20, 6);          // version needed
    cdh.writeUInt16LE(0, 8);           // gp bit flag
    cdh.writeUInt16LE(method, 10);
    cdh.writeUInt16LE(0, 12);          // mod time
    cdh.writeUInt16LE(0, 14);          // mod date
    cdh.writeUInt32LE(crc, 16);
    cdh.writeUInt32LE(payload.length, 20);
    cdh.writeUInt32LE(f.body.length, 24);
    cdh.writeUInt16LE(nameBytes.length, 28);
    cdh.writeUInt16LE(0, 30);          // extra field length
    cdh.writeUInt16LE(0, 32);          // comment length
    cdh.writeUInt16LE(0, 34);          // disk number start
    cdh.writeUInt16LE(0, 36);          // internal attrs
    cdh.writeUInt32LE(0, 38);          // external attrs
    cdh.writeUInt32LE(offset, 42);
    central.push(cdh, nameBytes);

    offset += lfh.length + nameBytes.length + payload.length;
  }

  const centralOffset = offset;
  const centralBuf = Buffer.concat(central);
  parts.push(centralBuf);

  // end of central directory record
  const eocd = Buffer.alloc(22);
  eocd.writeUInt32LE(0x06054b50, 0);
  eocd.writeUInt16LE(0, 4);
  eocd.writeUInt16LE(0, 6);
  eocd.writeUInt16LE(files.length, 8);
  eocd.writeUInt16LE(files.length, 10);
  eocd.writeUInt32LE(centralBuf.length, 12);
  eocd.writeUInt32LE(centralOffset, 16);
  eocd.writeUInt16LE(0, 20);
  parts.push(eocd);

  return Buffer.concat(parts);
}

function crc32(buf) {
  let c = 0xffffffff;
  for (let i = 0; i < buf.length; i++) {
    c = CRC_TABLE[(c ^ buf[i]) & 0xff] ^ (c >>> 8);
  }
  return (c ^ 0xffffffff) >>> 0;
}

function walk(dir, prefix) {
  const out = [];
  for (const name of readdirSync(dir)) {
    const abs = join(dir, name);
    const rel = prefix ? join(prefix, name) : name;
    const s = statSync(abs);
    if (s.isDirectory()) {
      out.push(...walk(abs, rel));
    } else {
      out.push(rel);
    }
  }
  return out;
}
