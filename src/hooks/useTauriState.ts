import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useRef, useState } from "react";

// Tauri 读模型的统一订阅器：首次拉取，收到任一依赖事件后再拉取完整 Rust 快照。
export function useTauriState<T>(command: string, eventNames: readonly string[], preview: T) {
  const [state, setState] = useState<T>(preview);
  const latestRequest = useRef(0);

  const refresh = useCallback(async () => {
    const request = ++latestRequest.current;
    try {
      const next = await invoke<T>(command);
      // 多个 Rust 事件可能连续到达；只接受最后发起的查询，避免旧响应覆盖新快照。
      if (request === latestRequest.current) setState(next);
    } catch {
      // 纯浏览器预览没有 Tauri IPC，继续使用显式的预览 fixture。
    }
  }, [command]);

  useEffect(() => {
    void refresh();
    if (!("__TAURI_INTERNALS__" in window)) return;

    let disposed = false;
    const cleanups: Array<() => void> = [];
    for (const eventName of eventNames) {
      void listen(eventName, () => void refresh())
        .then((unlisten) => {
          if (disposed) unlisten();
          else cleanups.push(unlisten);
        })
        .catch(() => undefined);
    }
    return () => {
      disposed = true;
      latestRequest.current += 1; // 让卸载前仍在途的 invoke 响应失效
      cleanups.forEach((cleanup) => cleanup());
    };
  }, [eventNames, refresh]);

  return { state };
}
