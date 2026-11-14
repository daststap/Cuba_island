import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { initialIslandState } from "./islandMachine";
import { PEEK_DURATION_MS, useIslandStore } from "./store";

const mode = () => useIslandStore.getState().state.mode;

describe("island store peek", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    useIslandStore.setState({ state: initialIslandState });
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("returns to compact after the peek duration", () => {
    useIslandStore.getState().peek("media");
    expect(mode()).toBe("peek");
    vi.advanceTimersByTime(PEEK_DURATION_MS.media - 1);
    expect(mode()).toBe("peek");
    vi.advanceTimersByTime(1);
    expect(mode()).toBe("compact");
  });

  it("restarts the timer on a new peek", () => {
    const { peek } = useIslandStore.getState();
    peek("media");
    vi.advanceTimersByTime(2000);
    peek("media");
    vi.advanceTimersByTime(2000);
    expect(mode()).toBe("peek");
    vi.advanceTimersByTime(1000);
    expect(mode()).toBe("compact");
  });

  it("uses a shorter peek for copies and lets it replace a media peek", () => {
    const { peek } = useIslandStore.getState();
    peek("media");
    peek("clipboard");
    expect(useIslandStore.getState().state).toEqual({ mode: "peek", kind: "clipboard" });
    vi.advanceTimersByTime(PEEK_DURATION_MS.clipboard);
    expect(mode()).toBe("compact");
  });

  it("does not peek over the open panel", () => {
    useIslandStore.setState({ state: { mode: "expanded", tab: "home" } });
    useIslandStore.getState().peek("media");
    vi.advanceTimersByTime(PEEK_DURATION_MS.media);
    expect(mode()).toBe("expanded");
  });
});
