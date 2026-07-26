import { useState } from "react";
import { AnimatedContent } from "./components/reactbits/AnimatedContent";
import { Icon } from "./components/Icon";
import { MonitorCanvas } from "./components/MonitorCanvas";
import { SettingsPanel } from "./components/SettingsPanel";
import { useMonitorState } from "./hooks/useMonitorState";

// 应用根组件：左侧固定导航栏 + 右侧工作区（监控画布 / 设置页二选一）。
// 全部状态来自 useMonitorState（订阅 Rust 后端），不再引入路由库——
// 页面切换只是一个本地 UI 状态，没有可分享的 URL 语义。
export function MonitorApp() {
  const { state, refresh } = useMonitorState();
  const [destination, setDestination] = useState<"monitor" | "settings">("monitor");
  const [sidebarExpanded, setSidebarExpanded] = useState(false);
  const statusText = `版本 ${state.appVersion}`;

  return (
    <div className="app-shell">
      <aside className={`sidebar${sidebarExpanded ? " expanded" : " collapsed"}`}>
        <div className="brand">
          <img src="/branding/aimonitor-logo.png" alt="AIMonitorDesktop" />
          <div className="brand-copy">
            <strong>AIMonitorDesktop</strong>
            <small>{statusText}</small>
          </div>
        </div>
        <button
          type="button"
          className="sidebar-toggle"
          aria-label={sidebarExpanded ? "收起侧边栏" : "展开侧边栏"}
          aria-expanded={sidebarExpanded}
          title={sidebarExpanded ? "收起侧边栏" : "展开侧边栏"}
          onClick={() => setSidebarExpanded((expanded) => !expanded)}
        >
          <Icon name="sidebar" />
        </button>
        <nav aria-label="主导航">
          <button
            title="监控"
            className={destination === "monitor" ? "active" : ""}
            onClick={() => setDestination("monitor")}
          >
            <Icon name="monitor" />
            <span>监控</span>
          </button>
          <button
            title="设置"
            className={destination === "settings" ? "active" : ""}
            onClick={() => setDestination("settings")}
          >
            <Icon name="settings" />
            <span>设置</span>
          </button>
        </nav>
        <div className="server-indicator">
          <i className={state.isServerRunning ? "online" : ""} />
          <span>{state.isServerRunning ? "服务运行中" : "服务启动中"}</span>
        </div>
      </aside>
      <div className="workspace">
        {/* key={destination} 让切换目的地时强制重新挂载，从而重播入场动画 */}
        <AnimatedContent
          className="page-motion"
          distance={destination === "monitor" ? -14 : 14}
          duration={0.42}
          key={destination}
        >
          {destination === "monitor" ? (
            <MonitorCanvas state={state} />
          ) : (
            <SettingsPanel state={state} onRefresh={refresh} />
          )}
        </AnimatedContent>
      </div>
    </div>
  );
}
