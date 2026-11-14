/**
 * Typed listeners for events emitted by Rust (PLAN.md §4.5). Each returns an
 * unsubscribe function suitable for a `useEffect` cleanup.
 */
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { Battery } from "./commands";

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
