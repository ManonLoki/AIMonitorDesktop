export type AppMode = "main" | "pet";
export type PetLayout = "single" | "row" | "column" | "row3" | "column3" | "grid";

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
  rowGeometry?: WindowGeometry | null;
  columnGeometry?: WindowGeometry | null;
  row3Geometry?: WindowGeometry | null;
  column3Geometry?: WindowGeometry | null;
  gridGeometry?: WindowGeometry | null;
  petSize: number;
  alwaysOnTop: boolean;
  locked: boolean;
}

export interface WindowState {
  activeMode: AppMode;
  petWindow: PetWindowPreferences;
  petSizeMin: number;
  petSizeMax: number;
}

export const previewWindowState: WindowState = {
  activeMode: "main",
  petWindow: {
    layout: "grid",
    focusedSlot: 0,
    petSize: 64,
    alwaysOnTop: true,
    locked: false,
  },
  petSizeMin: 32,
  petSizeMax: 180,
};
