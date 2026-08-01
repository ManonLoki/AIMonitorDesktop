import { useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { AnimatedContent } from "./reactbits/AnimatedContent";
import { SpotlightCard } from "./reactbits/SpotlightCard";
import { Icon } from "./Icon";
import type { MonitorState } from "../types/monitor";
import type { TranslationFunction } from "../i18n";
import { call } from "../lib/tauri";

// 只读的一行"标签 - 值"展示，value 同时作为 title 属性，便于过长内容悬浮查看完整值。
function SettingRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="setting-value">
      <span>{label}</span>
      <strong title={value}>{value}</strong>
    </div>
  );
}

// 可点击的外部链接行，通过 Tauri opener 交给系统默认浏览器打开。
function ExternalLinkRow({ label, url }: { label: string; url: string }) {
  return (
    <div className="setting-value">
      <span>{label}</span>
      <button className="setting-link" title={url} onClick={() => void openUrl(url)}>
        {url}
      </button>
    </div>
  );
}

// 1-5 的数字选择器，用于行数/列数设置。
function ChoiceRow({
  label,
  value,
  onChange,
  hint,
}: {
  label: string;
  value: number;
  onChange: (value: number) => void;
  hint: string;
}) {
  return (
    <div className="choice-row">
      <div>
        <strong>{label}</strong>
        <small>{hint}</small>
      </div>
      <div className="number-choices">
        {/* 固定渲染 1-5 五个按钮，当前值对应的按钮加 selected 样式 */}
        {[1, 2, 3, 4, 5].map((option) => (
          <button
            className={option === value ? "selected" : ""}
            onClick={() => onChange(option)} // 点击即回调父组件发起后端更新
            key={option}
          >
            {option}
          </button>
        ))}
      </div>
    </div>
  );
}

