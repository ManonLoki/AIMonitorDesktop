import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import { AnimatedContent } from "./components/reactbits/AnimatedContent";
import { Icon } from "./components/Icon";
import { MonitorCanvas } from "./components/MonitorCanvas";
import { SettingsPanel } from "./components/SettingsPanel";
import { useMonitorState } from "./hooks/useMonitorState";
import { useI18n } from "./i18n";

// 应用根组件：左侧固定导航栏 + 右侧工作区（监控画布 / 设置页二选一）。
// 全部状态来自 useMonitorState（订阅 Rust 后端），不再引入路由库——
// 页面切换只是一个本地 UI 状态，没有可分享的 URL 语义。
export function MonitorApp() {
  const { state, refresh } = useMonitorState(); // 唯一的数据来源：Rust 后端状态 + 手动刷新方法
  const [destination, setDestination] = useState<"monitor" | "settings">("monitor"); // 当前展示的工作区页面
  const [sidebarExpanded, setSidebarExpanded] = useState(false); // 侧边栏是否展开（默认折叠，仅显示图标）
  const { t } = useI18n(state.language);
  const statusText = t("version", { version: state.appVersion }); // 侧边栏品牌区展示的版本号文案

  return (
    <div className="app-shell">
      {/* 侧边栏，class 里叠加 expanded/collapsed 控制展开/折叠样式 */}
      <aside className={`sidebar${sidebarExpanded ? " expanded" : " collapsed"}`}>
        <div className="brand">
          {/* 应用 Logo 与名称 */}
          <img src="/branding/aimonitor-logo.png" alt="AIMonitorDesktop" />
          <div className="brand-copy">
            <strong>AIMonitorDesktop</strong>
            <small>{statusText}</small>
          </div>
        </div>
        <button
          type="button"
          className="sidebar-toggle"
          aria-label={sidebarExpanded ? t("collapseSidebar") : t("expandSidebar")} // 无障碍标签随展开状态变化
          aria-expanded={sidebarExpanded}
          title={sidebarExpanded ? t("collapseSidebar") : t("expandSidebar")}
          onClick={() => setSidebarExpanded((expanded) => !expanded)} // 点击切换展开/折叠
        >
          <Icon name="sidebar" />
        </button>
        <nav aria-label={t("mainNavigation")}>
          {/* “监控”导航项：点击切换到监控画布页 */}
          <button
            title={t("monitor")}
            className={destination === "monitor" ? "active" : ""}
            onClick={() => setDestination("monitor")}
          >
            <Icon name="monitor" />
            <span>{t("monitor")}</span>
          </button>
          {/* “设置”导航项：点击切换到设置页 */}
          <button
            title={t("settings")}
            className={destination === "settings" ? "active" : ""}
            onClick={() => setDestination("settings")}
          >
            <Icon name="settings" />
            <span>{t("settings")}</span>
          </button>
        </nav>
        <div className="mode-switch">
          <small>{t("displayMode")}</small>
          <button title={t("switchToPet")} onClick={() => void invoke("switch_app_mode", { mode: "pet" })}>
            <Icon name="pet" />
            <span>{t("petMode")}</span>
          </button>
        </div>
        <div className="server-indicator">
          {/* HTTP 服务是否已完成绑定，绿点/文案随之切换 */}
          <i className={state.isServerRunning ? "online" : ""} />
          <span>{state.isServerRunning ? t("serverRunning") : t("serverStarting")}</span>
        </div>
      </aside>
      <div className="workspace">
        {/* key={destination} 让切换目的地时强制重新挂载，从而重播入场动画 */}
        <AnimatedContent
          className="page-motion"
          distance={destination === "monitor" ? -14 : 14} // 两个页面从相反方向滑入，增强切换方向感
          duration={0.42}
          key={destination}
        >
          {destination === "monitor" ? (
            <MonitorCanvas state={state} t={t} /> // 监控画布：渲染宫格
          ) : (
            <SettingsPanel state={state} onRefresh={refresh} t={t} /> // 设置页：修改配置后调用 refresh 拉取最新状态
          )}
        </AnimatedContent>
      </div>
    </div>
  );
}
