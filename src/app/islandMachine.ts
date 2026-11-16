/**
 * Island states and transitions (PLAN.md §4.6). Pure: no timers, no IPC.
 */

export type Tab = "home" | "notes" | "chat" | "shelf" | "clipboard" | "launcher" | "settings";

export const TABS: readonly Tab[] = [
  "home",
  "notes",
  "chat",
  "shelf",
  "clipboard",
  "launcher",
  "settings",
];

/**
 * What a peek is about: a track change, a fresh copy, a message, Claude Code waiting,
 * or an alert (reminder due / timer done) that stays until the user reacts.
 */
export type PeekKind = "media" | "clipboard" | "toast" | "claude" | "alert" | "caps" | "download";

export type IslandState =
  | { mode: "hidden" }
  | { mode: "compact" }
  | { mode: "peek"; kind: PeekKind }
  | { mode: "expanded"; tab: Tab };

export type IslandEvent =
  | { type: "expand"; tab?: Tab }
  | { type: "collapse" }
  | { type: "selectTab"; tab: Tab }
  | { type: "peek"; kind: PeekKind }
  | { type: "endPeek" }
  | { type: "hide" }
  | { type: "show" };

export const initialIslandState: IslandState = { mode: "compact" };

export function islandReducer(state: IslandState, event: IslandEvent): IslandState {
  switch (event.type) {
    case "hide":
      return state.mode === "hidden" ? state : { mode: "hidden" };
    case "show":
      return state.mode === "hidden" ? { mode: "compact" } : state;
    case "expand":
      if (state.mode === "hidden") return state;
      if (state.mode === "expanded") {
        return event.tab && event.tab !== state.tab ? { mode: "expanded", tab: event.tab } : state;
      }
      // Opening an alert peek leads to the notes and timer it is about.
      return {
        mode: "expanded",
        tab: event.tab ?? (state.mode === "peek" && state.kind === "alert" ? "notes" : "home"),
      };
    case "collapse":
      return state.mode === "expanded" || state.mode === "peek" ? { mode: "compact" } : state;
    case "selectTab":
      return state.mode === "expanded" && state.tab !== event.tab
        ? { mode: "expanded", tab: event.tab }
        : state;
    case "peek":
      // A peek never interrupts the open panel and never shows while hidden.
      if (state.mode === "compact") return { mode: "peek", kind: event.kind };
      // A newer peek of another kind replaces the current one.
      if (state.mode === "peek" && state.kind !== event.kind) {
        return { mode: "peek", kind: event.kind };
      }
      return state;
    case "endPeek":
      return state.mode === "peek" ? { mode: "compact" } : state;
  }
}
