import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useState } from "react";
import { previewWindowState, type WindowState } from "../types/window";

export function useWindowState() {
  const [state, setState] = useState<WindowState>(previewWindowState);

  const refresh = useCallback(async () => {
    try {
      setState(await invoke<WindowState>("get_window_state"));
    } catch {
      // 浏览器预览没有 Tauri 命令，保留可交互的默认状态。
    }
  }, []);

  useEffect(() => {
    void refresh();
    if (!("__TAURI_INTERNALS__" in window)) return;
    let cleanup: (() => void) | undefined;
    void listen("window-state-changed", refresh).then((unlisten) => {
      cleanup = unlisten;
    });
    return () => cleanup?.();
  }, [refresh]);

  return { state, refresh };
}
