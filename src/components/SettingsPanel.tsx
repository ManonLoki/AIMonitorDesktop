import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { AnimatedContent } from "./reactbits/AnimatedContent";
import { SpotlightCard } from "./reactbits/SpotlightCard";
import { Icon } from "./Icon";
import type { MonitorState } from "../types/monitor";

// 只读的一行"标签 - 值"展示，value 同时作为 title 属性，便于过长内容悬浮查看完整值。
function SettingRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="setting-value">
      <span>{label}</span>
      <strong title={value}>{value}</strong>
    </div>
  );
}

// 1-5 的数字选择器，用于行数/列数设置。
function ChoiceRow({
  label,
  value,
  onChange,
}: {
  label: string;
  value: number;
  onChange: (value: number) => void;
}) {
  return (
    <div className="choice-row">
      <div>
        <strong>{label}</strong>
        <small>可选 1–5</small>
      </div>
      <div className="number-choices">
        {[1, 2, 3, 4, 5].map((option) => (
          <button
            className={option === value ? "selected" : ""}
            onClick={() => onChange(option)}
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
// 所有修改都通过 invoke 调用 Rust 侧 Tauri 命令，命令执行成功后立即调用 onRefresh
// 重新拉取一次完整状态，而不是在前端本地乐观更新——保证展示的始终是后端落盘后的真实值。
export function SettingsPanel({
  state,
  onRefresh,
}: {
  state: MonitorState;
  onRefresh: () => Promise<void>;
}) {
  // 设备名称使用本地受控输入，允许用户编辑到未保存状态，
  // 仅在后端状态变化时（例如首次加载）用后端值覆盖本地草稿。
  const [deviceName, setDeviceName] = useState(state.deviceName);
  useEffect(() => setDeviceName(state.deviceName), [state.deviceName]);

  const call = async (command: string, args: Record<string, unknown>) => {
    await invoke(command, args);
    await onRefresh();
  };

  const address = state.localIp;

  return (
    <main className="settings-page" id="settings-scroll-container">
      <div className="settings-content">
        <AnimatedContent container="#settings-scroll-container" delay={0.04} distance={16}>
          <SpotlightCard className="version-card">
            <SettingRow label="当前 APP 版本" value={state.appVersion} />
          </SpotlightCard>
        </AnimatedContent>

        <AnimatedContent container="#settings-scroll-container" delay={0.08} distance={20}>
          <h2>系统</h2>
          <SpotlightCard
            className="settings-card toggle-row"
            onClick={() => void call("set_auto_start", { enabled: !state.autoStart })}
          >
            <div>
              <strong>开机自启</strong>
              <small>登录系统后自动启动 AIMonitorDesktop</small>
            </div>
            <button role="switch" aria-checked={state.autoStart} className={`switch${state.autoStart ? " on" : ""}`}>
              <span />
            </button>
          </SpotlightCard>
        </AnimatedContent>

        <AnimatedContent container="#settings-scroll-container" delay={0.12} distance={22}>
          <h2>网络服务</h2>
          <SpotlightCard className="settings-card network-card">
            <div className="network-status">
              <div className="icon-box">
                <Icon name="wifi" />
              </div>
              <div>
                <strong>{state.isServerRunning ? "监听服务已启动" : "监听服务未启动"}</strong>
                <small>同一局域网设备可访问</small>
              </div>
            </div>
            <div className="divider" />
            <label className="input-field">
              <span>设备名称</span>
              <input
                value={deviceName}
                maxLength={40}
                onChange={(event) => setDeviceName(event.target.value)}
              />
              <small>发现设备后显示的名称</small>
            </label>
            <div className="save-row">
              <button
                className="primary-button"
                disabled={!deviceName.trim() || deviceName.trim() === state.deviceName}
                onClick={() => void call("set_device_name", { name: deviceName })}
              >
                保存名称
              </button>
            </div>
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
            <ChoiceRow
              label="行数"
              value={state.rows}
              onChange={(rows) => void call("set_grid", { rows, columns: state.columns })}
            />
            <ChoiceRow
              label="列数"
              value={state.columns}
              onChange={(columns) => void call("set_grid", { rows: state.rows, columns })}
            />
            <div className="divider" />
            <div className="display-mode">
              <div>
                <strong>画布图片显示</strong>
                <small>
                  {state.imageDisplayMode === "FIT_CENTER"
                    ? "保持图片比例并完整显示，空余区域显示黑边"
                    : "保持图片比例并填满格子，超出部分会被裁剪"}
                </small>
              </div>
              <div className="mode-choices">
                <button
                  className={state.imageDisplayMode === "FIT_CENTER" ? "selected" : ""}
                  onClick={() => void call("set_image_display_mode", { mode: "FIT_CENTER" })}
                >
                  等比缩放
                </button>
                <button
                  className={state.imageDisplayMode === "FILL_CROP" ? "selected" : ""}
                  onClick={() => void call("set_image_display_mode", { mode: "FILL_CROP" })}
                >
                  填满裁剪
                </button>
              </div>
            </div>
          </SpotlightCard>
        </AnimatedContent>
      </div>
    </main>
  );
}
