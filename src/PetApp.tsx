import { useEffect, useRef, useState, type MouseEvent, type WheelEvent } from "react";
import { usePetViewState } from "./hooks/usePetViewState";
import { call } from "./lib/tauri";
import { buildImageUrl, type MonitorTile } from "./types/monitor";
import type { PetPageDirection } from "./types/pet";
import { useI18n, type TranslationFunction } from "./i18n";

const turnPage = (direction: PetPageDirection) => void call("turn_pet_page", { direction });

// 桌宠里的单个宫格：有图片时只显示图片 + 悬浮标签，没有数据时显示序号占位。
function PetTile({ tile, index, imageUrl, t }: {
  tile?: MonitorTile;
  index: number;
  imageUrl?: string;
  t: TranslationFunction;
}) {
  const name = tile?.aiName.trim() || "AI";
  const username = tile?.username.trim() || t("user");
  const content = tile?.content.trim();
  const title = `${name}_${username}`;
  return (
    <section className={`pet-tile${imageUrl ? " occupied" : " empty"}`}>
      {imageUrl ? (
        <img src={imageUrl} alt={`${index + 1}-${title}`} draggable={false} />
      ) : (
        <div className="pet-empty"><span>{String(index + 1).padStart(2, "0")}</span><small>{t("waitingData")}</small></div>
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
  const { state } = usePetViewState(); // Rust 已将监控数据、窗口偏好与分页投影成一个一致快照
  const { t } = useI18n(state.language);
  const [isHovered, setIsHovered] = useState(false);
  const lastWheelAt = useRef(0); // 滚轮事件节流用的时间戳，避免触控板连续触发过多命令
  const ensuredVisiblePage = useRef(false);

  const switchToMain = () => void call("switch_app_mode", { mode: "main" });
  const openSettings = () => void call("show_pet_settings");

  const onMouseDown = (event: MouseEvent<HTMLElement>) => {
    // 只响应左键单击；双击交给 onDoubleClick；锁定状态下不允许拖拽；
    // data-pet-control 标记的是翻页按钮等控件区域，点在上面不应该触发拖拽。
    if (event.button !== 0 || event.detail > 1 || state.locked) return;
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
      // 按住 Ctrl/Cmd 滚轮：前端只上报放大/缩小意图，步长与边界由 Rust 决定。
      void call("resize_pet_step", { direction: event.deltaY < 0 ? "grow" : "shrink" });
      return;
    }
    if (state.pageCount > 1) turnPage(event.deltaY < 0 ? "previous" : "next");
  };

  const onContextMenu = (event: MouseEvent<HTMLElement>) => {
    event.preventDefault(); // 阻止浏览器默认右键菜单，改为打开桌宠自己的设置窗口
    openSettings();
  };

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "ArrowLeft") turnPage("previous");
      if (event.key === "ArrowRight") turnPage("next");
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
    if (ensuredVisiblePage.current || !state.hasAnyImage) return;
    ensuredVisiblePage.current = true;
    // “每次 WebView 挂载最多一次”属于 UI 生命周期；选哪一页以及是否需要跳转由 Rust 原子判断。
    void call("focus_first_populated_pet_page");
  }, [state.hasAnyImage]);

  return (
    <main
      className={`pet-shell ${state.layout}${state.locked ? " locked" : ""}${isHovered ? " hovered" : ""}${state.pageHasImage ? "" : " empty-page"}`}
      onMouseEnter={() => setIsHovered(true)}
      onMouseLeave={() => setIsHovered(false)}
      onMouseDown={onMouseDown}
      onDoubleClick={onDoubleClick}
      onWheel={onWheel}
      onContextMenu={onContextMenu}
      tabIndex={0}
      aria-label={t("petAria", { page: state.pageIndex + 1, pages: state.pageCount })}
    >
      <div className="pet-grid">
        {state.slots.map(({ slotIndex, tile }) => {
          const imageUrl = tile?.imageFilename
            ? buildImageUrl(state.port, tile.imageFilename)
            : undefined;
          return <PetTile tile={tile ?? undefined} index={slotIndex} imageUrl={imageUrl} t={t} key={slotIndex} />;
        })}
      </div>

      <div className="pet-pager" data-pet-control aria-hidden={!isHovered}>
        <button
          aria-label={t("previousPage")}
          title={t("previousPage")}
          disabled={state.pageCount <= 1}
          tabIndex={isHovered ? 0 : -1}
          onClick={() => turnPage("previous")}
        >‹</button>
        <span>{state.pageIndex + 1}/{state.pageCount}</span>
        <button
          aria-label={t("nextPage")}
          title={t("nextPage")}
          disabled={state.pageCount <= 1}
          tabIndex={isHovered ? 0 : -1}
          onClick={() => turnPage("next")}
        >›</button>
      </div>
    </main>
  );
}
