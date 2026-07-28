# AIMonitorDesktop 技术选型

当前技术基线：`2.0.3`（多原生窗口与 Rust 统一窗口状态架构）。

本文件用于固化项目基础技术栈。新增基础设施前，应优先复用下列方案，避免出现职责重叠的库。

| 职责 | 固化选型 | 使用边界 |
| --- | --- | --- |
| 桌面运行时 | Tauri 2 | 原生窗口、系统能力、Rust 命令与应用打包 |
| UI 视图 | React 19 + TypeScript | 组件与视图逻辑 |
| UI 动效 | React Bits + GSAP 3 | 页面入场、滚动呈现与轻量交互反馈；组件源码收敛在项目内 |
| 状态控制通路 | `@tauri-apps/api`（invoke / event） | 前端通过 `invoke` 调用 Rust 命令，通过 `listen("monitor-state-changed", ...)` / `listen("window-state-changed", ...)` 订阅状态变化；图片二进制仍从内置 HTTP 服务的回环地址读取 |
| 局域网 HTTP API | Axum 0.8 + Tokio（`net`/`fs`） + tower-http（CORS） | 面向局域网客户端的 REST API（含槽位、图片和 `/api/clients/{clientId}/heartbeat`）与 UDP 发现；纯异步请求处理，不手写 socket |
| 构建 | Vite 8 | 前端开发服务器与生产构建 |
| 包管理 | pnpm 10 | 使用 `pnpm-lock.yaml` 与精确依赖版本 |

页面内没有独立的服务端状态管理、路由或客户端全局状态库。`main.tsx` 根据原生
窗口 URL 选择 `MonitorApp`、`PetApp` 或 `PetSettingsApp` 三个组合根；它们全部读取
Rust 侧的单一 `Runtime`（见 `src-tauri/src/runtime.rs`）。监控状态通过
`useMonitorState` 拉取与订阅，窗口状态通过 `useWindowState` 拉取与订阅。此前预留的
Mantine、TanStack Router/Query、Axios、Jotai 均未被实际使用，已从依赖中移除；如
未来需要更复杂的路由、服务端状态或表单能力，再按需引入并回填这张表。

## 代码门禁：单文件不超过 400 行

这是本仓库记录在 `CLAUDE.md`（"Code gate: 400-line file limit" 一节）里的事实标准，前后端一并适用：任何 `.ts`/`.tsx`/`.rs` 源码文件都不允许超过 400 行，超出就按职责拆分为更小的模块/文件。`scripts/check-file-length.mjs` 扫描 `src/` 与 `src-tauri/src/` 并接入 `pnpm run check`，超限会直接使命令失败。新增代码前先看是否已有文件快要触顶，需要的话提前拆分，而不是等门禁报错再拆。

## 目录约定

```text
src/
├── components/
│   ├── Icon.tsx          # 内联 SVG 图标集合
│   ├── MonitorCanvas.tsx # 监控画布（宫格渲染）
│   ├── PetContextMenu.tsx # 桌宠设置项（独立设置窗口复用）
│   ├── SettingsPanel.tsx # 设置页
│   └── reactbits/        # React Bits 动效组件（AnimatedContent、SpotlightCard）
├── hooks/
│   ├── useMonitorState.ts # 订阅监控状态
│   └── useWindowState.ts  # 订阅模式、桌宠偏好与动态尺寸边界
├── types/
│   ├── monitor.ts         # MonitorState / MonitorTile 等共享类型
│   └── window.ts          # WindowState / PetWindowPreferences
├── MonitorApp.tsx         # 主窗口组合根（侧边栏 + 工作区）
├── PetApp.tsx             # 透明桌宠窗口组合根
├── PetSettingsApp.tsx     # 独立桌宠设置窗口组合根
├── main.tsx               # 按 view 参数选择组合根
├── pet.css                # 桌宠与桌宠设置窗口样式
└── styles.css             # 主窗口样式
```

后端 `src-tauri/src/` 按职责拆成多个模块（详见 `CLAUDE.md` 的 Architecture 一节），
核心是 HTTP、心跳租约清理、UDP 发现和 mDNS 注册等后台服务共享同一个
`Arc<Runtime>`；其中 HTTP、心跳清理与 UDP 均为纯异步实现（Axum + Tokio，
无手写线程池）：

```text
src-tauri/src/
├── main.rs / lib.rs        # 可执行文件入口 / Tauri 装配与启动
├── constants.rs, model.rs, runtime.rs, commands.rs
├── device_info.rs, image.rs, heartbeat.rs
├── window_geometry.rs      # 主窗口/桌宠几何、DPI 与显示器约束
├── window_manager.rs       # 模式切换、设置窗口与桌宠交互命令实现
├── discovery.rs             # UDP 发现，tokio::net::UdpSocket，纯异步
├── mdns.rs                   # mDNS 注册（mdns-sd 内部自带常驻线程，是该库自身实现）
└── http/                     # 基于 Axum 的纯异步 HTTP 服务器
    ├── mod.rs                # build_router + start_http_server（tokio::net::TcpListener + axum::serve）+ 共享 error_json
    └── {device,images,slots,clients}.rs  # 按资源拆分的处理函数
```

## 约束

1. 前端状态读写统一走 Tauri `invoke`（读写）+ `listen(...)`（变化通知），不引入
   HTTP 客户端传递进程内状态；监控图片按现有规则从
   `http://127.0.0.1:{port}/api/images/{filename}` 读取。
2. Rust 侧暴露给局域网的 HTTP API 需与 Android 版保持兼容；API v3 的槽位 POST
   必须携带 `clientId`，每 30 秒续租，2 分钟超时后按 DELETE 语义清理所属槽位。
3. 优先复用 `src/components/` 下已有组件，避免重复建设。
4. 动效必须尊重 `prefers-reduced-motion`，避免长时间循环或妨碍操作的过场。
5. 升级依赖时同时更新精确版本、pnpm 锁文件和本文件中的主版本说明；引入新基础设施前先确认本表是否已有等价方案。
6. 任何源码文件不超过 400 行，见上方"代码门禁"一节；新增功能导致文件超限时先拆分再继续。
7. 局域网 HTTP API 与 UDP 发现是刻意选择的纯异步实现（Axum + Tokio）；新增接口或网络代码沿用这一模型，不引入手写线程池或阻塞 socket。mDNS 注册（`mdns-sd`）内部自带常驻线程属于第三方库实现细节，不受此约束影响。
8. `main`、`pet`、`pet-settings` 是固定的 Tauri 窗口标签；窗口互斥、恢复、几何约束和偏好落盘统一由 Rust 管理，前端不得分别直接编排原生窗口状态。`pet-settings` 每次显示前必须按 `pet` 的当前显示器工作区重新定位。
