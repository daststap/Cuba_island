/**
 * Turns raw hover / edge-push signals from Rust into expand / collapse decisions:
 * hovering expands after a short delay (so a cursor passing by towards browser
 * tabs does not open the island), leaving collapses after 300 ms, pushing the
 * cursor against the top edge expands immediately (PLAN.md §4.6).
 */
import type { IslandEvent, IslandState } from "./islandMachine";

export const HOVER_EXPAND_DELAY_MS = 200;
export const LEAVE_COLLAPSE_DELAY_MS = 300;

export interface HoverIntent {
  pointer: (inside: boolean) => void;
  edgePush: () => void;
  dispose: () => void;
}

/**
 * @param isPinnedOpen while true (e.g. the user is typing in the island), leaving
 *   the island does not collapse it.
 */
export function createHoverIntent(
  getState: () => IslandState,
  dispatch: (event: IslandEvent) => void,
  isPinnedOpen: () => boolean = () => false,
): HoverIntent {
  let timer: ReturnType<typeof setTimeout> | undefined;
  let inside = false;

  const cancel = () => {
    if (timer !== undefined) clearTimeout(timer);
    timer = undefined;
  };
  const schedule = (event: IslandEvent, delay: number) => {
    cancel();
    timer = setTimeout(() => {
      timer = undefined;
      dispatch(event);
    }, delay);
  };

  return {
    pointer(next) {
      inside = next;
      const mode = getState().mode;
      if (inside) {
        // Hovering a peek opens the panel as well.
        if (mode === "compact" || mode === "peek") {
          schedule({ type: "expand" }, HOVER_EXPAND_DELAY_MS);
        } else cancel();
      } else if (mode === "expanded" && !isPinnedOpen()) {
        schedule({ type: "collapse" }, LEAVE_COLLAPSE_DELAY_MS);
      } else {
        cancel();
      }
    },
    edgePush() {
      cancel();
      dispatch({ type: "expand" });
      // The panel may not end up under the cursor; collapse unless it gets hovered.
      if (!inside) schedule({ type: "collapse" }, LEAVE_COLLAPSE_DELAY_MS);
    },
    dispose: cancel,
  };
}
