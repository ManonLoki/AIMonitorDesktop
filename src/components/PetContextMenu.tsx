import type { PetLayout, PetWindowPreferences } from "../types/window";

interface PetContextMenuProps {
  x?: number;
  y?: number;
  standalone?: boolean;
  preferences: PetWindowPreferences;
  sizeMin: number;
  sizeMax: number;
  onLayout: (layout: PetLayout) => void;
  onSize: (size: number) => void;
  onAlwaysOnTop: (enabled: boolean) => void;
  onLocked: (locked: boolean) => void;
  onMain: () => void;
  onHide: () => void;
}

export function PetContextMenu({
  x,
  y,
  standalone = false,
  preferences,
  sizeMin,
  sizeMax,
  onLayout,
  onSize,
  onAlwaysOnTop,
  onLocked,
  onMain,
  onHide,
}: PetContextMenuProps) {
  const left = Math.min(x ?? 8, Math.max(8, window.innerWidth - 190));
  const top = Math.min(y ?? 8, Math.max(8, window.innerHeight - 326));
  const size = Math.min(sizeMax, Math.max(sizeMin, preferences.petSize));
  return (
    <div
      className={`pet-context-menu${standalone ? " standalone" : ""}`}
      role="menu"
      data-pet-control
      style={standalone ? undefined : { left, top }}
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
          className={preferences.layout === "row" ? "selected" : ""}
          onClick={() => onLayout("row")}
        >
          1×2
        </button>
        <button
          className={preferences.layout === "column" ? "selected" : ""}
          onClick={() => onLayout("column")}
        >
          2×1
        </button>
        <button
          className={preferences.layout === "row3" ? "selected" : ""}
          onClick={() => onLayout("row3")}
        >
          1×3
        </button>
        <button
          className={preferences.layout === "column3" ? "selected" : ""}
          onClick={() => onLayout("column3")}
        >
          3×1
        </button>
        <button
          className={preferences.layout === "grid" ? "selected" : ""}
          onClick={() => onLayout("grid")}
        >
          2×2
        </button>
      </div>
      <div className="pet-menu-label">桌宠大小</div>
      <div className="pet-size-control">
        <input
          type="range"
          min={sizeMin}
          max={sizeMax}
          step="1"
          value={size}
          aria-label="桌宠大小"
          aria-valuetext={`${size} 像素`}
          onChange={(event) => onSize(Number(event.currentTarget.value))}
        />
        <output>{size}px</output>
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
