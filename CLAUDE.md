# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

AIMonitorDesktop — a Tauri 2 desktop app (Windows/macOS) that is a 1:1 port of an existing Android app (`/Users/manonloki/Documents/my-work/ai/AiMonitorAndroid`). It shows a 1–5×1–5 grid of tiles (AI name, username, image, content, updated time) and exposes an HTTP API + mDNS/UDP discovery so the Android app (or anything else on the LAN) can push tile updates to it. Product requirements are in `PRODUCT_REQUIREMENTS.md`; hard requirements worth remembering: no dashboard/decoration UI beyond the monitor canvas, and the window maximizes on launch.

## Code gate: 400-line file limit

**This is the enforced, canonical standard for this repo — every source file (`.ts`/`.tsx`/`.rs`) must stay at or under 400 lines.** When a file would grow past that, split it into modules/files along responsibility boundaries instead of letting it grow — see the backend module layout below for the reference example of how this repo does that split. `scripts/check-file-length.mjs` scans `src/` and `src-tauri/src/` (skipping `node_modules`, `target`, `dist`, `gen`) and fails the build if any file exceeds the limit; it's wired into `pnpm run check`, so it runs on every type-check and before every release build. Run it standalone with `pnpm run check:filesize`. Don't raise the limit or exclude a file from the scan to make a change fit — split the file instead.

## Commands

```bash
pnpm install
pnpm run dev              # vite dev server only
pnpm run tauri dev        # full desktop app with hot reload
pnpm run check            # tsc --build (type-check, no emit) + the 400-line file gate — run before any release build
pnpm run check:filesize   # just the 400-line file gate, standalone
pnpm run build            # tsc --build && vite build (frontend only)
```

Release builds (macOS host required for both):

```bash
pnpm run build:mac      # universal DMG; AIMONITOR_MAC_TARGET=aarch64-apple-darwin|x86_64-apple-darwin for single-arch
pnpm run build:win      # cross-compiles Windows x64 NSIS installer via cargo-xwin
pnpm run build:release  # both, sequentially
```

`scripts/build-release.mjs` only wipes/repopulates `publish/` after every requested platform build succeeds; it never copies partial artifacts. Windows cross-build needs `cargo-xwin`, NSIS (`makensis`), and LLVM (`llvm-rc` on `PATH`) — `brew install llvm nsis && cargo install --locked cargo-xwin && rustup target add x86_64-pc-windows-msvc`. There is no separate frontend test suite or linter configured — `pnpm run check` is the only automated verification step. Rust unit tests live inline next to the code they cover (`#[cfg(test)] mod tests` in `image.rs` and `window_geometry.rs`) and run with `cargo test` from `src-tauri/`.

Full release rules (canonical naming, source-to-output map, prerequisite troubleshooting) are in `.agents/skills/build-aimonitor-desktop/` — read `references/release-contract.md` before touching build config or diagnosing a packaging failure. Key invariant: the product/binary/bundle/installer name must stay `AIMonitorDesktop` everywhere; don't touch `bundle.targets`, `mainBinaryName`, or swap NSIS/DMG for WiX/MSI.

## Architecture

**The backend is the source of truth.** It's a single Cargo crate (`src-tauri/src/`) split into modules by responsibility — no file crosses the 400-line gate above. A single `Runtime` struct (rows/columns, tiles, device identity, window geometry, image dir) is held behind `Arc<RwLock<...>>` (the `SharedRuntime` alias) and shared between three concurrent subsystems started from `lib.rs::run()`:

