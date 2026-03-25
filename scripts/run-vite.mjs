#!/usr/bin/env node
/**
 * Run Vite from the repo root with shell:false-friendly spawn (no npm.cmd on Windows),
 * so Ctrl+C does not trigger "Terminate batch job (Y/N)?".
 *
 * Usage: node scripts/run-vite.mjs [vite CLI args...]
 *   dev default: no extra args (dev server)
 *   build: node scripts/run-vite.mjs build
 */

import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(scriptDir, "..");
const viteJs = path.join(projectRoot, "node_modules", "vite", "bin", "vite.js");
const extra = process.argv.slice(2);

if (!existsSync(viteJs)) {
  console.error("[run-vite] Missing vite. Run npm install.");
  process.exit(1);
}

const child = spawn(process.execPath, [viteJs, ...extra], {
  cwd: projectRoot,
  stdio: "inherit",
  shell: false,
});

child.on("error", (err) => {
  console.error("[run-vite]", err.message);
  process.exit(1);
});

child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }
  process.exit(code ?? 0);
});
