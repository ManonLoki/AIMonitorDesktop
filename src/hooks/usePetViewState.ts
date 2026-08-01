import { useTauriState } from "./useTauriState";
import { previewPetViewState, type PetViewState } from "../types/pet";

const PET_VIEW_EVENTS = ["monitor-state-changed", "window-state-changed"] as const;

export function usePetViewState() {
  return useTauriState<PetViewState>("get_pet_view_state", PET_VIEW_EVENTS, previewPetViewState);
}
