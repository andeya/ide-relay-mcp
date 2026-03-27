#!/usr/bin/env node
/**
 * Exit 0 when package.json, src-tauri/Cargo.toml [package].version, and
 * src-tauri/tauri.conf.json version are identical. Used in CI to catch drift.
 */
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = join(__dirname, "..");

function readJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

function cargoPackageVersion(toml) {
  const lines = toml.split(/\r?\n/);
  let inPackage = false;
  for (const line of lines) {
    const trimmed = line.trimEnd();
    if (trimmed.startsWith("[")) {
      inPackage = trimmed === "[package]";
      continue;
    }
    if (inPackage && trimmed.startsWith("version")) {
      const m = trimmed.match(/^version\s*=\s*"([^"]+)"/);
      return m ? m[1] : null;
    }
  }
  return null;
}

function main() {
  const pkg = readJson(join(root, "package.json")).version;
  const cargo = readFileSync(join(root, "src-tauri", "Cargo.toml"), "utf8");
  const tauri = readJson(join(root, "src-tauri", "tauri.conf.json")).version;
  const cargoV = cargoPackageVersion(cargo);

  const parts = [
    ["package.json", pkg],
    ["src-tauri/Cargo.toml [package]", cargoV],
    ["src-tauri/tauri.conf.json", tauri],
  ];

  if (!cargoV) {
    console.error("Could not read version from src-tauri/Cargo.toml [package]");
    process.exit(1);
  }

  if (pkg === cargoV && pkg === tauri) {
    console.log(`version sync OK: ${cargoV}`);
    return;
  }

  console.error("Version mismatch — run: npm run version:patch (or minor/major)");
  for (const [name, v] of parts) {
    console.error(`  ${name}: ${v ?? "(missing)"}`);
  }
  process.exit(1);
}

main();
