import { invoke } from "@tauri-apps/api/core";
import type { ResolvedLocale } from "../i18n";
import type { ImageDisplayMode, LanguagePreference } from "../types/monitor";
import type { PetPageDirection, PetResizeDirection } from "../types/pet";
import type { AppMode, PetLayout } from "../types/window";

type NoArgsCommand =
  | "focus_first_populated_pet_page"
  | "hide_current_window"
  | "hide_pet_settings"
  | "show_pet_settings"
  | "start_pet_drag";

interface CommandArgs {
  resize_pet_step: { direction: PetResizeDirection };
  set_auto_start: { enabled: boolean };
  set_device_name: { name: string };
  set_grid_columns: { columns: number };
  set_grid_rows: { rows: number };
  set_image_display_mode: { mode: ImageDisplayMode };
  set_language: { language: LanguagePreference };
  set_pet_always_on_top: { enabled: boolean };
  set_pet_layout: { layout: PetLayout };
  set_pet_locked: { locked: boolean };
  set_pet_size: { size: number };
  switch_app_mode: { mode: AppMode };
  sync_language: { locale: ResolvedLocale };
  turn_pet_page: { direction: PetPageDirection };
}

// 统一的类型化写命令边界；界面保持 Rust 事件驱动，不在本地乐观修改业务状态。
export function call(command: NoArgsCommand): Promise<void | undefined>;
export function call<K extends keyof CommandArgs>(command: K, args: CommandArgs[K]): Promise<void | undefined>;
export function call(command: string, args?: Record<string, unknown>) {
  return invoke<void>(command, args).catch(() => undefined);
}
