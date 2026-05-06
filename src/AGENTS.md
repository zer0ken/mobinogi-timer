# Codex Notes for Frontend

## Scope

This directory contains the static frontend loaded by Tauri:

- `settings.html`: settings window.
- `index.html`: transparent overlay window.
- `main.js`: overlay timer event handling.
- `style.css`: shared UI styling.

## Guidelines

- Keep this as dependency-free static HTML/CSS/JS unless the user explicitly asks for a frontend build system.
- Tauri APIs are accessed through `window.__TAURI__` because `withGlobalTauri` is enabled.
- Preserve the overlay's small, transparent, always-on-top use case. Avoid adding heavy layout, animations, or UI that can obscure gameplay.
- `timer-update` events come from Rust and drive overlay states: `idle`, `duration`, and `cooldown`.
- `settings-updated` should refresh visible overlay settings without requiring app restart.
