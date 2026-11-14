import { create } from "zustand";
import {
  initialIslandState,
  islandReducer,
  type IslandEvent,
  type IslandState,
} from "./islandMachine";

interface IslandStore {
  state: IslandState;
  dispatch: (event: IslandEvent) => void;
}

export const useIslandStore = create<IslandStore>()((set) => ({
  state: initialIslandState,
  dispatch: (event) => {
    set((store) => {
      const next = islandReducer(store.state, event);
      return next === store.state ? store : { state: next };
    });
  },
}));
