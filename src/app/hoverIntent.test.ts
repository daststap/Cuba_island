import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createHoverIntent, HOVER_EXPAND_DELAY_MS, LEAVE_COLLAPSE_DELAY_MS } from "./hoverIntent";
import {
  initialIslandState,
  islandReducer,
  type IslandEvent,
  type IslandState,
} from "./islandMachine";

function setup() {
  let state: IslandState = initialIslandState;
  const dispatch = vi.fn((event: IslandEvent) => {
    state = islandReducer(state, event);
  });
  const intent = createHoverIntent(() => state, dispatch);
  return { intent, dispatch, mode: () => state.mode };
}

describe("hover intent", () => {
  beforeEach(() => {
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("expands after the hover delay", () => {
    const { intent, mode } = setup();
    intent.pointer(true);
    vi.advanceTimersByTime(HOVER_EXPAND_DELAY_MS - 1);
    expect(mode()).toBe("compact");
    vi.advanceTimersByTime(1);
    expect(mode()).toBe("expanded");
  });

  it("does not expand when the cursor just passes by", () => {
    const { intent, dispatch } = setup();
    intent.pointer(true);
    vi.advanceTimersByTime(100);
    intent.pointer(false);
    vi.advanceTimersByTime(1000);
    expect(dispatch).not.toHaveBeenCalled();
  });

  it("collapses 300 ms after leaving, unless the cursor returns", () => {
    const { intent, mode } = setup();
    intent.pointer(true);
    vi.advanceTimersByTime(HOVER_EXPAND_DELAY_MS);

    intent.pointer(false);
    vi.advanceTimersByTime(LEAVE_COLLAPSE_DELAY_MS - 50);
    intent.pointer(true);
    vi.advanceTimersByTime(1000);
    expect(mode()).toBe("expanded");

    intent.pointer(false);
    vi.advanceTimersByTime(LEAVE_COLLAPSE_DELAY_MS);
    expect(mode()).toBe("compact");
  });

  it("stays open on leave while pinned open (typing)", () => {
    let state: IslandState = { mode: "expanded", tab: "clipboard" };
    let pinned = true;
    const intent = createHoverIntent(
      () => state,
      (event) => {
        state = islandReducer(state, event);
      },
      () => pinned,
    );
    intent.pointer(false);
    vi.advanceTimersByTime(1000);
    expect(state.mode).toBe("expanded");
    pinned = false;
    intent.pointer(false);
    vi.advanceTimersByTime(LEAVE_COLLAPSE_DELAY_MS);
    expect(state.mode).toBe("compact");
  });

  it("expands immediately on edge push and collapses if never hovered", () => {
    const { intent, mode } = setup();
    intent.edgePush();
    expect(mode()).toBe("expanded");
    vi.advanceTimersByTime(LEAVE_COLLAPSE_DELAY_MS);
    expect(mode()).toBe("compact");
  });

  it("stays expanded after edge push once the panel is hovered", () => {
    const { intent, mode } = setup();
    intent.edgePush();
    intent.pointer(true);
    vi.advanceTimersByTime(1000);
    expect(mode()).toBe("expanded");
  });
});
