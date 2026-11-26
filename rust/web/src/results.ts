import type { Value } from "./types";

export type Panel = "results" | "history" | "plan" | "object" | "stats" | "output";
export type ObjectTab = "columns" | "constraints" | "indexes" | "triggers" | "grants";

// Virtualised grid: fixed row height, only the visible window (plus overscan) is rendered.
export const ROW_HEIGHT = 24;
export const OVERSCAN = 8;
export const VISIBLE_ROWS = 60;

export function cellText(value: Value): string {
  if (value.kind === "null") return "";
  if (value.kind === "binary") {
    return value.value.map((byte) => byte.toString(16).padStart(2, "0")).join("");
  }
  return value.value;
}
