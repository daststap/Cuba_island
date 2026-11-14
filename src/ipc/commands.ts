/**
 * Typed wrappers over Tauri `invoke`. Every command mirrors a Rust command in
 * `src-tauri/src/` (PLAN.md §4.5).
 */
import { invoke } from "@tauri-apps/api/core";

/** Mirrors `AppError::kind()` in `src-tauri/src/error.rs`. */
export type AppErrorKind =
  "io" | "tauri" | "invalid_argument" | "windows" | "unavailable" | "internal";

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

/** Mirrors `media::MediaState`. */
export interface MediaState {
  /** Changes whenever the track changes. */
  trackId: string;
  title: string;
  artist: string;
  album: string;
  appId: string;
  isPlaying: boolean;
  positionMs: number;
  durationMs: number;
  /** Unix ms at which `positionMs` was measured. */
  positionUpdatedAt: number;
  /** JPEG data URL. */
  cover: string | null;
}

export type MediaCommand = "toggle" | "next" | "prev";

export function mediaGetState(): Promise<MediaState | null> {
  return invoke("media_get_state");
}

export function mediaCommand(cmd: MediaCommand): Promise<void> {
  return invoke("media_command", { cmd });
}

/** Mirrors `clipboard::store::ClipItem`. */
export interface ClipItem {
  id: number;
  kind: "text" | "image";
  /** Up to 400 characters of the text. */
  text: string | null;
  /** Absolute path of the 256 px preview; load it via `convertFileSrc`. */
  thumbPath: string | null;
  pinned: boolean;
  createdAt: number;
  usedAt: number;
}

export function clipboardList(query: string, limit: number, offset = 0): Promise<ClipItem[]> {
  return invoke("clipboard_list", { query: query || null, limit, offset });
}

/** Puts a clip on the clipboard without pasting. */
export function clipboardCopy(id: number): Promise<void> {
  return invoke("clipboard_copy", { id });
}

/** Puts a clip on the clipboard, returns focus to the previous window and pastes there. */
export function clipboardPaste(id: number): Promise<void> {
  return invoke("clipboard_paste", { id });
}

export function clipboardPin(id: number, pinned: boolean): Promise<void> {
  return invoke("clipboard_pin", { id, pinned });
}

export function clipboardDelete(id: number): Promise<void> {
  return invoke("clipboard_delete", { id });
}

export function clipboardClear(): Promise<void> {
  return invoke("clipboard_clear");
}

/** Lets the island take keyboard focus (true) or gives focus back (false). */
export function setInputMode(enabled: boolean): Promise<void> {
  return invoke("set_input_mode", { enabled });
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
