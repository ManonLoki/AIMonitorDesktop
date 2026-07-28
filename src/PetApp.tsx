import { invoke } from "@tauri-apps/api/core";
import { useEffect, useMemo, useRef, useState, type MouseEvent, type WheelEvent } from "react";
import { PetContextMenu } from "./components/PetContextMenu";
import { useMonitorState } from "./hooks/useMonitorState";
import { useWindowState } from "./hooks/useWindowState";
import type { MonitorTile } from "./types/monitor";
import type { PetLayout } from "./types/window";

interface MenuPosition { x: number; y: number }

const call = (command: string, args?: Record<string, unknown>) =>
  invoke<void>(command, args).catch(() => undefined);

function PetTile({ tile, index, imageUrl }: {
  tile?: MonitorTile;
  index: number;
  imageUrl?: string;
}) {
  const name = tile?.aiName.trim() || "AI";
  const username = tile?.username.trim() || "等待数据";
  return (
    <section className={`pet-tile${imageUrl ? " occupied" : " empty"}`}>
      {imageUrl ? (
        <img src={imageUrl} alt={`${index + 1}-${name}-${username}`} draggable={false} />
      ) : (
        <div className="pet-empty"><span>{String(index + 1).padStart(2, "0")}</span><small>等待数据</small></div>
      )}
      {tile && (imageUrl || tile.content) && (
        <div className="pet-info">
          <strong>{String(index + 1).padStart(2, "0")} · {name}</strong>
          <small>{username}</small>
          {tile.content && <p>{tile.content}</p>}
          {tile.updatedAtMillis && (
            <time>{new Date(tile.updatedAtMillis).toLocaleTimeString("zh-CN", { hour12: false })}</time>
          )}
        </div>
      )}
    </section>
  );
}

export function PetApp() {
  const { state: monitor } = useMonitorState();
  const { state: windows } = useWindowState();
  const preferences = windows.petWindow;
  const [menu, setMenu] = useState<MenuPosition | null>(null);
  const lastWheelAt = useRef(0);
  const capacity = preferences.layout === "single" ? 1 : 4;
  const visibleSlotCount = Math.max(1, monitor.rows * monitor.columns);
  const pageCount = Math.max(1, Math.ceil(visibleSlotCount / capacity));
  const pageIndex = Math.min(pageCount - 1, Math.floor(preferences.focusedSlot / capacity));
  const pageStart = pageIndex * capacity;

  const pageSlots = useMemo(
    () => Array.from({ length: capacity }, (_, offset) => pageStart + offset),
    [capacity, pageStart],
  );

  const focusPage = (nextPage: number) => {
    const wrapped = (nextPage + pageCount) % pageCount;
    void call("set_pet_focused_slot", { slot: wrapped * capacity });
  };

  const turnPage = (direction: -1 | 1) => focusPage(pageIndex + direction);

  const closeMenuAfter = (action: () => void) => {
    setMenu(null);
    action();
  };

  const switchToMain = () => closeMenuAfter(() => {
    void call("switch_app_mode", { mode: "main" });
  });

  const onMouseDown = (event: MouseEvent<HTMLElement>) => {
    if (event.button !== 0 || event.detail > 1 || preferences.locked) return;
    if ((event.target as HTMLElement).closest("[data-pet-control]")) return;
    setMenu(null);
    void call("start_pet_drag");
  };

  const onDoubleClick = (event: MouseEvent<HTMLElement>) => {
    if ((event.target as HTMLElement).closest("[data-pet-control]")) return;
    switchToMain();
  };

  const onWheel = (event: WheelEvent<HTMLElement>) => {
    const now = Date.now();
    if (now - lastWheelAt.current < 220) return;
    lastWheelAt.current = now;
    if (event.ctrlKey || event.metaKey) {
      void call("resize_pet_by", { delta: event.deltaY < 0 ? 24 : -24 });
      return;
    }
    if (pageCount > 1) turnPage(event.deltaY < 0 ? -1 : 1);
  };

  const onContextMenu = (event: MouseEvent<HTMLElement>) => {
    event.preventDefault();
    setMenu({ x: event.clientX, y: event.clientY });
  };

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") setMenu(null);
      if (event.key === "ArrowLeft") turnPage(-1);
      if (event.key === "ArrowRight") turnPage(1);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  });

  return (
    <main
      className={`pet-shell ${preferences.layout}${preferences.locked ? " locked" : ""}`}
      onMouseDown={onMouseDown}
      onDoubleClick={onDoubleClick}
      onWheel={onWheel}
      onContextMenu={onContextMenu}
      onClick={() => menu && setMenu(null)}
      tabIndex={0}
      aria-label={`桌宠，第 ${pageIndex + 1} 页，共 ${pageCount} 页`}
    >
      <div className="pet-grid">
        {pageSlots.map((slot) => {
          const tile = slot < visibleSlotCount ? monitor.tiles[slot] : undefined;
          const imageUrl = tile?.imageFilename
            ? `http://127.0.0.1:${monitor.port}/api/images/${encodeURIComponent(tile.imageFilename)}`
            : undefined;
          return <PetTile tile={tile} index={slot} imageUrl={imageUrl} key={slot} />;
        })}
      </div>

      {pageCount > 1 && (
        <div className="pet-pager" data-pet-control>
          <button aria-label="上一页" title="上一页" onClick={() => turnPage(-1)}>‹</button>
          <span>{pageIndex + 1}/{pageCount}</span>
          <button aria-label="下一页" title="下一页" onClick={() => turnPage(1)}>›</button>
        </div>
      )}

      <button
        className="pet-menu-trigger"
        data-pet-control
        aria-label="桌宠菜单"
        title="桌宠菜单"
        onClick={(event) => {
          event.stopPropagation();
          setMenu(menu ? null : { x: event.clientX, y: event.clientY });
        }}
      >
        •••
      </button>

      {!preferences.locked && (
        <button
          className="pet-resize-handle"
          data-pet-control
          aria-label="调整桌宠大小"
          title="拖动调整大小"
          onMouseDown={(event) => {
            event.stopPropagation();
            void call("start_pet_resize");
          }}
        />
      )}

      {menu && (
        <PetContextMenu
          x={menu.x}
          y={menu.y}
          preferences={preferences}
          onLayout={(layout: PetLayout) => closeMenuAfter(() => void call("set_pet_layout", { layout }))}
          onScale={(preset) => closeMenuAfter(() => void call("set_pet_scale", { preset }))}
          onAlwaysOnTop={(enabled) => closeMenuAfter(() => void call("set_pet_always_on_top", { enabled }))}
          onLocked={(locked) => closeMenuAfter(() => void call("set_pet_locked", { locked }))}
          onMain={switchToMain}
          onHide={() => closeMenuAfter(() => void call("hide_current_window"))}
        />
      )}
    </main>
  );
}
