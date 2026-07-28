import { PetContextMenu } from "./components/PetContextMenu";
import { useWindowState } from "./hooks/useWindowState";
import { call } from "./lib/tauri";
import type { PetLayout } from "./types/window";
import { useMonitorState } from "./hooks/useMonitorState";
import { useI18n } from "./i18n";

export function PetSettingsApp() {
  const { state } = useWindowState();
  const { state: monitor } = useMonitorState();
  const { t } = useI18n(monitor.language);
  const preferences = state.petWindow;

  // 每次都是“关掉设置窗口”+“做另一件事”两个独立命令，并发发出即可，不必等前一个返回。
  const switchToMain = () => {
    void Promise.all([
      call("hide_pet_settings"),
      call("switch_app_mode", { mode: "main" }),
    ]);
  };

  const hidePet = () => {
    void Promise.all([
      call("hide_pet_settings"),
      call("hide_current_window"),
    ]);
  };

  return (
    <main className="pet-settings-shell">
      <header>
        <div>
          <strong>{t("petSettings")}</strong>
          <small>{t("petSettingsHint")}</small>
        </div>
        <button
          type="button"
          aria-label={t("closePetSettings")}
          title={t("close")}
          onClick={() => void call("hide_pet_settings")}
        >
          ×
        </button>
      </header>
      <PetContextMenu
        standalone
        preferences={preferences}
        sizeMin={state.petSizeMin}
        sizeMax={state.petSizeMax}
        onLayout={(layout: PetLayout) => void call("set_pet_layout", { layout })}
        onSize={(size) => void call("set_pet_size", { size })}
        onAlwaysOnTop={(enabled) => void call("set_pet_always_on_top", { enabled })}
        onLocked={(locked) => void call("set_pet_locked", { locked })}
        onMain={switchToMain}
        onHide={hidePet}
        t={t}
      />
    </main>
  );
}
