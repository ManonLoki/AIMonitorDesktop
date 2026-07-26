# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

AIMonitorDesktop — a Tauri 2 desktop app (Windows/macOS) that is a 1:1 port of an existing Android app (`/Users/manonloki/Documents/my-work/ai/AiMonitorAndroid`). It shows a 1–5×1–5 grid of tiles (AI name, username, image, content, updated time) and exposes an HTTP API + mDNS/UDP discovery so the Android app (or anything else on the LAN) can push tile updates to it. Product requirements are in `PRODUCT_REQUIREMENTS.md`; hard requirements worth remembering: no dashboard/decoration UI beyond the monitor canvas, and the window maximizes on launch.

## Commands

```bash
pnpm install
pnpm run dev          # vite dev server only
pnpm run tauri dev    # full desktop app with hot reload
pnpm run check        # tsc --build (type-check, no emit) — run before any release build
pnpm run build        # tsc --build && vite build (frontend only)
```

Release builds (macOS host required for both):

```bash
pnpm run build:mac      # universal DMG; AIMONITOR_MAC_TARGET=aarch64-apple-darwin|x86_64-apple-darwin for single-arch
pnpm run build:win      # cross-compiles Windows x64 NSIS installer via cargo-xwin
pnpm run build:release  # both, sequentially
```

`scripts/build-release.mjs` only wipes/repopulates `publish/` after every requested platform build succeeds; it never copies partial artifacts. Windows cross-build needs `cargo-xwin`, NSIS (`makensis`), and LLVM (`llvm-rc` on `PATH`) — `brew install llvm nsis && cargo install --locked cargo-xwin && rustup target add x86_64-pc-windows-msvc`. There is no separate frontend test suite or linter configured — `pnpm run check` is the only automated verification step. Rust unit tests live inline in `src-tauri/src/lib.rs` (`#[cfg(test)] mod tests`) and run with `cargo test` from `src-tauri/`.

Full release rules (canonical naming, source-to-output map, prerequisite troubleshooting) are in `.agents/skills/build-aimonitor-desktop/` — read `references/release-contract.md` before touching build config or diagnosing a packaging failure. Key invariant: the product/binary/bundle/installer name must stay `AIMonitorDesktop` everywhere; don't touch `bundle.targets`, `mainBinaryName`, or swap NSIS/DMG for WiX/MSI.

## Architecture

**The backend is the source of truth, and it lives entirely in one file: `src-tauri/src/lib.rs`.** There is no separate backend crate or module split — a single `Runtime` struct (rows/columns, tiles, device identity, window geometry, image dir) is held behind `Arc<RwLock<...>>` and shared between three concurrent subsystems started in `run()`:

1. **A hand-rolled HTTP server** (`start_http_server`, plain `std::net::TcpListener` + a 4-thread worker pool — no web framework) serving the Android-compatible REST API: `/health`, `/api/device`, `/api/config`, `/api/images` (GET list/POST upload), `/api/images/{filename}` (GET/DELETE), `/api/slots/{1..25}` (POST update tile / DELETE clear tile). Port auto-selects upward from `10241`. This API's shape is dictated by the Android app — don't change request/response fields without checking Android-side compatibility. `GET /api/images` uses `probe_image_file` to sniff each file's magic bytes and stat its size instead of reading the whole file into memory — keep that pattern if you touch the listing path, since the image directory can hold multi-MB GIFs.
2. **UDP discovery** (`start_udp_discovery`) — listens on `8080`, replies to a literal `AIMONITOR_DISCOVER_V1` broadcast with device JSON.
3. **mDNS** (`start_mdns`) — registers `_aimonitor._tcp.local.` via `mdns-sd`.

Tauri commands (`get_monitor_state`, `set_grid`, `set_image_display_mode`, `set_device_name`, `set_auto_start`) are the only way the frontend mutates state; every mutation calls `runtime.save_preferences()` (writes `preferences.json` in the app config dir) and `runtime.changed()` (emits a `monitor-state-changed` Tauri event). Window geometry is persisted on move/resize/close and validated against currently attached monitors on restore (`rectangles_have_visible_overlap` — this is what the two Rust unit tests cover).

**The frontend has no router, server-state library, or global client-state store — it doesn't need one.** The whole app is one Rust-driven data source fanned out to a few components:

- `src/types/monitor.ts` — shared types (`MonitorState`, `MonitorTile`, `ImageDisplayMode`) mirroring the Rust structs field-for-field (serde converts `snake_case` → `camelCase`).
- `src/hooks/useMonitorState.ts` — the single data source. Calls `get_monitor_state` once on mount, then re-fetches on the `monitor-state-changed` Tauri event. There is no local optimistic state; every mutation goes Rust → event → refetch.
- `src/components/Icon.tsx`, `MonitorCanvas.tsx`, `SettingsPanel.tsx` — presentational pieces. `MonitorCanvas` renders the tile grid (images fetched from `http://127.0.0.1:{port}/api/images/{filename}`); `SettingsPanel` is the only place that calls the mutating Tauri commands.
- `src/MonitorApp.tsx` — composition root (sidebar nav + workspace), rendered directly by `src/main.tsx`. Switching between "monitor" and "settings" is local `useState`, not a route.
- `src/components/reactbits/` — hand-maintained React Bits animation components (`AnimatedContent`, `SpotlightCard`) adapted for desktop and `prefers-reduced-motion` — see `THIRD_PARTY_NOTICES.md` for upstream licensing. Extend these in place rather than pulling the upstream package.

All Chinese UI copy must match the existing Android app's wording per the product requirements.

## Stack constraints (`TECH_STACK.md`)

These are binding choices for this repo — don't introduce a second library covering the same responsibility:

| Responsibility | Library | Boundary |
| --- | --- | --- |
| Desktop runtime | Tauri 2 | native window, OS capabilities, Rust commands, packaging |
| Motion | React Bits + GSAP 3 | entrance/scroll/interaction only; must respect `prefers-reduced-motion` |
| Frontend↔backend communication | `@tauri-apps/api` (`invoke` + `listen`) | all state reads/writes; no HTTP client in the loop |

Mantine, TanStack Router/Query, Axios, and Jotai were present in `package.json` from initial scaffolding but never wired into the live app — they've been removed along with the dead files that referenced them (`src/router.tsx`, `src/pages/`, `src/query-client.ts`, `src/state/`, `src/api/`). If a future feature genuinely needs routing, server-state caching, or shared client state, evaluate and re-add deliberately rather than assuming the old scaffold reflects current intent. When bumping a dependency's major version, update the pinned version, `pnpm-lock.yaml`, and the table in `TECH_STACK.md` together.

## Conventions

- Dependencies use exact/pinned versions (`package.json` has no `^`/`~`; `Cargo.toml` pins with `=`) and the lockfiles are committed — don't loosen version specifiers.
- All user-facing strings are Simplified Chinese, matching the Android app.
