# Codex Notes for Tauri Backend

## Scope

This directory contains the Rust/Tauri application, configuration, icons, and generated Tauri metadata.

## Build Context

- `tauri.conf.json` is the app-facing version source for update checks and release artifacts.
- `Cargo.toml` keeps the Rust crate version at `0.0.0`; do not use it for product release versioning unless the release process is changed deliberately.
- The app is Windows-first and uses packet capture via the `pcap` crate.
- Runtime Npcap detection checks for `wpcap.dll` under `System32\Npcap` or `System32`.

## Maintenance Guidelines

- When changing settings, keep serde defaults compatible with existing `settings.json` files.
- When changing Tauri commands, update frontend callers in `src` in the same change.
- Avoid panics in packet-capture startup paths. Missing Npcap or missing interfaces should fail quietly in the app UI flow.
- Keep update checking suffix-aware. Auto builds should only offer compatible `-auto` releases.
