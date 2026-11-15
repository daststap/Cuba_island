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
  /** Orange/green dots while the microphone/camera is in use. */
  privacyDots: boolean;
  /** A short peek when Caps Lock is toggled. */
  capsPeek: boolean;
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

/** Mirrors `claude::model::Session`: one Claude Code session seen through its hooks. */
export interface ClaudeSession {
  id: string;
  /** Project folder name. */
  project: string;
  state: "working" | "waiting" | "idle";
  /** Tool in use while working (Edit, Bash…). */
  tool: string | null;
  /** What Claude is waiting for. */
  message: string | null;
  updatedAt: number;
}

export function claudeStatus(): Promise<ClaudeSession[]> {
  return invoke("claude_status");
}

/** Mirrors `claude::HooksStatus`. */
export interface ClaudeHooksStatus {
  installed: boolean;
  settingsPath: string;
  /** The local endpoint could not start (port busy). */
  error: string | null;
}

export function claudeHooksStatus(): Promise<ClaudeHooksStatus> {
  return invoke("claude_hooks_status");
}

/** Adds or removes Escape's hooks in Claude Code's settings.json. */
export function claudeHooksSet(enabled: boolean): Promise<ClaudeHooksStatus> {
  return invoke("claude_hooks_set", { enabled });
}

/** Mirrors `shelf::ShelfItem`: a reference to a file parked on the island. */
export interface ShelfItem {
  id: number;
  path: string;
  name: string;
  isDir: boolean;
  /** Bytes; null for folders and missing files. */
  size: number | null;
  /** The original was moved or deleted. */
  missing: boolean;
  /** Icon or thumbnail; load it via `convertFileSrc`. */
  imagePath: string | null;
  addedAt: number;
}

export function shelfList(): Promise<ShelfItem[]> {
  return invoke("shelf_list");
}

export function shelfRemove(ids: number[]): Promise<void> {
  return invoke("shelf_remove", { ids });
}

export function shelfClear(): Promise<void> {
  return invoke("shelf_clear");
}

export function shelfOpen(id: number): Promise<void> {
  return invoke("shelf_open", { id });
}

/** Starts a system drag of the items; resolves `true` if they were dropped somewhere. */
export function shelfDragOut(ids: number[]): Promise<boolean> {
  return invoke("shelf_drag_out", { ids });
}

/** Mirrors `notes::Note`. */
export interface Note {
  id: number;
  text: string;
  /** Unix ms, or null for no reminder. */
  remindAt: number | null;
  reminded: boolean;
  done: boolean;
  createdAt: number;
}

export function notesList(): Promise<Note[]> {
  return invoke("notes_list");
}

export function notesAdd(text: string, remindAt: number | null): Promise<Note> {
  return invoke("notes_add", { text, remindAt });
}

export function notesSetDone(id: number, done: boolean): Promise<void> {
  return invoke("notes_set_done", { id, done });
}

export function notesDelete(id: number): Promise<void> {
  return invoke("notes_delete", { id });
}

/** Mirrors `notes::TimerState`. */
export interface TimerState {
  /** 0 = no timer. */
  durationMs: number;
  /** Unix ms when it ends; null while paused or idle. */
  endsAt: number | null;
  pausedRemainingMs: number | null;
}

export function timerGet(): Promise<TimerState> {
  return invoke("timer_get");
}

export function timerStart(durationMs: number): Promise<TimerState> {
  return invoke("timer_start", { durationMs });
}

export function timerControl(action: "pause" | "resume" | "cancel"): Promise<TimerState> {
  return invoke("timer_control", { action });
}

/** Stops the repeating "time's up" sound. */
export function timerSilence(): Promise<void> {
  return invoke("timer_silence");
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

/** Mirrors `indicators::Sensors`: apps using each device right now. */
export interface Sensors {
  microphone: string[];
  camera: string[];
}

/** Mirrors `indicators::Indicators`. */
export interface Indicators {
  sensors: Sensors;
  capsLock: boolean;
}

export function indicatorsGet(): Promise<Indicators> {
  return invoke("indicators_get");
}
