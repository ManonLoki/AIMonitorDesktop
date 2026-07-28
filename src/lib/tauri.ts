import { invoke } from "@tauri-apps/api/core";

// 触发式命令：不关心返回值，失败时静默忽略（桌宠/设置窗口的交互操作走这条路径）。
export const call = (command: string, args?: Record<string, unknown>) =>
  invoke<void>(command, args).catch(() => undefined);
