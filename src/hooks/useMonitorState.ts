import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
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
  const [state, setState] = useState<MonitorState>(previewState);

  const refresh = useCallback(async () => {
    try {
      const next = await invoke<MonitorState>("get_monitor_state");
      setState(next);
    } catch {
      // 非 Tauri 环境（例如纯浏览器预览）没有该命令，保留预览状态即可。
    }
  }, []);

  useEffect(() => {
    void refresh();
    // 纯浏览器预览下没有 Tauri 事件总线，跳过订阅避免报错。
    if (!("__TAURI_INTERNALS__" in window)) return;
    let cleanup: (() => void) | undefined;
    void listen("monitor-state-changed", refresh)
      .then((unlisten) => {
        cleanup = unlisten;
      })
      .catch(() => undefined);
    return () => cleanup?.();
  }, [refresh]);

  return { state, refresh };
}
