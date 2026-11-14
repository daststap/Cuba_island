import { useEffect } from "react";
import { ru } from "../i18n/ru";
import { hotkeysStatus, reportError, setErrorSink, type HotkeyStatus } from "../ipc/commands";
import {
  onClipboardHotkey,
  onEdgePush,
  onHover,
  onLauncherHotkey,
  onVisibility,
} from "../ipc/events";
import { createHoverIntent } from "./hoverIntent";
import type { Tab } from "./islandMachine";
import { useIslandStore } from "./store";

/** A toast text if some hotkey is missing or replaced by its fallback. */
export function hotkeyWarning(status: HotkeyStatus): string | null {
  if (!status.launcher) return ru.hotkeys.launcherUnavailable;
  if (status.launcherFallback) return ru.hotkeys.launcherFallback(status.launcher);
  if (!status.clipboard) return ru.hotkeys.clipboardUnavailable;
  return null;
}

/** Wires Rust island events, hotkeys, focus and the Esc key to the island state. */
export function useIslandEvents(): void {
  useEffect(() => {
    const store = useIslandStore;
    const { dispatch } = store.getState();
    const intent = createHoverIntent(
      () => store.getState().state,
      dispatch,
      () => store.getState().inputMode,
    );

    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") dispatch({ type: "collapse" });
    };
    // The user clicked into another app while typing here: close without
    // stealing focus back (Rust only restores focus if the island still has it).
    const onBlur = () => {
      if (store.getState().inputMode) dispatch({ type: "collapse" });
    };
    window.addEventListener("keydown", onKeyDown);
    window.addEventListener("blur", onBlur);
    // Command errors become toasts in the island (PLAN.md §4.5).
    setErrorSink((message) => {
      store.getState().showToast(message);
    });

    const unsubscribe = [
      onHover(({ inside }) => {
        intent.pointer(inside);
      }),
      onEdgePush(() => {
        intent.edgePush();
      }),
      onVisibility(({ visible }) => {
        intent.dispose();
        dispatch({ type: visible ? "show" : "hide" });
      }),
      onClipboardHotkey(() => {
        openByHotkey("clipboard");
      }),
      onLauncherHotkey(() => {
        openByHotkey("launcher");
      }),
    ];

    /** Opens a tab with keyboard focus; the same hotkey again closes it. */
    function openByHotkey(tab: Tab) {
      const { state, inputMode } = store.getState();
      if (state.mode === "expanded" && state.tab === tab && inputMode) {
        dispatch({ type: "collapse" });
        return;
      }
      intent.dispose();
      // Rust has already given the island focus.
      store.setState({ inputMode: true });
      dispatch({ type: "expand", tab });
    }

    // Tell the user once if a hotkey could not be registered (PLAN.md stage 4).
    hotkeysStatus()
      .then((status) => {
        const message = hotkeyWarning(status);
        if (message) store.getState().showToast(message);
      })
      .catch((error: unknown) => {
        reportError("hotkeys_status", error);
      });

    return () => {
      setErrorSink(undefined);
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("blur", onBlur);
      unsubscribe.forEach((fn) => {
        fn();
      });
      intent.dispose();
    };
  }, []);
}
