import type { PetLayout, PetWindowPreferences } from "../types/window";

interface PetContextMenuProps {
  x: number;
  y: number;
  preferences: PetWindowPreferences;
  onLayout: (layout: PetLayout) => void;
  onScale: (preset: number) => void;
  onAlwaysOnTop: (enabled: boolean) => void;
  onLocked: (locked: boolean) => void;
  onMain: () => void;
  onHide: () => void;
}

export function PetContextMenu({
  x,
  y,
  preferences,
  onLayout,
  onScale,
  onAlwaysOnTop,
  onLocked,
  onMain,
  onHide,
}: PetContextMenuProps) {
  const left = Math.min(x, Math.max(8, window.innerWidth - 190));
  const top = Math.min(y, Math.max(8, window.innerHeight - 310));
  return (
    <div
      className="pet-context-menu"
      role="menu"
      data-pet-control
      style={{ left, top }}
      onMouseDown={(event) => event.stopPropagation()}
    >
      <div className="pet-menu-label">显示数量</div>
      <div className="pet-menu-segment" role="group" aria-label="桌宠布局">
        <button
          className={preferences.layout === "single" ? "selected" : ""}
          onClick={() => onLayout("single")}
        >
          1×1
        </button>
        <button
          className={preferences.layout === "grid" ? "selected" : ""}
          onClick={() => onLayout("grid")}
        >
          2×2
        </button>
      </div>
      <div className="pet-menu-label">桌宠大小</div>
      <div className="pet-scale-grid" role="group" aria-label="桌宠大小">
        {[75, 100, 125, 150].map((preset) => (
          <button
            className={preferences.scalePreset === preset ? "selected" : ""}
            key={preset}
            onClick={() => onScale(preset)}
          >
            {preset}%
          </button>
        ))}
      </div>
      <button
        className="pet-menu-check"
        role="menuitemcheckbox"
        aria-checked={preferences.alwaysOnTop}
        onClick={() => onAlwaysOnTop(!preferences.alwaysOnTop)}
      >
        <span>{preferences.alwaysOnTop ? "✓" : ""}</span>始终置顶
      </button>
      <button
        className="pet-menu-check"
        role="menuitemcheckbox"
        aria-checked={preferences.locked}
        onClick={() => onLocked(!preferences.locked)}
      >
        <span>{preferences.locked ? "✓" : ""}</span>锁定位置和大小
      </button>
      <div className="pet-menu-divider" />
      <button role="menuitem" onClick={onMain}>返回主界面</button>
      <button role="menuitem" onClick={onHide}>隐藏到托盘</button>
    </div>
  );
}
