# Releasing (GitHub Release CI)

## Continuous integration

On push/PR to `main` or `master`, **`.github/workflows/ci.yml`** runs: `npm run version:check` (sync of `package.json`, `src-tauri/Cargo.toml`, `tauri.conf.json`), ESLint, `vue-tsc`, Vite build, `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`.

## When it runs (release builds)

The **release** workflow (`.github/workflows/release.yml`) runs **only** when you push a **release tag**. It does **not** replace CI on every push.

- Tag pattern: `v` prefix, e.g. `v1.0.0`, `v1.2.3-beta.1` (workflow glob: `v*`).
- Example:

  ```bash
  git tag v1.0.0
  git push origin v1.0.0
  ```

## Build matrix (mainstream CPUs)

| OS      | Arch          | Runner                 |
| ------- | ------------- | ---------------------- |
| macOS   | Apple Silicon | `macos-latest`         |
| macOS   | Intel x64     | `macos-latest` (cross) |
| Linux   | x64           | `ubuntu-22.04`         |
| Linux   | arm64         | `ubuntu-24.04-arm`     |
| Windows | x64           | `windows-latest`       |
| Windows | arm64         | `windows-11-arm`       |

If a matrix row fails in your environment (e.g. private repo without ARM runners), remove the corresponding `include` entry in `.github/workflows/release.yml`.

## Repository settings

1. **GitHub** → repo **Settings** → **Actions** → **General** → **Workflow permissions**  
   Enable **Read and write permissions** (required to create Releases and upload assets).

2. Before release, align **`src-tauri/tauri.conf.json`** and **`src-tauri/Cargo.toml`** `version` with the tag (e.g. tag `v1.0.0` → version `1.0.0`) so users and updaters stay consistent.

## Artifacts

The workflow uses [tauri-apps/tauri-action](https://github.com/tauri-apps/tauri-action) to upload per-platform installers for the tagged **GitHub Release** (`.dmg`, `.msi`/`.exe`, `.deb`/`.AppImage`, etc., per current Tauri bundle settings).
