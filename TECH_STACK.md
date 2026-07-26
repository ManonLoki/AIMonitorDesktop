# AIMonitorDesktop 技术选型

本文件用于固化项目基础技术栈。新增基础设施前，应优先复用下列方案，避免出现职责重叠的库。

| 职责 | 固化选型 | 使用边界 |
| --- | --- | --- |
| 桌面运行时 | Tauri 2 | 原生窗口、系统能力、Rust 命令与应用打包 |
| UI 视图 | React 19 + TypeScript | 组件与视图逻辑 |
| UI 动效 | React Bits + GSAP 3 | 页面入场、滚动呈现与轻量交互反馈；组件源码收敛在项目内 |
| 前后端通信 | `@tauri-apps/api`（invoke / event） | 前端通过 `invoke` 调用 Rust 命令，通过 `listen("monitor-state-changed", ...)` 订阅状态变化；不经过 HTTP |
| 构建 | Vite 8 | 前端开发服务器与生产构建 |
| 包管理 | pnpm 10 | 使用 `pnpm-lock.yaml` 与精确依赖版本 |

页面内没有独立的服务端状态管理、路由或客户端全局状态库：整个应用只有"监控画布"与"设置"两个本地 UI 切换态，全部数据来自 Rust 侧的单一 `Runtime`（见 `src-tauri/src/lib.rs`），通过 `useMonitorState`（`src/hooks/useMonitorState.ts`）拉取与订阅。此前预留的 Mantine、TanStack Router/Query、Axios、Jotai 均未被实际使用，已从依赖中移除；如未来需要更复杂的路由/服务端状态/表单能力，再按需引入并回填这张表。

## 目录约定

```text
src/
├── components/
│   ├── Icon.tsx          # 内联 SVG 图标集合
│   ├── MonitorCanvas.tsx # 监控画布（宫格渲染）
│   ├── SettingsPanel.tsx # 设置页
│   └── reactbits/        # React Bits 动效组件（AnimatedContent、SpotlightCard）
├── hooks/
│   └── useMonitorState.ts # 订阅 Rust 侧状态的唯一数据源
├── types/
│   └── monitor.ts        # MonitorState / MonitorTile 等共享类型
├── MonitorApp.tsx         # 应用根组件（侧边栏 + 工作区）
├── main.tsx                # 应用入口
└── styles.css
```

后端只有一个文件 `src-tauri/src/lib.rs`：状态管理、手写 HTTP 服务器（Android 兼容 API）、UDP 发现、mDNS 注册都在其中，共享同一个 `Arc<Runtime>`。

## 约束

1. 前端与后端的数据通路统一走 Tauri `invoke`（写）+ `listen("monitor-state-changed")`（读），不引入 HTTP 客户端做进程内通信。
2. Rust 侧暴露给局域网的 HTTP API（`/health`、`/api/device`、`/api/config`、`/api/images`、`/api/slots/{slot}`）需与 Android 版保持兼容，改动前核对字段与状态码。
3. 优先复用 `src/components/` 下已有组件，避免重复建设。
4. 动效必须尊重 `prefers-reduced-motion`，避免长时间循环或妨碍操作的过场。
5. 升级依赖时同时更新精确版本、pnpm 锁文件和本文件中的主版本说明；引入新基础设施前先确认本表是否已有等价方案。
