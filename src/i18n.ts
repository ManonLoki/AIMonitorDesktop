import { useEffect, useMemo, useState } from "react";
import { call } from "./lib/tauri";

export type LanguagePreference = "system" | "zh-CN" | "en";
export type ResolvedLocale = "zh-CN" | "en";

const zh = {
  version: "版本 {version}", collapseSidebar: "收起侧边栏", expandSidebar: "展开侧边栏",
  mainNavigation: "主导航", monitor: "监控", settings: "设置", displayMode: "显示模式",
  switchToPet: "切换到桌宠模式", petMode: "桌宠模式", serverRunning: "服务运行中",
  serverStarting: "服务启动中", appVersion: "当前 APP 版本", author: "作者", github: "GitHub 地址",
  system: "系统", language: "语言", languageDescription: "选择界面显示语言",
  languageSystem: "跟随系统", languageChinese: "简体中文", languageEnglish: "English",
  autoStart: "开机自启", autoStartDescription: "登录系统后自动启动 AIMonitorDesktop",
  networkService: "网络服务", listenerStarted: "监听服务已启动", listenerStopped: "监听服务未启动",
  lanAccessible: "同一局域网设备可访问", deviceName: "设备名称", deviceNameHint: "发现设备后显示的名称",
  saveName: "保存名称", ipAddress: "IP 地址", port: "监听端口", discoveryType: "发现类型",
  deviceId: "设备 ID", healthCheck: "健康检查", deviceInfo: "设备信息", initializing: "初始化中",
  notConnectedLan: "未连接局域网",
  monitorGrid: "监控宫格", rows: "行数", columns: "列数", choicesOneToFive: "可选 1–5",
  canvasImage: "画布图片显示", fitDescription: "保持图片比例并完整显示，空余区域显示黑边",
  cropDescription: "保持图片比例并填满格子，超出部分会被裁剪", fit: "等比缩放", crop: "填满裁剪",
  gridAria: "{rows} 行 {columns} 列监控宫格", waitingData: "等待数据", user: "用户",
  petAria: "桌宠，第 {page} 页，共 {pages} 页", previousPage: "上一页", nextPage: "下一页",
  petSettings: "桌宠设置", petSettingsHint: "尺寸按当前显示器自动限制", closePetSettings: "关闭桌宠设置",
  close: "关闭", displayCount: "显示数量", petLayout: "桌宠布局", petSize: "桌宠大小",
  pixels: "{size} 像素", alwaysOnTop: "始终置顶", lockPositionSize: "锁定位置和大小",
  returnMain: "返回主界面", hideToTray: "隐藏到托盘",
} as const;

type MessageKey = keyof typeof zh;
export type TranslationFunction = (key: MessageKey, values?: Record<string, string | number>) => string;

const en: Record<MessageKey, string> = {
  version: "Version {version}", collapseSidebar: "Collapse sidebar", expandSidebar: "Expand sidebar",
  mainNavigation: "Main navigation", monitor: "Monitor", settings: "Settings", displayMode: "Display mode",
  switchToPet: "Switch to desktop pet mode", petMode: "Desktop pet", serverRunning: "Service running",
  serverStarting: "Service starting", appVersion: "Current app version", author: "Author", github: "GitHub",
  system: "System", language: "Language", languageDescription: "Choose the interface language",
  languageSystem: "Use system language", languageChinese: "简体中文", languageEnglish: "English",
  autoStart: "Launch at startup", autoStartDescription: "Start AIMonitorDesktop automatically after sign-in",
  networkService: "Network service", listenerStarted: "Listening service started", listenerStopped: "Listening service stopped",
  lanAccessible: "Available to devices on the same LAN", deviceName: "Device name", deviceNameHint: "Name shown during device discovery",
  saveName: "Save name", ipAddress: "IP address", port: "Listening port", discoveryType: "Discovery type",
  deviceId: "Device ID", healthCheck: "Health check", deviceInfo: "Device info", initializing: "Initializing",
  notConnectedLan: "Not connected to a LAN",
  monitorGrid: "Monitor grid", rows: "Rows", columns: "Columns", choicesOneToFive: "Choose from 1–5",
  canvasImage: "Canvas image display", fitDescription: "Preserve the full image and show black bars in unused space",
  cropDescription: "Fill the tile while preserving aspect ratio and crop any overflow", fit: "Fit", crop: "Fill & crop",
  gridAria: "{rows} row by {columns} column monitor grid", waitingData: "Waiting for data", user: "User",
  petAria: "Desktop pet, page {page} of {pages}", previousPage: "Previous page", nextPage: "Next page",
  petSettings: "Desktop pet settings", petSettingsHint: "Size is limited automatically for the current display", closePetSettings: "Close desktop pet settings",
  close: "Close", displayCount: "Display count", petLayout: "Desktop pet layout", petSize: "Desktop pet size",
  pixels: "{size} pixels", alwaysOnTop: "Always on top", lockPositionSize: "Lock position and size",
  returnMain: "Return to main window", hideToTray: "Hide to tray",
};

function resolveSystemLocale(): ResolvedLocale {
  return navigator.language.toLowerCase().startsWith("zh") ? "zh-CN" : "en";
}

export function useI18n(preference: LanguagePreference) {
  const [systemLocale, setSystemLocale] = useState<ResolvedLocale>(resolveSystemLocale);
  const locale = preference === "system" ? systemLocale : preference;

  useEffect(() => {
    const update = () => setSystemLocale(resolveSystemLocale());
    window.addEventListener("languagechange", update);
    return () => window.removeEventListener("languagechange", update);
  }, []);

  useEffect(() => {
    document.documentElement.lang = locale;
    document.title = "AIMonitorDesktop";
    if ("__TAURI_INTERNALS__" in window) void call("sync_language", { locale });
  }, [locale]);

  const t = useMemo(() => {
    const messages = locale === "zh-CN" ? zh : en;
    return (key: MessageKey, values: Record<string, string | number> = {}) =>
      Object.entries(values).reduce(
        (text, [name, value]) => text.split(`{${name}}`).join(String(value)),
        messages[key] as string,
      );
  }, [locale]);

  return { locale, t };
}
