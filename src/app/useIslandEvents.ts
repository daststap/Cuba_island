import { useEffect } from "react";
import { onClipboardHotkey, onEdgePush, onHover, onVisibility } from "../ipc/events";
import { createHoverIntent } from "./hoverIntent";
import { useIslandStore } from "./store";

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
        const { state, inputMode } = store.getState();
        // Pressing the hotkey again closes the history.
        if (state.mode === "expanded" && state.tab === "clipboard" && inputMode) {
          dispatch({ type: "collapse" });
          return;
        }
        intent.dispose();
        // Rust has already given the island focus.
        store.setState({ inputMode: true });
        dispatch({ type: "expand", tab: "clipboard" });
      }),
    ];

    return () => {
      window.removeEventListener("keydown", onKeyDown);
      window.removeEventListener("blur", onBlur);
      unsubscribe.forEach((fn) => {
        fn();
      });
      intent.dispose();
    };
  }, []);
}
