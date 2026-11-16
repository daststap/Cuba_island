import { create } from "zustand";
import { reportError, setInputMode } from "../ipc/commands";
import {
  initialIslandState,
  islandReducer,
  type IslandEvent,
  type IslandState,
  type PeekKind,
} from "./islandMachine";

/** How long a peek stays open (PLAN.md §4.6). */
export const PEEK_DURATION_MS: Record<PeekKind, number> = {
  media: 3000,
  clipboard: 1500,
  toast: 4000,
  claude: 6000,
  // Reminders and finished timers stay until the user reacts.
  alert: Number.POSITIVE_INFINITY,
  caps: 1500,
  download: 4000,
};

interface IslandStore {
  state: IslandState;
  /** The island has keyboard focus (search field / hotkey). */
  inputMode: boolean;
  /** A short message (errors, hotkey conflicts): a peek when compact, a banner when open. */
  toast: string | null;
  dispatch: (event: IslandEvent) => void;
  /** Shows a peek; a new peek restarts the timer. */
  peek: (kind: PeekKind) => void;
  showToast: (message: string) => void;
  /** Asks Rust for keyboard focus (remembering the user's window). */
  enterInput: () => void;
  /** Gives keyboard focus back to the user's window. */
  leaveInput: () => void;
}

let peekTimer: ReturnType<typeof setTimeout> | undefined;
let toastTimer: ReturnType<typeof setTimeout> | undefined;

export const useIslandStore = create<IslandStore>()((set, get) => ({
  state: initialIslandState,
  inputMode: false,
  toast: null,
  showToast: (message) => {
    set({ toast: message });
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => {
      set({ toast: null });
    }, PEEK_DURATION_MS.toast);
    get().peek("toast");
  },
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
    const duration = PEEK_DURATION_MS[kind];
    if (!Number.isFinite(duration)) return;
    peekTimer = setTimeout(() => {
      dispatch({ type: "endPeek" });
    }, duration);
  },
  enterInput: () => {
    if (get().inputMode) return;
    set({ inputMode: true });
    setInputMode(true).catch((error: unknown) => {
      reportError("set_input_mode", error);
    });
  },
  leaveInput: () => {
    if (!get().inputMode) return;
    set({ inputMode: false });
    setInputMode(false).catch((error: unknown) => {
      reportError("set_input_mode", error);
    });
  },
}));

// Whenever the panel closes, keyboard focus goes back to where the user was.
useIslandStore.subscribe((store, prev) => {
  if (prev.state.mode === "expanded" && store.state.mode !== "expanded") store.leaveInput();
});
