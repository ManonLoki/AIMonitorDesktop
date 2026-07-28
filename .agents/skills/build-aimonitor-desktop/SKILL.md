---
name: build-aimonitor-desktop
description: Build, package, and verify AIMonitorDesktop release artifacts for macOS and Windows. Use when Codex needs to check release prerequisites, build a macOS DMG, cross-compile a Windows NSIS installer with xwin, troubleshoot Tauri packaging, verify application or installer naming and icons, or clean and populate the project's publish directory.
---

# Build AIMonitorDesktop

Use the repository-owned release script instead of assembling Tauri commands manually. Preserve the canonical product name `AIMonitorDesktop` across the window title, Dock or taskbar identity, executable, bundle, installer, and published filenames.

## Workflow

1. Run from the repository root.
2. Read `references/release-contract.md` before changing build configuration or diagnosing a release failure.
3. Inspect `package.json`, `src-tauri/tauri.conf.json`, and the relevant platform-specific Tauri configuration.
4. Run `pnpm run check` before a release build.
5. Select exactly one build command:
   - macOS: `pnpm run build:mac`
   - Windows x64 via xwin: `pnpm run build:win`
   - Both: `pnpm run build:release`
6. Inspect `publish/` and verify every deliverable starts with `AIMonitorDesktop`.
7. Verify `AIMonitorDesktop-SHA256SUMS.txt` contains one entry per installer.

## Guardrails

- Do not copy target artifacts manually. The release script only clears `publish/` after every requested platform build succeeds.
- Do not change `bundle.targets` in the base Tauri config. Keep DMG and NSIS targets in `tauri.macos.conf.json` and `tauri.windows.conf.json`.
- Do not use WiX or MSI for xwin builds. Cross-platform packaging is NSIS-only.
- Do not remove `mainBinaryName`; it guarantees the executable is named `AIMonitorDesktop`.
- Do not bypass missing `cargo-xwin`, `makensis`, or `llvm-rc`. Report the missing prerequisite and use the setup command in the reference.
- Do not claim Windows signing; xwin builds remain unsigned until a separate
  Authenticode certificate and signing command are configured.
- Do not claim Apple notarization unless the build log and artifact verification
  confirm it. macOS release builds are Developer ID signed, while notarization
  additionally requires separately managed Apple credentials.

## Verification

After building, verify:

- macOS filename: `AIMonitorDesktop-macOS-<architecture>-v<version>.dmg`
- Windows filename: `AIMonitorDesktop-Windows-x64-v<version>-setup.exe`
- The macOS bundle executable is `AIMonitorDesktop`.
- The Windows PE executable and NSIS metadata use `AIMonitorDesktop`.
- NSIS uses the checked-in `.ico`, header/sidebar bitmaps, and custom Chinese/English strings.
- Recompute a published file's SHA-256 and compare it with the checksum file when integrity matters.