```text
src-tauri/src/
├── main.rs              # binary entry point, forwards to lib::run()
├── lib.rs                # Tauri entry: module wiring, setup() assembly, invoke_handler list
├── constants.rs          # shared constants: ports, size limits, API_VERSION
├── model.rs               # MonitorTile / MonitorState / WindowGeometry / Preferences (serde structs)
├── runtime.rs             # Runtime struct + SharedRuntime, snapshot/save_preferences/changed, load_preferences
├── commands.rs            # the 5 #[tauri::command] functions — the frontend's only write path
├── device_info.rs         # default_device_name, local_ipv4
├── image.rs                # detect_image, make_gif_loop_forever, safe_image_filename (sync) + probe_image_file (async, tokio::fs) + tests
├── window_geometry.rs      # rectangles_have_visible_overlap, restore/save window geometry (+ tests)
├── discovery.rs            # UDP broadcast discovery, tokio::net::UdpSocket, pure async
├── mdns.rs                  # mDNS service registration (start_mdns)
└── http/
    ├── mod.rs                # build_router + start_http_server (tokio::net::TcpListener + axum::serve, no manual thread pool) + shared error_json helper
    ├── device.rs              # /health, /api/config, /api/device
    ├── images.rs              # /api/images (list/upload), /api/images/{filename} (get/delete)
    └── slots.rs                # /api/slots/{1..25} (update/clear a tile)
```

1. **The HTTP server** (`http/`) is an Axum app running on Tauri's own Tokio runtime — no manual `TcpListener` parsing or thread pool. `http::start_http_server` finds the first available port from `10241` upward with an async bind loop (via `tauri::async_runtime::block_on`, since Tauri's `setup()` callback is itself synchronous), then hands the listener to `axum::serve(...)` spawned as a background task (`tauri::async_runtime::spawn`) — every connection after that is scheduled onto Tokio, not a hand-rolled worker thread. It serves the Android-compatible REST API: `/health`, `/api/device`, `/api/config`, `/api/images` (GET list/POST upload), `/api/images/{filename}` (GET/DELETE), `/api/slots/{1..25}` (POST update tile / DELETE clear tile). This API's shape is dictated by the Android app — don't change request/response fields or status codes without checking Android-side compatibility; handlers build `{"error": "..."}` JSON responses via the shared `http::error_json` helper (not Axum's default rejection bodies) specifically to preserve that contract — route modules import it with `use super::error_json` rather than each defining their own. `GET /api/images` (`http/images.rs`) uses `probe_image_file` (`image.rs`, now `async fn` over `tokio::fs`) to sniff each file's magic bytes and stat its size instead of reading the whole file into memory — keep that pattern if you touch the listing path, since the image directory can hold multi-MB GIFs. Request body size is capped via `DefaultBodyLimit` (`constants::MAX_BODY_BYTES`); CORS is a permissive `tower_http::cors::CorsLayer` handling `OPTIONS` automatically.
2. **UDP discovery** (`discovery.rs::start_udp_discovery`) — a `tokio::net::UdpSocket` task (also via `tauri::async_runtime::spawn`) listening on `8080`, replies to a literal `AIMONITOR_DISCOVER_V1` broadcast with device JSON. No blocking socket or dedicated `std::thread`.
3. **mDNS** (`mdns.rs::start_mdns`) — registers `_aimonitor._tcp.local.` via `mdns-sd`. This crate manages its own internal thread for the service daemon; that's the third-party library's implementation detail, not something this codebase hand-rolls, so it's out of scope for the "network layer is pure async" rule below.

The HTTP/UDP network layer is pure async by design (Tokio + Axum, no manual threads or blocking sockets) — this is a deliberate architectural choice, not just an implementation detail, so keep new endpoints and socket code on this model rather than reintroducing blocking I/O or manual thread spawning.

Tauri commands (`get_monitor_state`, `set_grid`, `set_image_display_mode`, `set_device_name`, `set_auto_start`, all in `commands.rs`) are the only way the frontend mutates state; every mutation calls `runtime.save_preferences()` (writes `preferences.json` in the app config dir) and `runtime.changed()` (emits a `monitor-state-changed` Tauri event). Window geometry is persisted on move/resize/close and validated against currently attached monitors on restore (`window_geometry::rectangles_have_visible_overlap` — this is what two of the Rust unit tests cover).

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
- No source file exceeds 400 lines — see "Code gate: 400-line file limit" above. This is enforced by `pnpm run check`, not just a style preference.
