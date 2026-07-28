import { AnimatedContent } from "./reactbits/AnimatedContent";
import { Icon } from "./Icon";
import { buildImageUrl, type MonitorState } from "../types/monitor";
import type { TranslationFunction } from "../i18n";

// 监控画布：按当前 rows × columns 渲染实际可见的宫格（最多 25 个，多余的裁掉不渲染）。
// 每个宫格展示：图片（若有）、"序号-AI名称-用户名" 表头、正文文案与更新时间。
export function MonitorCanvas({ state, t }: { state: MonitorState; t: TranslationFunction }) {
  // 后端始终维护满 25 个宫格的数组，这里只取当前行列布局需要展示的前 N 个。
  const visible = state.tiles.slice(0, state.rows * state.columns);
  // 行列数较多时（>=4）空间更紧张，用 compact 样式收敛内边距与文案。
  const compact = Math.max(state.rows, state.columns) >= 4;

  return (
    <main
      className={`monitor-grid${compact ? " compact" : ""}`}
      style={{
        gridTemplateColumns: `repeat(${state.columns}, minmax(0, 1fr))`, // CSS Grid 按列数均分宽度
        gridTemplateRows: `repeat(${state.rows}, minmax(0, 1fr))`, // CSS Grid 按行数均分高度
      }}
      aria-label={t("gridAria", { rows: state.rows, columns: state.columns })}
    >
      {visible.map((tile, index) => {
        const imageUrl = tile.imageFilename
          ? buildImageUrl(state.port, tile.imageFilename)
          : undefined; // 没有图片文件名则不渲染 img 标签
        const aiName = tile.aiName.trim() || "AI"; // 空 AI 名称时的占位文案
        const username = tile.username.trim() || t("waitingData"); // 空用户名时的占位文案
        return (
          // 入场动画的延迟按序号递增但设置上限，避免宫格很多时最后几个等待过久。
          <AnimatedContent
            className="tile-motion"
            delay={Math.min(index * 0.045, 0.35)} // 每格间隔 0.045s，但最多延迟 0.35s
            distance={18}
            duration={0.48}
            scale={0.97}
            key={index} // 宫格顺序固定不变，用下标作为 key 是安全的
          >
            <section className="monitor-tile">
              {imageUrl && (
                // 有图片时渲染图片本体，模式决定是等比缩放（contain）还是铺满裁剪（cover）
                <img
                  src={imageUrl}
                  alt={`${index + 1}-${aiName}-${username}`}
                  className={state.imageDisplayMode === "FILL_CROP" ? "cover" : "contain"}
                />
              )}
              <div className="tile-header">
                {/* 序号补零到两位，格式为 "01-AI名-用户名" */}
                {String(index + 1).padStart(2, "0")}-{aiName}-{username}
              </div>
              {!imageUrl && (
                // 没有图片时展示占位图标 + 文案（compact 模式下省略文案以节省空间）
                <div className="empty-state">
                  <Icon name="image" />
                  {!compact && <span>{t("waitingData")}</span>}
                </div>
              )}
              {tile.content && (
                // 有正文内容才展示这一行，同时附带更新时间（若存在）
                <div className="tile-caption">
                  <span>{tile.content}</span>
                  {tile.updatedAtMillis && (
                    <time>
                      {new Date(tile.updatedAtMillis).toLocaleTimeString(document.documentElement.lang, {
                        hour12: false, // 使用 24 小时制展示时间
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
