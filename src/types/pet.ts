import type { LanguagePreference, MonitorTile } from "./monitor";
import type { PetLayout } from "./window";

export type PetPageDirection = "previous" | "next";
export type PetResizeDirection = "grow" | "shrink";

export interface PetViewSlot {
  slotIndex: number;
  tile: MonitorTile | null;
}

// Rust 将监控状态与窗口偏好组合成单一读模型；前端只负责渲染，不再自行计算分页。
export interface PetViewState {
  language: LanguagePreference;
  port: number;
  layout: PetLayout;
  locked: boolean;
  pageIndex: number;
  pageCount: number;
  pageHasImage: boolean;
  hasAnyImage: boolean;
  slots: PetViewSlot[];
}

// 仅用于脱离 Tauri 的浏览器预览；生产环境挂载后会被 Rust 快照整体替换。
export const previewPetViewState: PetViewState = {
  language: "system",
  port: 10241,
  layout: "grid",
  locked: false,
  pageIndex: 0,
  pageCount: 1,
  pageHasImage: false,
  hasAnyImage: false,
  slots: Array.from({ length: 4 }, (_, slotIndex) => ({ slotIndex, tile: null })),
};
