#!/usr/bin/env node
/**
 * Sync app version across package.json, package-lock.json (root), Cargo.toml [package], tauri.conf.json.
 * Usage: node scripts/bump-version.mjs patch|minor|major|<semver>
 */
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = join(__dirname, "..");

const SEMVER = /^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?(\+[0-9A-Za-z.-]+)?$/;

function readJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

function parseVersion(v) {
  const m = v.match(/^(\d+)\.(\d+)\.(\d+)/);
  if (!m) return null;
  return { major: +m[1], minor: +m[2], patch: +m[3], rest: v.slice(m[0].length) };
}

function bumpFrom(current, kind) {
  const p = parseVersion(current);
  if (!p) throw new Error(`Invalid current version: ${current}`);
  const { major, minor, patch, rest } = p;
  if (rest) {
    throw new Error(`Cannot bump prerelease/build suffix in place: ${current}. Pass an explicit semver instead.`);
  }
  if (kind === "major") return `${major + 1}.0.0`;
  if (kind === "minor") return `${major}.${minor + 1}.0`;
  if (kind === "patch") return `${major}.${minor}.${patch + 1}`;
  throw new Error(`Unknown bump kind: ${kind}`);
}

function resolveNext(current, arg) {
  if (arg === "patch" || arg === "minor" || arg === "major") {
    return bumpFrom(current, arg);
  }
  if (!SEMVER.test(arg)) {
    console.error(`Invalid version or bump: ${arg}`);
    console.error("Expected: patch | minor | major | e.g. 2.1.0");
    process.exit(1);
  }
  return arg;
}

function setCargoPackageVersion(content, newVersion) {
  const lines = content.split("\n");
  let inPackage = false;
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (line.startsWith("[")) {
      inPackage = line === "[package]";
      continue;
    }
    if (inPackage && line.startsWith("version = ")) {
      lines[i] = `version = "${newVersion}"`;
      return lines.join("\n");
    }
  }
  throw new Error('Could not find [package] version = "..." in Cargo.toml');
}

function main() {
  const arg = process.argv[2];
  if (!arg) {
    console.error("Usage: node scripts/bump-version.mjs <patch|minor|major|semver>");
    process.exit(1);
  }

  const pkgPath = join(root, "package.json");
  const lockPath = join(root, "package-lock.json");
  const cargoPath = join(root, "src-tauri", "Cargo.toml");
  const tauriPath = join(root, "src-tauri", "tauri.conf.json");

  const pkg = readJson(pkgPath);
  const current = pkg.version;
  if (!current) {
    console.error("package.json has no version");
    process.exit(1);
  }

  const next = resolveNext(current, arg);
  if (next === current) {
    console.log(`Already at ${next}; no file changes.`);
    return;
  }

  pkg.version = next;
  writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + "\n", "utf8");

  try {
    const lock = readJson(lockPath);
    lock.version = next;
    if (lock.packages && lock.packages[""]) {
      lock.packages[""].version = next;
    }
    writeFileSync(lockPath, JSON.stringify(lock, null, 2) + "\n", "utf8");
  } catch (e) {
    console.warn("package-lock.json not updated:", e.message);
  }

  const cargo = readFileSync(cargoPath, "utf8");
  writeFileSync(cargoPath, setCargoPackageVersion(cargo, next), "utf8");

  const tauri = readJson(tauriPath);
  tauri.version = next;
  writeFileSync(tauriPath, JSON.stringify(tauri, null, 2) + "\n", "utf8");

  console.log(`Version: ${current} → ${next}`);
  console.log("Updated: package.json, package-lock.json (root), src-tauri/Cargo.toml, src-tauri/tauri.conf.json");
}

main();
