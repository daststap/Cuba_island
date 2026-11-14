/**
 * Typed wrappers over Tauri `invoke`. Every command mirrors a Rust command in
 * `src-tauri/src/` (PLAN.md §4.5).
 */
import { invoke } from "@tauri-apps/api/core";

/** Mirrors `AppError::kind()` in `src-tauri/src/error.rs`. */
export type AppErrorKind = "io" | "tauri" | "invalid_argument" | "internal";

/** Mirrors the serialized form of `AppError`. */
export interface AppError {
  kind: AppErrorKind;
  message: string;
}

export function isAppError(value: unknown): value is AppError {
  return (
    typeof value === "object" &&
    value !== null &&
    "kind" in value &&
    "message" in value &&
    typeof value.kind === "string" &&
    typeof value.message === "string"
  );
}

/** Logical px rectangle relative to the window's top-left corner. */
export interface IslandRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Mirrors `system::Battery`. */
export interface Battery {
  percent: number;
  charging: boolean;
  pluggedIn: boolean;
}

export function setIslandRect(rect: IslandRect): Promise<void> {
  return invoke("set_island_rect", { ...rect });
}

export function getBattery(): Promise<Battery | null> {
  return invoke("get_battery");
}

/** Logs a failed command. Errors are shown as toasts once toasts exist. */
export function reportError(context: string, error: unknown): void {
  const message = isAppError(error) ? `${error.kind}: ${error.message}` : String(error);
  console.error(`[${context}] ${message}`);
}
