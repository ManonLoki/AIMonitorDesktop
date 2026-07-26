import { atom } from "jotai";

export const monitoringEnabledAtom = atom(true);
export const selectedEnvironmentAtom = atom<"production" | "staging">(
  "production",
);
