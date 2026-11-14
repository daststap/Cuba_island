/**
 * Island states and transitions (PLAN.md §4.6). Pure: no timers, no IPC.
 * `peek` arrives in stage 2 together with the first event that needs it.
 */

export type Tab = "home" | "clipboard" | "launcher" | "settings";

export const TABS: readonly Tab[] = ["home", "clipboard", "launcher", "settings"];

export type IslandState = { mode: "hidden" } | { mode: "compact" } | { mode: "expanded"; tab: Tab };

export type IslandEvent =
  | { type: "expand"; tab?: Tab }
  | { type: "collapse" }
  | { type: "selectTab"; tab: Tab }
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
      return { mode: "expanded", tab: event.tab ?? "home" };
    case "collapse":
      return state.mode === "expanded" ? { mode: "compact" } : state;
    case "selectTab":
      return state.mode === "expanded" && state.tab !== event.tab
        ? { mode: "expanded", tab: event.tab }
        : state;
  }
}
