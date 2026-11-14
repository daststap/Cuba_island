import { describe, expect, it } from "vitest";
import { initialIslandState, islandReducer, type IslandState } from "./islandMachine";

const compact: IslandState = { mode: "compact" };
const home: IslandState = { mode: "expanded", tab: "home" };

describe("islandReducer", () => {
  it("starts compact", () => {
    expect(initialIslandState).toEqual(compact);
  });

  it("expands to home by default and to a requested tab", () => {
    expect(islandReducer(compact, { type: "expand" })).toEqual(home);
    expect(islandReducer(compact, { type: "expand", tab: "clipboard" })).toEqual({
      mode: "expanded",
      tab: "clipboard",
    });
  });

  it("keeps the current tab when expanding again without a tab", () => {
    const clipboard: IslandState = { mode: "expanded", tab: "clipboard" };
    expect(islandReducer(clipboard, { type: "expand" })).toBe(clipboard);
  });

  it("collapses only from expanded", () => {
    expect(islandReducer(home, { type: "collapse" })).toEqual(compact);
    expect(islandReducer(compact, { type: "collapse" })).toBe(compact);
  });

  it("switches tabs only while expanded", () => {
    expect(islandReducer(home, { type: "selectTab", tab: "settings" })).toEqual({
      mode: "expanded",
      tab: "settings",
    });
    expect(islandReducer(compact, { type: "selectTab", tab: "settings" })).toBe(compact);
  });

  it("peeks only from compact and ends back in compact", () => {
    const peek = islandReducer(compact, { type: "peek", kind: "media" });
    expect(peek).toEqual({ mode: "peek", kind: "media" });
    expect(islandReducer(peek, { type: "endPeek" })).toEqual(compact);
    expect(islandReducer(home, { type: "peek", kind: "media" })).toBe(home);
    expect(islandReducer({ mode: "hidden" }, { type: "peek", kind: "media" })).toEqual({
      mode: "hidden",
    });
  });

  it("expands from a peek, and endPeek does not close the panel", () => {
    const peek: IslandState = { mode: "peek", kind: "media" };
    const expanded = islandReducer(peek, { type: "expand" });
    expect(expanded).toEqual(home);
    expect(islandReducer(expanded, { type: "endPeek" })).toBe(expanded);
    expect(islandReducer(peek, { type: "collapse" })).toEqual(compact);
  });

  it("ignores expand while hidden and returns to compact on show", () => {
    const hidden = islandReducer(home, { type: "hide" });
    expect(hidden).toEqual({ mode: "hidden" });
    expect(islandReducer(hidden, { type: "expand" })).toBe(hidden);
    expect(islandReducer(hidden, { type: "show" })).toEqual(compact);
    expect(islandReducer(home, { type: "show" })).toBe(home);
  });
});