// 设置页：展示版本、开机自启、网络服务信息，以及宫格行列/图片显示模式的调整入口。
// 所有修改都只向 Rust 表达意图；Rust 落盘并广播事件后，订阅 hook 会拉取权威快照。
export function SettingsPanel({
  state,
  t,
}: {
  state: MonitorState;
  t: TranslationFunction;
}) {
  // 设备名称使用本地受控输入，允许用户编辑到未保存状态，
  // 仅在后端状态变化时（例如首次加载）用后端值覆盖本地草稿。
  const [deviceName, setDeviceName] = useState(state.deviceName); // 本地输入框草稿值
  useEffect(() => setDeviceName(state.deviceName), [state.deviceName]); // 后端设备名变化时同步覆盖草稿

  const address = state.localIp || t("notConnectedLan"); // 局域网 IP，下方多处 URL 展示都会用到

  return (
    <main className="settings-page" id="settings-scroll-container">
      <div className="settings-content">
        {/* 应用信息卡片 */}
        <AnimatedContent container="#settings-scroll-container" delay={0.04} distance={16}>
          <SpotlightCard className="version-card">
            <SettingRow label={t("appVersion")} value={state.appVersion} />
            <SettingRow label={t("author")} value="ManonLoki" />
            <ExternalLinkRow label={t("github")} url="https://github.com/ManonLoki/AIMonitorDesktop" />
          </SpotlightCard>
        </AnimatedContent>

        {/* 系统设置：开机自启开关 */}
        <AnimatedContent container="#settings-scroll-container" delay={0.08} distance={20}>
          <h2>{t("system")}</h2>
          <SpotlightCard className="settings-card language-row">
            <div>
              <strong>{t("language")}</strong>
              <small>{t("languageDescription")}</small>
            </div>
            <select
              aria-label={t("language")}
              value={state.language}
              onChange={(event) => void call("set_language", {
                language: event.currentTarget.value as MonitorState["language"],
              })}
            >
              <option value="system">{t("languageSystem")}</option>
              <option value="zh-CN">{t("languageChinese")}</option>
              <option value="en">{t("languageEnglish")}</option>
            </select>
          </SpotlightCard>
          <SpotlightCard
            className="settings-card toggle-row"
            onClick={() => void call("set_auto_start", { enabled: !state.autoStart })} // 点击整行即可切换开关状态（取反当前值）
          >
            <div>
              <strong>{t("autoStart")}</strong>
              <small>{t("autoStartDescription")}</small>
            </div>
            <button role="switch" aria-checked={state.autoStart} className={`switch${state.autoStart ? " on" : ""}`}>
              <span />
            </button>
          </SpotlightCard>
        </AnimatedContent>

        {/* 网络服务：服务状态、设备名称编辑、连接信息展示 */}
        <AnimatedContent container="#settings-scroll-container" delay={0.12} distance={22}>
          <h2>{t("networkService")}</h2>
          <SpotlightCard className="settings-card network-card">
            <div className="network-status">
              <div className="icon-box">
                <Icon name="wifi" />
              </div>
              <div>
                <strong>{state.isServerRunning ? t("listenerStarted") : t("listenerStopped")}</strong>
                <small>{t("lanAccessible")}</small>
              </div>
            </div>
            <div className="divider" />
            <label className="input-field">
              <span>{t("deviceName")}</span>
              <input
                value={deviceName}
                maxLength={40} // 输入体验的即时限制；最终清洗和校验仍以 Rust 为准
                onChange={(event) => setDeviceName(event.target.value)} // 仅更新本地草稿，不立即提交
              />
              <small>{t("deviceNameHint")}</small>
            </label>
            <div className="save-row">
              <button
                className="primary-button"
                disabled={!deviceName.trim() || deviceName.trim() === state.deviceName} // 空白或未修改时禁用保存按钮
                onClick={() => void call("set_device_name", { name: deviceName })} // 提交草稿到后端
              >
                {t("saveName")}
              </button>
            </div>
            <div className="divider" />
            <SettingRow label={t("ipAddress")} value={address} />
            <SettingRow label={t("port")} value={String(state.port)} />
            <SettingRow label={t("discoveryType")} value="_aimonitor._tcp." />
            <SettingRow label={t("deviceId")} value={state.deviceId || t("initializing")} />
            <SettingRow label={t("healthCheck")} value={`http://${address}:${state.port}/health`} />
            <SettingRow label={t("deviceInfo")} value={`http://${address}:${state.port}/api/device`} />
          </SpotlightCard>
        </AnimatedContent>

        {/* 监控宫格设置：行列数、图片显示模式 */}
        <AnimatedContent container="#settings-scroll-container" delay={0.08} distance={22}>
          <h2>{t("monitorGrid")}</h2>
          <SpotlightCard className="settings-card grid-card">
            <ChoiceRow
              label={t("rows")}
              hint={t("choicesOneToFive")}
              value={state.rows}
              onChange={(rows) => void call("set_grid_rows", { rows })}
            />
            <ChoiceRow
              label={t("columns")}
              hint={t("choicesOneToFive")}
              value={state.columns}
              onChange={(columns) => void call("set_grid_columns", { columns })}
            />
            <div className="divider" />
            <div className="display-mode">
              <div>
                <strong>{t("canvasImage")}</strong>
                <small>
                  {state.imageDisplayMode === "FIT_CENTER"
                    ? t("fitDescription")
                    : t("cropDescription")}
                </small>
              </div>
              <div className="mode-choices">
                <button
                  className={state.imageDisplayMode === "FIT_CENTER" ? "selected" : ""}
                  onClick={() => void call("set_image_display_mode", { mode: "FIT_CENTER" })}
                >
                  {t("fit")}
                </button>
                <button
                  className={state.imageDisplayMode === "FILL_CROP" ? "selected" : ""}
                  onClick={() => void call("set_image_display_mode", { mode: "FILL_CROP" })}
                >
                  {t("crop")}
                </button>
              </div>
            </div>
          </SpotlightCard>
        </AnimatedContent>
      </div>
    </main>
  );
}
