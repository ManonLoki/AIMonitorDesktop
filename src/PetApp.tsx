import { useEffect, useMemo, useRef, useState, type MouseEvent, type WheelEvent } from "react";
import { useMonitorState } from "./hooks/useMonitorState";
import { useWindowState } from "./hooks/useWindowState";
import { call } from "./lib/tauri";
import { buildImageUrl, type MonitorTile } from "./types/monitor";

const LAYOUT_CAPACITY = {
  single: 1,
  row: 2,
  column: 2,
  row3: 3,
  column3: 3,
  grid: 4,
} as const;

// 桌宠里的单个宫格：有图片时只显示图片 + 悬浮标签，没有数据时显示序号占位。
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
  const { state: monitor } = useMonitorState(); // 宫格数据（rows/columns/tiles），跟主窗口共享同一份状态
  const { state: windows } = useWindowState(); // 桌宠自己的窗口偏好（布局/锁定/焦点槽位等）
  const preferences = windows.petWindow;
  const [isHovered, setIsHovered] = useState(false);
  const lastWheelAt = useRef(0); // 滚轮事件节流用的时间戳，避免触控板连续触发过多命令
  const capacity = LAYOUT_CAPACITY[preferences.layout];
  const visibleSlotCount = Math.max(1, monitor.rows * monitor.columns); // 当前行列数下实际有效的宫格数
  const pageCount = Math.max(1, Math.ceil(visibleSlotCount / capacity));
  // focusedSlot 是后端持久化的“当前聚焦第几个宫格”，换算成页码；行列数变小后可能越界，用 min 收敛。
  const pageIndex = Math.min(pageCount - 1, Math.floor(preferences.focusedSlot / capacity));
  const pageStart = pageIndex * capacity;

  // 当前页要渲染的宫格下标列表，例如 2×2 布局第 2 页是 [4,5,6,7]。
  const pageSlots = useMemo(
    () => Array.from({ length: capacity }, (_, offset) => pageStart + offset),
    [capacity, pageStart],
  );
  const firstImageSlot = monitor.tiles
    .slice(0, visibleSlotCount)
    .findIndex((tile) => Boolean(tile?.imageFilename));
  const pageHasImage = pageSlots.some((slot) => Boolean(monitor.tiles[slot]?.imageFilename));
  const ensuredVisiblePage = useRef(false);

  // 跳到指定页：页码取模实现首尾循环翻页（最后一页下一页回到第一页）。
  const focusPage = (nextPage: number) => {
    const wrapped = (nextPage + pageCount) % pageCount;
    void call("set_pet_focused_slot", { slot: wrapped * capacity });
  };

  const turnPage = (direction: -1 | 1) => focusPage(pageIndex + direction);
  // 键盘事件监听只挂载一次（见下面 useEffect 的空依赖数组），但要用到最新的 turnPage
  // （它闭包捕获了 pageIndex/pageCount 等每次渲染都可能变化的值），所以用 ref 存最新引用。
  const turnPageRef = useRef(turnPage);
  turnPageRef.current = turnPage;

  const switchToMain = () => void call("switch_app_mode", { mode: "main" });
  const openSettings = () => void call("show_pet_settings");

  const onMouseDown = (event: MouseEvent<HTMLElement>) => {
    // 只响应左键单击；双击交给 onDoubleClick；锁定状态下不允许拖拽；
    // data-pet-control 标记的是翻页按钮等控件区域，点在上面不应该触发拖拽。
    if (event.button !== 0 || event.detail > 1 || preferences.locked) return;
    if ((event.target as HTMLElement).closest("[data-pet-control]")) return;
    void call("start_pet_drag");
  };

  const onDoubleClick = (event: MouseEvent<HTMLElement>) => {
    if ((event.target as HTMLElement).closest("[data-pet-control]")) return;
    switchToMain(); // 双击桌宠空白区域切回主看板
  };

  const onWheel = (event: WheelEvent<HTMLElement>) => {
    const now = Date.now();
    if (now - lastWheelAt.current < 220) return; // 220ms 节流：触控板一次手势会连续触发几十个 wheel 事件
    lastWheelAt.current = now;
    if (event.ctrlKey || event.metaKey) {
      // 按住 Ctrl/Cmd 滚轮：缩放桌宠尺寸，而不是翻页。
      void call("resize_pet_by", { delta: event.deltaY < 0 ? 24 : -24 });
      return;
    }
    if (pageCount > 1) turnPage(event.deltaY < 0 ? -1 : 1); // 只有一页时滚轮不需要做任何事
  };

  const onContextMenu = (event: MouseEvent<HTMLElement>) => {
    event.preventDefault(); // 阻止浏览器默认右键菜单，改为打开桌宠自己的设置窗口
    openSettings();
  };

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "ArrowLeft") turnPageRef.current(-1);
      if (event.key === "ArrowRight") turnPageRef.current(1);
    };
    const hideControls = () => setIsHovered(false);
    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("blur", hideControls);
    document.addEventListener("mouseleave", hideControls);
    return () => {
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("blur", hideControls);
      document.removeEventListener("mouseleave", hideControls);
    };
  }, []); // 空依赖：只挂载一次，避免 monitor-state-changed 等高频重渲染反复重新绑定全局监听

  useEffect(() => {
    if (ensuredVisiblePage.current || firstImageSlot < 0) return;
    ensuredVisiblePage.current = true;
    if (!pageHasImage) {
      void call("set_pet_focused_slot", { slot: firstImageSlot });
    }
  }, [firstImageSlot, pageHasImage]);

  return (
    <main
      className={`pet-shell ${preferences.layout}${preferences.locked ? " locked" : ""}${isHovered ? " hovered" : ""}${pageHasImage ? "" : " empty-page"}`}
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
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
            ? buildImageUrl(monitor.port, tile.imageFilename)
            : undefined;
          return <PetTile tile={tile} index={slot} imageUrl={imageUrl} key={slot} />;
        })}
      </div>

      <div className="pet-pager" data-pet-control aria-hidden={!isHovered}>
        <button
          aria-label="上一页"
          title="上一页"
          disabled={pageCount <= 1}
          tabIndex={isHovered ? 0 : -1}
          onClick={() => turnPage(-1)}
        >‹</button>
        <span>{pageIndex + 1}/{pageCount}</span>
        <button
          aria-label="下一页"
          title="下一页"
          disabled={pageCount <= 1}
          tabIndex={isHovered ? 0 : -1}
          onClick={() => turnPage(1)}
        >›</button>
      </div>
    </main>
  );
}
