/**
 * Typed wrappers over Tauri `invoke`. Every command mirrors a Rust command in
 * `src-tauri/src/` (PLAN.md §4.5). Commands are added stage by stage.
 */

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
