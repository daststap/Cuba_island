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
  /** `app:…`, `cmd:…`, `calc:…`, `web:…` or `ask:…` — passed back to `launcherRun`. */
  id: string;
  kind: "app" | "command" | "calc" | "web" | "ask";
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
  screenshot: string | null;
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
  screenshotHotkey: string;
  historyLimit: number;
  copyPeek: boolean;
  /** Orange/green dots while the microphone/camera is in use. */
  privacyDots: boolean;
  /** A short peek when Caps Lock is toggled. */
  capsPeek: boolean;
  /** Downloads in the pill and on the home tab. */
  downloads: boolean;
  /** One of CHAT_MODELS. */
  chatModel: string;
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
  // Also into the app log; never let reporting itself fail loudly.
  try {
    invoke("report_error", { context, message }).catch(() => undefined);
  } catch {
    // No Tauri (unit tests without IPC mocks).
  }
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

/** The island's button: folds the island, then freezes the screen under the cursor. */
export function screenshotStart(): Promise<void> {
  return invoke("screenshot_start");
}

/** The frozen monitor: 8-byte little-endian `[width, height]` header, then RGBA. */
export async function screenshotFrame(): Promise<ArrayBuffer> {
  const raw: unknown = await invoke("screenshot_frame");
  if (raw instanceof ArrayBuffer) return raw;
  // Depending on the IPC transport, raw responses can also arrive as a typed or plain array.
  const kind = Object.prototype.toString.call(raw);
  console.warn(`[screenshot_frame] got ${kind}`);
  invoke("report_error", { context: "screenshot_frame", message: `raw response as ${kind}` }).catch(
    () => undefined,
  );
  if (ArrayBuffer.isView(raw)) {
    return new Uint8Array(raw.buffer, raw.byteOffset, raw.byteLength).slice().buffer;
  }
  if (Array.isArray(raw)) return Uint8Array.from(raw as number[]).buffer;
  throw new Error(`unexpected frame: ${kind}`);
}

/** The overlay has drawn the picture and can be shown. */
export function screenshotReady(): Promise<void> {
  return invoke("screenshot_ready");
}

/** Copies the finished image (same format as the frame) and closes the overlay. */
export function screenshotCopy(image: Uint8Array): Promise<void> {
  return invoke("screenshot_copy", image);
}

/** Closes the overlay without copying. */
export function screenshotClose(): Promise<void> {
  return invoke("screenshot_close");
}

/** Mirrors `downloads::Download`. */
export interface Download {
  id: string;
  source: "browser" | "steam";
  name: string;
  received: number;
  /** null when unknown (browsers do not reveal it). */
  total: number | null;
  /** Bytes per second; null until measured. */
  speed: number | null;
}

/** Mirrors `downloads::Finished`. */
export interface FinishedDownload {
  source: "browser" | "steam";
  name: string;
  /** The file, for "show in folder" (browser downloads only). */
  path: string | null;
}

export function downloadsList(): Promise<Download[]> {
  return invoke("downloads_list");
}

/** Opens Explorer with the file selected. */
export function downloadsReveal(path: string): Promise<void> {
  return invoke("downloads_reveal", { path });
}

export type ChatProvider = "claude" | "gemini";

/** Models offered for the chat (mirrors `chat::MODELS`); the first of each is its default. */
export const CHAT_MODELS: Record<ChatProvider, readonly string[]> = {
  claude: ["claude-haiku-5-5", "claude-sonnet-5-5", "claude-opus-5-5"],
  gemini: ["gemini-3.5-flash-lite", "gemini-3.8-flash", "gemini-4-hydrogen"],
};

/** Mirrors `chat::ChatMessage`. */
export interface ChatMessage {
  role: "user" | "assistant";
  text: string;
}

/** Mirrors `chat::ChatState`. */
export interface ChatState {
  messages: ChatMessage[];
  streaming: boolean;
  /** Provider of the model chosen in settings. */
  provider: ChatProvider;
  /** A key for that provider is saved (the key itself never leaves Rust). */
  hasKey: boolean;
}

/** Mirrors `chat::ChatDone`. */
export interface ChatDone {
  error: string | null;
  refused: boolean;
  truncated: boolean;
}

export function chatGet(): Promise<ChatState> {
  return invoke("chat_get");
}

/** Sends a message; the reply streams as `chat://delta`, then `chat://done`. */
export function chatSend(text: string): Promise<ChatState> {
  return invoke("chat_send", { text });
}

export function chatStop(): Promise<void> {
  return invoke("chat_stop");
}

export function chatNew(): Promise<ChatState> {
  return invoke("chat_new");
}

/** Saves a provider's API key in Windows Credential Manager; an empty string removes it. */
export function chatSetKey(provider: ChatProvider, key: string): Promise<ChatState> {
  return invoke("chat_set_key", { provider, key });
}
