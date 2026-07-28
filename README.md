# AIMonitorDesktop

AI监控平台桌面版，由 ManonLoki 开发。

局域网 HTTP API 当前为 v3。槽位更新必须携带控制端 `clientId`，并通过
`POST /api/clients/{clientId}/heartbeat` 每 30 秒续租；连续 2 分钟未续租时，
桌面端只清理该控制端拥有的宫格。

## 实机截图

### 监控界面

![AIMonitorDesktop macOS 实机运行界面](./docs/screenshots/monitor-dashboard.jpg)

> macOS 实机运行效果：2 × 2 监控宫格，其中一个终端正在上报内容。

### 设置界面

![AIMonitorDesktop macOS 设置界面](./docs/screenshots/settings.jpg)

> 可配置开机自启、设备名称、监控宫格和画布图片显示方式，并查看局域网服务信息。

## 技术栈

- 桌面框架：Tauri 2
- 前端框架：React 19 + TypeScript
- UI 动效：React Bits + GSAP
- 前后端通信：`@tauri-apps/api`（invoke / event），不经过 HTTP
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

## 发布构建（维护者手册）

发布入口已经集成到 Tauri 构建流程中，不需要再手工运行独立的签名或公证脚本。
macOS 包只有在 Developer ID 签名、公证、票据装订和 Gatekeeper 校验全部通过后，
才会进入 `publish/`。

### 首次配置构建机

安装项目依赖和 Rust 目标：

```bash
pnpm install
rustup target add aarch64-apple-darwin x86_64-apple-darwin
rustup target add x86_64-pc-windows-msvc
```

macOS 钥匙串中必须安装有效的 `Developer ID Application` 证书及其私钥。可用
以下命令检查签名身份：

```bash
security find-identity -v -p codesigning
```

Windows 交叉构建还需要 `cargo-xwin`、NSIS 和 LLVM：

```bash
brew install llvm nsis
cargo install --locked cargo-xwin
```

创建 Developer 权限的 App Store Connect API Key，将下载的 `.p8` 私钥保存到
本机安全目录，再把公证凭据写入钥匙串。尖括号内容必须替换为自己的值：

```bash
mkdir -p "$HOME/.appstoreconnect/private_keys"
chmod 700 "$HOME/.appstoreconnect/private_keys"
chmod 600 "$HOME/.appstoreconnect/private_keys/AuthKey_<KEY_ID>.p8"

xcrun notarytool store-credentials AIMonitorNotary \
  --key "$HOME/.appstoreconnect/private_keys/AuthKey_<KEY_ID>.p8" \
  --key-id "<KEY_ID>" \
  --issuer "<ISSUER_ID>"
```

验证钥匙串凭据是否可用：

```bash
xcrun notarytool history --keychain-profile AIMonitorNotary
```

证书、证书私钥、API Key、`.p8` 文件和 Issuer ID 都不得提交到仓库。若使用了
其他 profile 名称，构建前设置 `AIMONITOR_NOTARY_PROFILE`。也可以使用 Tauri
支持的 `APPLE_API_KEY`、`APPLE_API_ISSUER` 和 `APPLE_API_KEY_PATH` 环境变量，
但日常发布推荐使用钥匙串 profile，避免密钥出现在终端历史或 CI 日志中。

### 每次发布

1. 同步修改 `package.json`、`src-tauri/Cargo.toml` 和
   `src-tauri/tauri.conf.json` 中的版本号，三处必须一致。
2. 提交发布前先执行完整检查：

   ```bash
   pnpm run check
   pnpm run build
   ```

3. 根据目标选择一个发布命令：

   ```bash
   # macOS 通用架构（Apple Silicon + Intel）
   pnpm run build:mac

   # Windows x64（在 macOS/Linux 上使用 cargo-xwin）
   pnpm run build:win

   # 依次构建 macOS 通用架构和 Windows x64
   pnpm run build:release
   ```

   如只需单一 macOS 架构，可覆盖默认目标：

   ```bash
   AIMONITOR_MAC_TARGET=aarch64-apple-darwin pnpm run build:mac
   AIMONITOR_MAC_TARGET=x86_64-apple-darwin pnpm run build:mac
   ```

4. 发布成功后检查 `publish/`。脚本会在所有目标均成功后清理旧产物，并生成：

   - `AIMonitorDesktop-macOS-<架构>-v<版本>.dmg`
   - `AIMonitorDesktop-Windows-x64-v<版本>-setup.exe`
   - `AIMonitorDesktop-SHA256SUMS.txt`

macOS 自动流程为：Tauri 构建并签名 → 校验签名 → 提交 Apple 公证并等待
`Accepted` → staple 公证票据 → Gatekeeper 校验 → 复制到 `publish/`。任一步骤
失败都会终止发布，不会把未公证的 DMG 当作正式产物。

Windows 安装器目前使用 `--no-sign` 构建，因此没有 Authenticode 签名；这与
macOS 的 Developer ID 签名、公证是两套独立机制。

### 发布后验证

将下面的 DMG 文件名替换为本次实际产物：

```bash
xcrun stapler validate "publish/AIMonitorDesktop-macOS-<架构>-v<版本>.dmg"
spctl --assess --verbose=2 --type open \
  --context context:primary-signature \
  "publish/AIMonitorDesktop-macOS-<架构>-v<版本>.dmg"
shasum -a 256 -c publish/AIMonitorDesktop-SHA256SUMS.txt
```

`stapler validate` 应成功；`spctl` 输出应包含 `accepted` 和
`source=Notarized Developer ID`。最后建议把 DMG 下载到另一台 Mac，按真实用户
路径完成一次安装和首次启动测试。

### 更换电脑或轮换密钥

新电脑需要同时迁移两类凭据：Developer ID 证书及其私钥，以及 App Store
Connect `.p8` 私钥。导入签名证书后，在新电脑重新执行
`notarytool store-credentials`，不要复制钥匙串 profile 文件。确认新配置可用后，
再到 App Store Connect 撤销不再使用的旧 API Key。

### 常见问题

- 找不到签名身份：确认钥匙串内同时存在证书和对应私钥，再运行
  `security find-identity -v -p codesigning`。
- 找不到 `AIMonitorNotary`：重新执行 `notarytool store-credentials`，或设置正确的
  `AIMONITOR_NOTARY_PROFILE`。
- 公证返回 `Invalid`：从构建输出取得 Submission ID，然后执行
  `xcrun notarytool log <SUBMISSION_ID> --keychain-profile AIMonitorNotary` 查看原因。
- Windows 构建提示缺少命令：检查 `cargo-xwin`、`makensis`、`llvm-rc` 是否都在
  `PATH` 中。
- DMG 能签名但仍被 Gatekeeper 拦截：不要绕过安全检查发布；确认
  `stapler validate` 成功且 `spctl` 显示 `Notarized Developer ID` 后重新分发。

## 环境变量

复制 `.env.example` 为 `.env.local`，按需设置 API 地址：

```dotenv
VITE_API_BASE_URL=http://localhost:8080/api
```
