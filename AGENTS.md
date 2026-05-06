# Codex Project Notes

## Project

- This is the auto version of `mobinogi-timer`, a Windows Tauri v2 overlay timer for Mabinogi Mobile emblem awakening.
- The app detects awakening through packet capture and requires Npcap at runtime. Native packet capture code also expects the Npcap SDK at `C:\npcap-sdk` when building.
- Current branch context is `auto`. The repository has separate release lines:
  - `manual`: hotkey/manual timer version.
  - `auto`: packet capture/automatic timer version.
  - `main`: README and GitHub Actions release workflows.

## Stack

- Frontend: static HTML/CSS/JavaScript in `src`.
- Backend: Rust/Tauri in `src-tauri`.
- Package manager: npm, with only `@tauri-apps/cli` in the root `package.json`.

## Common Commands

- Install dependencies: `npm install`
- Development app: `npx tauri dev`
- Production build: `npx tauri build`
- Rust compile check: `cargo check --manifest-path src-tauri/Cargo.toml`
- Rust formatting check: `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`

Run Tauri commands on Windows. Full builds may require Npcap SDK files and Windows build tooling.

## Release Rules

- Version format: `YYYY.M.DD-auto` for the first release of a date, or `YYYY.M.DDNN-auto` for additional releases on the same date.
  - First release example: `2026.4.26-auto`.
  - Additional same-day release example: `2026.4.2601-auto`.
- Keep the `-auto` suffix for this branch. Update checking filters releases by suffix so manual and auto versions remain independent.
- Release order for the auto version:
  1. Update `src-tauri/tauri.conf.json` version.
  2. Commit and push to `origin auto`.
  3. Run `.github/workflows/release-auto.yml` from `main`.
- Release notes for auto builds should keep the warning about possible unauthorized-program classification and the Npcap install requirement.

## Important Behavior

- Do not remove the warning language from `README.md` or release notes without explicit user instruction.
- `check_update` in `src-tauri/src/lib.rs` intentionally filters by major year and version suffix, with a legacy no-suffix to `-auto` migration path.
- Settings are stored under the user config directory in `mobinogi-timer/settings.json`.
- Development packet debug logs are written to `%APPDATA%\mobinogi-timer\debug.log` behind `debug_assertions`.

## Repo Hygiene

- Keep generated build outputs out of commits: `node_modules/`, `src-tauri/target/`, `src-tauri/gen/`, `dist/`, and `*.exe` are ignored.
- Prefer focused fixes over broad refactors. Packet constants and release/version logic are especially sensitive to game updates and existing user installs.
