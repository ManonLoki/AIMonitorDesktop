// 监控相关的共享类型与默认值，字段需与 Rust 侧
// src-tauri/src/lib.rs 中的 MonitorState / MonitorTile 保持一致（camelCase 由 serde 自动转换而来）。

// 图片显示模式：
// - FIT_CENTER：保持图片比例完整显示，多余区域留黑边
// - FILL_CROP：保持比例并铺满宫格，超出部分会被裁剪
export type ImageDisplayMode = "FIT_CENTER" | "FILL_CROP";

// 单个监控宫格的数据，由局域网内的客户端通过 POST /api/slots/{slot} 推送。
export interface MonitorTile {
  username: string;
  aiName: string;
  content: string;
  imageFilename?: string | null;
  updatedAtMillis?: number | null;
}

// 桌面端运行时状态整体快照，来自 Tauri 命令 get_monitor_state。
export interface MonitorState {
  rows: number;
  columns: number;
  imageDisplayMode: ImageDisplayMode;
  autoStart: boolean;
  port: number;
  appVersion: string;
  deviceId: string;
  deviceName: string;
  isServerRunning: boolean;
  localIp: string;
  tiles: MonitorTile[];
}

// 生成一个空宫格，用于初始占位。
export const emptyTile = (): MonitorTile => ({
  username: "",
  aiName: "",
  content: "",
});

// Tauri 后端尚未完成初始化（或在纯浏览器中预览）时展示的占位状态，
// 避免界面在等待 invoke 返回前出现空白。
export const previewState: MonitorState = {
  rows: 2,
  columns: 2,
  imageDisplayMode: "FIT_CENTER",
  autoStart: false,
  port: 10241,
  appVersion: "1.0.2",
  deviceId: "初始化中",
  deviceName: "AIMonitorDesktop",
  isServerRunning: false,
  localIp: "未连接局域网",
  tiles: Array.from({ length: 25 }, emptyTile),
};
