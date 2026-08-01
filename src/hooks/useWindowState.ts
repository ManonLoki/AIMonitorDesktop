import { previewWindowState, type WindowState } from "../types/window";
import { useTauriState } from "./useTauriState";

const WINDOW_EVENTS = ["window-state-changed"] as const;

export function useWindowState() {
  return useTauriState<WindowState>("get_window_state", WINDOW_EVENTS, previewWindowState);
}
