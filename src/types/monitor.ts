// 监控相关的共享类型与默认值，字段需与 Rust 侧
// src-tauri/src/lib.rs 中的 MonitorState / MonitorTile 保持一致（camelCase 由 serde 自动转换而来）。

// 图片显示模式：
// - FIT_CENTER：保持图片比例完整显示，多余区域留黑边
// - FILL_CROP：保持比例并铺满宫格，超出部分会被裁剪
export type ImageDisplayMode = "FIT_CENTER" | "FILL_CROP";

// 单个监控宫格的数据，由局域网内的客户端通过 POST /api/slots/{slot} 推送。
export interface MonitorTile {
  username: string; // 推送方设置的用户名
  aiName: string; // 推送方设置的 AI 名称
  content: string; // 展示的正文内容
  imageFilename?: string | null; // 已上传图片的文件名，不存在则不展示图片
  updatedAtMillis?: number | null; // 最近一次更新的毫秒时间戳
}

// 桌面端运行时状态整体快照，来自 Tauri 命令 get_monitor_state。
export interface MonitorState {
  rows: number; // 当前宫格行数（1-5）
  columns: number; // 当前宫格列数（1-5）
  imageDisplayMode: ImageDisplayMode; // 全局图片显示模式
  autoStart: boolean; // 是否已启用开机自启
  port: number; // 当前 HTTP 服务实际监听的端口
  appVersion: string; // 应用版本号
  deviceId: string; // 持久化的设备唯一标识
  deviceName: string; // 展示给局域网其他设备的名称
  isServerRunning: boolean; // HTTP 服务器是否已完成绑定并开始监听
  localIp: string; // 当前探测到的局域网 IPv4 地址
  tiles: MonitorTile[]; // 固定长度 25 的宫格数组
}

// 生成一个空宫格，用于初始占位。
export const emptyTile = (): MonitorTile => ({
  username: "", // 空用户名，等待客户端推送
  aiName: "", // 空 AI 名称
  content: "", // 空正文
});

// Tauri 后端尚未完成初始化（或在纯浏览器中预览）时展示的占位状态，
// 避免界面在等待 invoke 返回前出现空白。
export const previewState: MonitorState = {
  rows: 2, // 预览时的默认行数
  columns: 2, // 预览时的默认列数
  imageDisplayMode: "FIT_CENTER", // 预览时的默认图片显示模式
  autoStart: false, // 预览时默认未开启自启
  port: 10241, // 预览时展示的默认端口（与后端 FIRST_HTTP_PORT 一致）
  appVersion: "2.0.0", // 预览时展示的占位版本号
  deviceId: "初始化中", // 真实设备 ID 尚未从后端拉取到时的占位文案
  deviceName: "AIMonitorDesktop", // 预览时的默认设备名
  isServerRunning: false, // 预览时服务尚未启动
  localIp: "未连接局域网", // 预览时的占位 IP 文案
  tiles: Array.from({ length: 25 }, emptyTile), // 生成 25 个空宫格占位
};
