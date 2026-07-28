# AIMonitorDesktop release contract

## Canonical identity

- Product, window, Dock or taskbar, executable, app bundle, installer, and published artifact prefix: `AIMonitorDesktop`
- Bundle identifier: `com.manonloki.aimonitordesktop`
- Publisher: `ManonLoki`
- Published directory: `publish/`

## Commands

| Intent | Command | Output |
| --- | --- | --- |
| Type-check | `pnpm run check` | No release artifact |
| macOS | `pnpm run build:mac` | Universal DMG by default |
| Windows | `pnpm run build:win` | x64 NSIS installer via `cargo-xwin` |
| Both | `pnpm run build:release` | DMG and NSIS installer |

Override the macOS target with `AIMONITOR_MAC_TARGET=aarch64-apple-darwin` or `AIMONITOR_MAC_TARGET=x86_64-apple-darwin`.

## Prerequisites

Common: Node.js 22.12+, pnpm 10, Rust stable, installed frontend dependencies.

macOS signing uses the Developer ID identity declared in
`src-tauri/tauri.macos.conf.json` and installed in the login keychain. Notarization
is completed automatically by Tauri when either its Apple ID app-specific password
variables or App Store Connect API key variables are present. Otherwise the release
script uses the `AIMonitorNotary` keychain profile (overridable with
`AIMONITOR_NOTARY_PROFILE`) and refuses to publish without a stapled ticket.

macOS host:

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
```

Windows x64 cross-build on macOS:

```bash
brew install llvm nsis
cargo install --locked cargo-xwin
rustup target add x86_64-pc-windows-msvc
```

On Linux, install LLVM or LLD and NSIS with the system package manager, then install `cargo-xwin` and the Rust target as above.

## Source-to-output map

| Concern | Source |
| --- | --- |
| Published artifact prefix and version | `package.json` (`releaseName`, `version`) |
| Product and main binary name | `src-tauri/tauri.conf.json` |
| macOS bundle target | `src-tauri/tauri.macos.conf.json` |
| Windows NSIS target and branding | `src-tauri/tauri.windows.conf.json` |
| NSIS text | `src-tauri/windows/nsis/*.nsh` |
| NSIS images | `src-tauri/windows/nsis/*.bmp` |
| Release orchestration and publish cleanup | `scripts/build-release.mjs` |

## Failure interpretation

- Missing `llvm-rc`: add the Homebrew LLVM bin directory to `PATH`; the script checks common Homebrew paths automatically.
- Missing `makensis`: install NSIS.
- Missing `cargo-xwin`: install it with Cargo.
- Missing Apple target: use `rustup target add`.
- No installer in `publish/`: read the earlier build failure. The script leaves the previous `publish/` intact until all requested builds succeed.
- Missing macOS signing identity: install the Developer ID Application certificate,
  including its private key, in the login keychain and keep the identity in
  `tauri.macos.conf.json` synchronized with that certificate.
- Missing `AIMonitorNotary` credentials: create a Developer-role App Store Connect
  API key and use `xcrun notarytool store-credentials AIMonitorNotary --key
  "<AuthKey.p8>" --key-id "<KEY_ID>" --issuer "<ISSUER_ID>"`. Keep credentials
  outside the repository.
