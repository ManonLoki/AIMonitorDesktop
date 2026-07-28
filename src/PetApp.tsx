import { invoke } from "@tauri-apps/api/core";
import { useEffect, useMemo, useRef, type MouseEvent, type WheelEvent } from "react";
import { useMonitorState } from "./hooks/useMonitorState";
import { useWindowState } from "./hooks/useWindowState";
import type { MonitorTile } from "./types/monitor";


const call = (command: string, args?: Record<string, unknown>) =>
  invoke<void>(command, args).catch(() => undefined);

function PetTile({ tile, index, imageUrl }: {
  tile?: MonitorTile;
  index: number;
  imageUrl?: string;
}) {
  const name = tile?.aiName.trim() || "AI";
  const username = tile?.username.trim() || "用户";
  const content = tile?.content.trim();
  const title = `${name}_${username}`;
  return (
    <section className={`pet-tile${imageUrl ? " occupied" : " empty"}`}>
      {imageUrl ? (
        <img src={imageUrl} alt={`${index + 1}-${title}`} draggable={false} />
      ) : (
        <div className="pet-empty"><span>{String(index + 1).padStart(2, "0")}</span><small>等待数据</small></div>
      )}
      {tile && (
        <div className="pet-labels" aria-label={content ? `${title}，${content}` : title}>
          <strong>{title}</strong>
          {content && <span>{content}</span>}
        </div>
      )}
    </section>
  );
}

export function PetApp() {
  const { state: monitor } = useMonitorState();
  const { state: windows } = useWindowState();
  const preferences = windows.petWindow;
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

  const switchToMain = () => void call("switch_app_mode", { mode: "main" });
  const openSettings = () => void call("show_pet_settings");

  const onMouseDown = (event: MouseEvent<HTMLElement>) => {
    if (event.button !== 0 || event.detail > 1 || preferences.locked) return;
    if ((event.target as HTMLElement).closest("[data-pet-control]")) return;
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
    openSettings();
  };

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
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

      <div className="pet-pager" data-pet-control>
        <button
          aria-label="上一页"
          title="上一页"
          disabled={pageCount <= 1}
          onClick={() => turnPage(-1)}
        >‹</button>
        <span>{pageIndex + 1}/{pageCount}</span>
        <button
          aria-label="下一页"
          title="下一页"
          disabled={pageCount <= 1}
          onClick={() => turnPage(1)}
        >›</button>
      </div>
    </main>
  );
}
