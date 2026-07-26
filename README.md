# AIMonitorDesktop

AI监控平台桌面版，由 ManonLoki 开发。

## 技术栈

- 桌面框架：Tauri 2
- 前端框架：React 19 + TypeScript
- UI 组件与动效：Mantine + React Bits + GSAP
- 路由：TanStack Router
- 服务端状态与网络请求：TanStack Query + Axios
- 客户端状态：Jotai
- 构建工具：Vite 8
- 包管理器：pnpm（依赖使用精确版本并提交锁文件）

具体选型边界见 [TECH_STACK.md](./TECH_STACK.md)。

React Bits 组件以可维护源码方式放在 `src/components/reactbits/`，并针对桌面端交互与系统“减少动态效果”偏好做了适配。上游许可见 [THIRD_PARTY_NOTICES.md](./THIRD_PARTY_NOTICES.md)。

## 环境要求

- Node.js >= 22.12
- pnpm 10
- Rust stable
- Tauri 对应平台依赖

## 开发

```bash
pnpm install
pnpm run dev
```

启动桌面应用：

```bash
pnpm run tauri dev
```

## 校验与发布构建

```bash
pnpm run check
pnpm run build
```

统一发布脚本会先完成平台构建，全部成功后清理旧 `publish` 内容，再复制并重命名安装包：

```bash
pnpm run build:mac
pnpm run build:win
pnpm run build:release
```

- `build:mac`：在 macOS 上生成通用架构 DMG。
- `build:win`：在 macOS/Linux 上使用 `cargo-xwin` 交叉编译 Windows x64，并生成 NSIS 安装器。
- `build:release`：依次构建 macOS 和 Windows 安装器。
- 设置 `AIMONITOR_MAC_TARGET=aarch64-apple-darwin` 或 `x86_64-apple-darwin` 可只构建单一 macOS 架构。

Windows 交叉构建要求安装 `cargo-xwin`、NSIS、LLVM，并确保 `llvm-rc` 位于 `PATH`。macOS 可执行：

```bash
brew install llvm nsis
cargo install --locked cargo-xwin
rustup target add x86_64-pc-windows-msvc
```

最终产物统一位于 `publish/`，文件名以 `AIMonitorDesktop` 开头，并附带 SHA-256 校验文件。

## 环境变量

复制 `.env.example` 为 `.env.local`，按需设置 API 地址：

```dotenv
VITE_API_BASE_URL=http://localhost:8080/api
```
