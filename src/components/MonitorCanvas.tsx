import { AnimatedContent } from "./reactbits/AnimatedContent";
import { Icon } from "./Icon";
import type { MonitorState } from "../types/monitor";

// 监控画布：按当前 rows × columns 渲染实际可见的宫格（最多 25 个，多余的裁掉不渲染）。
// 每个宫格展示：图片（若有）、"序号-AI名称-用户名" 表头、正文文案与更新时间。
export function MonitorCanvas({ state }: { state: MonitorState }) {
  // 后端始终维护满 25 个宫格的数组，这里只取当前行列布局需要展示的前 N 个。
  const visible = state.tiles.slice(0, state.rows * state.columns);
  // 行列数较多时（>=4）空间更紧张，用 compact 样式收敛内边距与文案。
  const compact = Math.max(state.rows, state.columns) >= 4;

  return (
    <main
      className={`monitor-grid${compact ? " compact" : ""}`}
      style={{
        gridTemplateColumns: `repeat(${state.columns}, minmax(0, 1fr))`,
        gridTemplateRows: `repeat(${state.rows}, minmax(0, 1fr))`,
      }}
      aria-label={`${state.rows} 行 ${state.columns} 列监控宫格`}
    >
      {visible.map((tile, index) => {
        // 图片由桌面端内置的 HTTP 服务器（src-tauri/src/lib.rs）提供，
        // 走本机回环地址而非局域网地址，避免受网络环境影响。
        const imageUrl = tile.imageFilename
          ? `http://127.0.0.1:${state.port}/api/images/${encodeURIComponent(tile.imageFilename)}`
          : undefined;
        const aiName = tile.aiName.trim() || "AI";
        const username = tile.username.trim() || "等待数据";
        return (
          // 入场动画的延迟按序号递增但设置上限，避免宫格很多时最后几个等待过久。
          <AnimatedContent
            className="tile-motion"
            delay={Math.min(index * 0.045, 0.35)}
            distance={18}
            duration={0.48}
            scale={0.97}
            key={index}
          >
            <section className="monitor-tile">
              {imageUrl && (
                <img
                  src={imageUrl}
                  alt={`${index + 1}-${aiName}-${username}`}
                  className={state.imageDisplayMode === "FILL_CROP" ? "cover" : "contain"}
                />
              )}
              <div className="tile-header">
                {String(index + 1).padStart(2, "0")}-{aiName}-{username}
              </div>
              {!imageUrl && (
                <div className="empty-state">
                  <Icon name="image" />
                  {!compact && <span>等待数据</span>}
                </div>
              )}
              {tile.content && (
                <div className="tile-caption">
                  <span>{tile.content}</span>
                  {tile.updatedAtMillis && (
                    <time>
                      {new Date(tile.updatedAtMillis).toLocaleTimeString("zh-CN", {
                        hour12: false,
                      })}
                    </time>
                  )}
                </div>
              )}
            </section>
          </AnimatedContent>
        );
      })}
    </main>
  );
}
