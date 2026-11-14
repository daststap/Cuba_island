import { useEffect } from "react";
import { onEdgePush, onHover, onVisibility } from "../ipc/events";
import { createHoverIntent } from "./hoverIntent";
import { useIslandStore } from "./store";

/** Wires Rust island events and the Esc key to the island state machine. */
export function useIslandEvents(): void {
  useEffect(() => {
    const { dispatch } = useIslandStore.getState();
    const intent = createHoverIntent(() => useIslandStore.getState().state, dispatch);

    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") dispatch({ type: "collapse" });
    };
    window.addEventListener("keydown", onKeyDown);

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
    ];

    return () => {
      window.removeEventListener("keydown", onKeyDown);
      unsubscribe.forEach((fn) => {
        fn();
      });
      intent.dispose();
    };
  }, []);
}
