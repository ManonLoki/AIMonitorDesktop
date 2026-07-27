import { invoke } from "@tauri-apps/api/core"; // 调用 Tauri 命令
import { listen } from "@tauri-apps/api/event"; // 订阅 Tauri 后端发出的事件
import { useCallback, useEffect, useState } from "react";
import { previewState, type MonitorState } from "../types/monitor";

// 订阅 Rust 侧的监控状态，作为整个前端唯一的数据来源。
// 工作方式：
// 1. 组件挂载时主动调用一次 Tauri 命令 get_monitor_state，拉取当前完整状态；
// 2. 监听 Rust 侧在任意状态变更（宫格更新、设置修改等）后发出的
//    monitor-state-changed 事件，收到即重新拉取一次完整状态。
// 状态本身没有本地写入路径——所有修改都通过 invoke 调用具体的 Tauri 命令，
// 由 Rust 落盘并广播事件后，再经由这里刷新，前端不维护独立的“乐观状态”。
export function useMonitorState() {
  // 初始值用 previewState 占位，避免首次渲染时界面空白
  const [state, setState] = useState<MonitorState>(previewState);

  // 用 useCallback 包一层，保证依赖它的 useEffect 不会因为函数引用变化而重复订阅
  const refresh = useCallback(async () => {
    try {
      const next = await invoke<MonitorState>("get_monitor_state"); // 调用 Rust 命令拉取最新完整状态
      setState(next); // 用最新状态整体替换，不做局部合并
    } catch {
      // 非 Tauri 环境（例如纯浏览器预览）没有该命令，保留预览状态即可。
    }
  }, []);

  useEffect(() => {
    void refresh(); // 组件挂载时立即拉取一次
    // 纯浏览器预览下没有 Tauri 事件总线，跳过订阅避免报错。
    if (!("__TAURI_INTERNALS__" in window)) return;
    let cleanup: (() => void) | undefined; // 保存取消订阅函数，供卸载时调用
    void listen("monitor-state-changed", refresh) // 订阅后端状态变更事件，收到即重新拉取
      .then((unlisten) => {
        cleanup = unlisten; // 订阅成功后记录取消订阅的函数
      })
      .catch(() => undefined); // 订阅失败（理论上不会发生）静默忽略
    return () => cleanup?.(); // 组件卸载时取消订阅，避免内存泄漏
  }, [refresh]);

  return { state, refresh }; // 同时暴露状态和手动刷新方法（设置页调用命令后主动触发）
}
