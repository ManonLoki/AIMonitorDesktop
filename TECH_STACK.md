# AIMonitorDesktop 技术选型

本文件用于固化项目基础技术栈。新增基础设施前，应优先复用下列方案，避免出现职责重叠的库。

| 职责 | 固化选型 | 使用边界 |
| --- | --- | --- |
| 桌面运行时 | Tauri 2 | 原生窗口、系统能力、Rust 命令与应用打包 |
| UI 视图 | React 19 + TypeScript | 组件与视图逻辑 |
| UI 组件 | Mantine 9 | 布局、表单、反馈、主题与可访问性基础 |
| UI 动效 | React Bits + GSAP 3 | 页面入场、滚动呈现与轻量交互反馈；组件源码收敛在项目内 |
| 路由 | TanStack Router | 页面导航、路由参数与路由级数据协调 |
| 服务端状态 | TanStack Query | 请求缓存、失效、重试与异步状态 |
| HTTP 客户端 | Axios | 请求实例、拦截器、超时与传输层配置 |
| 客户端状态 | Jotai | 跨组件本地状态；不承载服务端缓存 |
| 构建 | Vite 8 | 前端开发服务器与生产构建 |
| 包管理 | pnpm 10 | 使用 `pnpm-lock.yaml` 与精确依赖版本 |

## 目录约定

```text
src/
├── api/           # Axios 实例、请求函数、Query Options
├── components/    # 通用组件与 React Bits 动效组件
├── pages/         # 路由页面
├── state/         # Jotai atoms
├── main.tsx       # 应用 Provider 组合
├── query-client.ts
└── router.tsx
```

## 约束

1. HTTP 请求统一通过 `src/api/client.ts` 的 Axios 实例。
2. 服务端数据统一交给 TanStack Query 管理，不复制进 Jotai。
3. Jotai 仅保存客户端共享状态，例如偏好、当前工作区和 UI 状态。
4. 页面导航统一注册到 TanStack Router。
5. 优先使用 Mantine 组件和主题 token，避免重复建设基础组件。
6. 动效必须尊重 `prefers-reduced-motion`，避免长时间循环或妨碍操作的过场。
7. 升级依赖时同时更新精确版本、pnpm 锁文件和本文件中的主版本说明。
