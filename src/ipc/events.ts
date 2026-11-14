/**
 * Typed listeners for events emitted by Rust (PLAN.md §4.5). Each returns an
 * unsubscribe function suitable for a `useEffect` cleanup.
 */
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  Battery,
  ClaudeSession,
  ClipItem,
  MediaState,
  Note,
  Settings,
  TimerState,
} from "./commands";

export interface HoverPayload {
  inside: boolean;
}

export interface VisibilityPayload {
  visible: boolean;
}

/** Event name → payload, mirroring the `emit` calls in Rust. */
interface EventMap {
  "island://hover": HoverPayload;
  "island://edge-push": null;
  "island://visibility": VisibilityPayload;
  "system://battery": Battery | null;
  "media://changed": MediaState | null;
  "clipboard://added": ClipItem;
  "hotkey://clipboard": null;
  "hotkey://launcher": null;
  "settings://changed": Settings;
  "tray://settings": null;
  "claude://status": ClaudeSession[];
  "claude://attention": ClaudeSession | null;
  "shelf://changed": null;
  "notes://changed": null;
  "notes://reminder": Note;
  "notes://timer": TimerState;
  "notes://timer-done": null;
}

function subscribe<K extends keyof EventMap>(
  event: K,
  handler: (payload: EventMap[K]) => void,
): () => void {
  let unlisten: UnlistenFn | undefined;
  let disposed = false;
  listen<EventMap[K]>(event, (e) => {
    handler(e.payload);
  })
    .then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    })
    .catch((error: unknown) => {
      console.error(`[listen ${event}]`, error);
    });
  return () => {
    disposed = true;
    unlisten?.();
  };
}

export const onHover = (handler: (p: HoverPayload) => void) => subscribe("island://hover", handler);

export const onEdgePush = (handler: () => void) => subscribe("island://edge-push", handler);

export const onVisibility = (handler: (p: VisibilityPayload) => void) =>
  subscribe("island://visibility", handler);

export const onBattery = (handler: (p: Battery | null) => void) =>
  subscribe("system://battery", handler);

export const onMediaChanged = (handler: (p: MediaState | null) => void) =>
  subscribe("media://changed", handler);

export const onClipAdded = (handler: (p: ClipItem) => void) =>
  subscribe("clipboard://added", handler);

export const onClipboardHotkey = (handler: () => void) => subscribe("hotkey://clipboard", handler);

export const onLauncherHotkey = (handler: () => void) => subscribe("hotkey://launcher", handler);

export const onSettingsChanged = (handler: (s: Settings) => void) =>
  subscribe("settings://changed", handler);

export const onTraySettings = (handler: () => void) => subscribe("tray://settings", handler);

export const onClaudeStatus = (handler: (sessions: ClaudeSession[]) => void) =>
  subscribe("claude://status", handler);

export const onClaudeAttention = (handler: (session: ClaudeSession | null) => void) =>
  subscribe("claude://attention", handler);

export const onShelfChanged = (handler: () => void) => subscribe("shelf://changed", handler);

export const onNotesChanged = (handler: () => void) => subscribe("notes://changed", handler);

export const onReminder = (handler: (note: Note) => void) => subscribe("notes://reminder", handler);

export const onTimer = (handler: (state: TimerState) => void) =>
  subscribe("notes://timer", handler);

export const onTimerDone = (handler: () => void) => subscribe("notes://timer-done", handler);
