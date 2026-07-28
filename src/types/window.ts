export type AppMode = "main" | "pet";
export type PetLayout = "single" | "grid";

export interface WindowGeometry {
  x: number;
  y: number;
  width: number;
  height: number;
  scaleFactor: number;
}

export interface PetWindowPreferences {
  layout: PetLayout;
  focusedSlot: number;
  singleGeometry?: WindowGeometry | null;
  gridGeometry?: WindowGeometry | null;
  scalePreset: number;
  alwaysOnTop: boolean;
  locked: boolean;
}

export interface WindowState {
  activeMode: AppMode;
  petWindow: PetWindowPreferences;
}

export const previewWindowState: WindowState = {
  activeMode: "main",
  petWindow: {
    layout: "grid",
    focusedSlot: 0,
    scalePreset: 100,
    alwaysOnTop: true,
    locked: false,
  },
};
