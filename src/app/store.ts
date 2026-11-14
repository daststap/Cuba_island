import { create } from "zustand";
import {
  initialIslandState,
  islandReducer,
  type IslandEvent,
  type IslandState,
  type PeekKind,
} from "./islandMachine";

/** How long a peek stays open (PLAN.md §4.6). */
export const PEEK_DURATION_MS = 3000;

interface IslandStore {
  state: IslandState;
  dispatch: (event: IslandEvent) => void;
  /** Shows a peek for `PEEK_DURATION_MS`; a new peek restarts the timer. */
  peek: (kind: PeekKind) => void;
}

let peekTimer: ReturnType<typeof setTimeout> | undefined;

export const useIslandStore = create<IslandStore>()((set, get) => ({
  state: initialIslandState,
  dispatch: (event) => {
    set((store) => {
      const next = islandReducer(store.state, event);
      return next === store.state ? store : { state: next };
    });
  },
  peek: (kind) => {
    const { dispatch } = get();
    dispatch({ type: "peek", kind });
    if (get().state.mode !== "peek") return;
    clearTimeout(peekTimer);
    peekTimer = setTimeout(() => {
      dispatch({ type: "endPeek" });
    }, PEEK_DURATION_MS);
  },
}));
