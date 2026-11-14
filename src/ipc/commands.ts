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

/** Mirrors `launcher::LaunchResult`. */
export interface LaunchResult {
  /** `app:…`, `cmd:…`, `calc:…` or `web:…` — passed back to `launcherRun`. */
  id: string;
  kind: "app" | "command" | "calc" | "web";
  /** App name, calculator answer or search query (commands are labelled in `ru.ts`). */
  title: string;
  iconPath: string | null;
}

export function launcherSearch(query: string): Promise<LaunchResult[]> {
  return invoke("launcher_search", { query });
}

export function launcherRun(id: string): Promise<void> {
  return invoke("launcher_run", { id });
}

/** Mirrors `hotkeys::HotkeyStatus`. */
export interface HotkeyStatus {
  clipboard: string | null;
  launcher: string | null;
  launcherFallback: boolean;
}

export function hotkeysStatus(): Promise<HotkeyStatus> {
  return invoke("hotkeys_status");
}

/** Releases our global hotkeys while the user records a new one. */
export function hotkeysPause(paused: boolean): Promise<void> {
  return invoke("hotkeys_pause", { paused });
}

/** Mirrors `settings::Settings`. */
export interface Settings {
  monitor: string | null;
  autostart: boolean;
  hideInFullscreen: boolean;
  hour12: boolean;
  clipboardHotkey: string;
  launcherHotkey: string;
  historyLimit: number;
  copyPeek: boolean;
  /** 0.5–1 */
  opacity: number;
  onboarded: boolean;
}

export function settingsGet(): Promise<Settings> {
  return invoke("settings_get");
}

/** Applies and saves some settings; rejects (saving nothing) if applying fails. */
export function settingsSet(patch: Partial<Settings>): Promise<Settings> {
  return invoke("settings_set", { patch });
}

/** Mirrors `window::MonitorInfo`. */
export interface MonitorInfo {
  name: string;
  width: number;
  height: number;
  primary: boolean;
}

export function monitorsList(): Promise<MonitorInfo[]> {
  return invoke("monitors_list");
}

/** Mirrors `updater::UpdateInfo`. */
export interface UpdateInfo {
  version: string;
  notes: string | null;
}

export function updateCheck(): Promise<UpdateInfo | null> {
  return invoke("update_check");
}

/** Downloads, verifies and installs the update, then restarts the app. */
export function updateInstall(): Promise<void> {
  return invoke("update_install");
}

type ErrorSink = (message: string) => void;
let errorSink: ErrorSink | undefined;

/** Lets the app show command errors (as toasts in the island). */
export function setErrorSink(sink: ErrorSink | undefined): void {
  errorSink = sink;
}

/** Logs a failed command and shows its message to the user. */
export function reportError(context: string, error: unknown): void {
  const message = isAppError(error) ? error.message : String(error);
  console.error(`[${context}] ${isAppError(error) ? `${error.kind}: ` : ""}${message}`);
  errorSink?.(message);
}
