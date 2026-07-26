import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useState } from "react";
import { AnimatedContent } from "./components/reactbits/AnimatedContent";
import { SpotlightCard } from "./components/reactbits/SpotlightCard";

type ImageDisplayMode = "FIT_CENTER" | "FILL_CROP";

interface MonitorTile {
  username: string;
  aiName: string;
  content: string;
  imageFilename?: string | null;
  updatedAtMillis?: number | null;
}

interface MonitorState {
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

const emptyTile = (): MonitorTile => ({ username: "", aiName: "", content: "" });
const previewState: MonitorState = {
  rows: 2,
  columns: 2,
  imageDisplayMode: "FIT_CENTER",
  autoStart: false,
  port: 10241,
  appVersion: "1.0.1",
  deviceId: "初始化中",
  deviceName: "AIMonitorDesktop",
  isServerRunning: false,
  localIp: "未连接局域网",
  tiles: Array.from({ length: 25 }, emptyTile),
};

function Icon({ name }: { name: "monitor" | "settings" | "wifi" | "image" | "sidebar" }) {
  const paths = {
    monitor: <><rect x="3" y="4" width="18" height="14" rx="2"/><path d="M8 22h8M12 18v4"/></>,
    settings: <><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .34 1.88l.06.06-2.83 2.83-.06-.06a1.7 1.7 0 0 0-1.88-.34 1.7 1.7 0 0 0-1.03 1.56V21h-4v-.08A1.7 1.7 0 0 0 9 19.37a1.7 1.7 0 0 0-1.88.34l-.06.06-2.83-2.83.06-.06A1.7 1.7 0 0 0 4.63 15 1.7 1.7 0 0 0 3.08 14H3v-4h.08A1.7 1.7 0 0 0 4.63 9a1.7 1.7 0 0 0-.34-1.88l-.06-.06 2.83-2.83.06.06A1.7 1.7 0 0 0 9 4.63 1.7 1.7 0 0 0 10 3.08V3h4v.08A1.7 1.7 0 0 0 15 4.63a1.7 1.7 0 0 0 1.88-.34l.06-.06 2.83 2.83-.06.06A1.7 1.7 0 0 0 19.37 9 1.7 1.7 0 0 0 20.92 10H21v4h-.08A1.7 1.7 0 0 0 19.4 15Z"/></>,
    wifi: <><path d="M5 12.55a11 11 0 0 1 14 0M8.5 16a6 6 0 0 1 7 0M12 20h.01M2 9a16 16 0 0 1 20 0"/></>,
    image: <><rect x="3" y="3" width="18" height="18" rx="2"/><circle cx="8.5" cy="8.5" r="1.5"/><path d="m21 15-5-5L5 21"/></>,
    sidebar: <><rect x="3" y="4" width="18" height="16" rx="2"/><path d="M9 4v16"/></>,
  };
  return <svg viewBox="0 0 24 24" aria-hidden="true">{paths[name]}</svg>;
}

function useMonitorState() {
  const [state, setState] = useState<MonitorState>(previewState);
  const refresh = useCallback(async () => {
    try {
      const next = await invoke<MonitorState>("get_monitor_state");
      setState(next);
    } catch { /* 保留预览状态，等待桌面端完成初始化 */ }
  }, []);
  useEffect(() => {
    void refresh();
    if (!("__TAURI_INTERNALS__" in window)) return;
    let cleanup: (() => void) | undefined;
    void listen("monitor-state-changed", refresh)
      .then((unlisten) => { cleanup = unlisten; })
      .catch(() => undefined);
    return () => cleanup?.();
  }, [refresh]);
  return { state, refresh };
}

function MonitorCanvas({ state }: { state: MonitorState }) {
  const visible = state.tiles.slice(0, state.rows * state.columns);
  const compact = Math.max(state.rows, state.columns) >= 4;
  return (
    <main
      className={`monitor-grid${compact ? " compact" : ""}`}
      style={{ gridTemplateColumns: `repeat(${state.columns}, minmax(0, 1fr))`, gridTemplateRows: `repeat(${state.rows}, minmax(0, 1fr))` }}
      aria-label={`${state.rows} 行 ${state.columns} 列监控宫格`}
    >
      {visible.map((tile, index) => {
        const imageUrl = tile.imageFilename
          ? `http://127.0.0.1:${state.port}/api/images/${encodeURIComponent(tile.imageFilename)}`
          : undefined;
        const aiName = tile.aiName.trim() || "AI";
        const username = tile.username.trim() || "等待数据";
        return (
          <AnimatedContent
            className="tile-motion"
            delay={Math.min(index * 0.045, 0.35)}
            distance={18}
            duration={0.48}
            scale={0.97}
            key={index}
          >
            <section className="monitor-tile">
              {imageUrl && <img src={imageUrl} alt={`${index + 1}-${aiName}-${username}`} className={state.imageDisplayMode === "FILL_CROP" ? "cover" : "contain"} />}
              <div className="tile-header">{String(index + 1).padStart(2, "0")}-{aiName}-{username}</div>
              {!imageUrl && <div className="empty-state"><Icon name="image" />{!compact && <span>等待数据</span>}</div>}
              {tile.content && <div className="tile-caption"><span>{tile.content}</span>{tile.updatedAtMillis && <time>{new Date(tile.updatedAtMillis).toLocaleTimeString("zh-CN", { hour12: false })}</time>}</div>}
            </section>
          </AnimatedContent>
        );
      })}
    </main>
  );
}

function SettingRow({ label, value }: { label: string; value: string }) {
  return <div className="setting-value"><span>{label}</span><strong title={value}>{value}</strong></div>;
}

function ChoiceRow({ label, value, onChange }: { label: string; value: number; onChange: (value: number) => void }) {
  return <div className="choice-row"><div><strong>{label}</strong><small>可选 1–5</small></div><div className="number-choices">{[1, 2, 3, 4, 5].map((option) => <button className={option === value ? "selected" : ""} onClick={() => onChange(option)} key={option}>{option}</button>)}</div></div>;
}

function SettingsPanel({ state, onRefresh }: { state: MonitorState; onRefresh: () => Promise<void> }) {
  const [deviceName, setDeviceName] = useState(state.deviceName);
  useEffect(() => setDeviceName(state.deviceName), [state.deviceName]);
  const call = async (command: string, args: Record<string, unknown>) => {
    await invoke(command, args);
    await onRefresh();
  };
  const address = state.localIp === "未连接局域网" ? state.localIp : state.localIp;
  return <main className="settings-page" id="settings-scroll-container">
    <div className="settings-content">
      <AnimatedContent container="#settings-scroll-container" delay={0.04} distance={16}>
        <SpotlightCard className="version-card"><SettingRow label="当前 APP 版本" value={state.appVersion} /></SpotlightCard>
      </AnimatedContent>

      <AnimatedContent container="#settings-scroll-container" delay={0.08} distance={20}>
        <h2>系统</h2>
        <SpotlightCard className="settings-card toggle-row" onClick={() => void call("set_auto_start", { enabled: !state.autoStart })}>
          <div><strong>开机自启</strong><small>登录系统后自动启动 AIMonitorDesktop</small></div>
          <button role="switch" aria-checked={state.autoStart} className={`switch${state.autoStart ? " on" : ""}`}><span /></button>
        </SpotlightCard>
      </AnimatedContent>

      <AnimatedContent container="#settings-scroll-container" delay={0.12} distance={22}>
        <h2>网络服务</h2>
        <SpotlightCard className="settings-card network-card">
          <div className="network-status"><div className="icon-box"><Icon name="wifi" /></div><div><strong>{state.isServerRunning ? "监听服务已启动" : "监听服务未启动"}</strong><small>同一局域网设备可访问</small></div></div>
          <div className="divider" />
          <label className="input-field"><span>设备名称</span><input value={deviceName} maxLength={40} onChange={(event) => setDeviceName(event.target.value)} /><small>发现设备后显示的名称</small></label>
          <div className="save-row"><button className="primary-button" disabled={!deviceName.trim() || deviceName.trim() === state.deviceName} onClick={() => void call("set_device_name", { name: deviceName })}>保存名称</button></div>
          <div className="divider" />
          <SettingRow label="IP 地址" value={address} />
          <SettingRow label="监听端口" value={String(state.port)} />
          <SettingRow label="发现类型" value="_aimonitor._tcp." />
          <SettingRow label="设备 ID" value={state.deviceId || "初始化中"} />
          <SettingRow label="健康检查" value={`http://${address}:${state.port}/health`} />
          <SettingRow label="设备信息" value={`http://${address}:${state.port}/api/device`} />
        </SpotlightCard>
      </AnimatedContent>

      <AnimatedContent container="#settings-scroll-container" delay={0.08} distance={22}>
        <h2>监控宫格</h2>
        <SpotlightCard className="settings-card grid-card">
          <ChoiceRow label="行数" value={state.rows} onChange={(rows) => void call("set_grid", { rows, columns: state.columns })} />
          <ChoiceRow label="列数" value={state.columns} onChange={(columns) => void call("set_grid", { rows: state.rows, columns })} />
          <div className="divider" />
          <div className="display-mode"><div><strong>画布图片显示</strong><small>{state.imageDisplayMode === "FIT_CENTER" ? "保持图片比例并完整显示，空余区域显示黑边" : "保持图片比例并填满格子，超出部分会被裁剪"}</small></div><div className="mode-choices"><button className={state.imageDisplayMode === "FIT_CENTER" ? "selected" : ""} onClick={() => void call("set_image_display_mode", { mode: "FIT_CENTER" })}>等比缩放</button><button className={state.imageDisplayMode === "FILL_CROP" ? "selected" : ""} onClick={() => void call("set_image_display_mode", { mode: "FILL_CROP" })}>填满裁剪</button></div></div>
        </SpotlightCard>
      </AnimatedContent>
    </div>
  </main>;
}

export function MonitorApp() {
  const { state, refresh } = useMonitorState();
  const [destination, setDestination] = useState<"monitor" | "settings">("monitor");
  const [sidebarExpanded, setSidebarExpanded] = useState(false);
  const statusText = `版本 ${state.appVersion}`;
  return <div className="app-shell">
    <aside className={`sidebar${sidebarExpanded ? " expanded" : " collapsed"}`}>
      <div className="brand"><img src="/branding/aimonitor-logo.png" alt="AIMonitorDesktop" /><div className="brand-copy"><strong>AIMonitorDesktop</strong><small>{statusText}</small></div></div>
      <button
        type="button"
        className="sidebar-toggle"
        aria-label={sidebarExpanded ? "收起侧边栏" : "展开侧边栏"}
        aria-expanded={sidebarExpanded}
        title={sidebarExpanded ? "收起侧边栏" : "展开侧边栏"}
        onClick={() => setSidebarExpanded((expanded) => !expanded)}
      ><Icon name="sidebar" /></button>
      <nav aria-label="主导航">
        <button title="监控" className={destination === "monitor" ? "active" : ""} onClick={() => setDestination("monitor")}><Icon name="monitor" /><span>监控</span></button>
        <button title="设置" className={destination === "settings" ? "active" : ""} onClick={() => setDestination("settings")}><Icon name="settings" /><span>设置</span></button>
      </nav>
      <div className="server-indicator"><i className={state.isServerRunning ? "online" : ""} /><span>{state.isServerRunning ? "服务运行中" : "服务启动中"}</span></div>
    </aside>
    <div className="workspace">
      <AnimatedContent className="page-motion" distance={destination === "monitor" ? -14 : 14} duration={0.42} key={destination}>
        {destination === "monitor" ? <MonitorCanvas state={state} /> : <SettingsPanel state={state} onRefresh={refresh} />}
      </AnimatedContent>
    </div>
  </div>;
}
