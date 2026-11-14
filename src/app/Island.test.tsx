import { act, cleanup, render, screen } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ru } from "../i18n/ru";
import { Island } from "./Island";
import { initialIslandState } from "./islandMachine";
import { useIslandStore } from "./store";

describe("Island", () => {
  beforeEach(() => {
    useIslandStore.setState({ state: initialIslandState });
    mockIPC(
      (cmd) => {
        if (cmd === "get_battery") return { percent: 64, charging: false, pluggedIn: false };
        return null;
      },
      { shouldMockEvents: true },
    );
    vi.stubGlobal(
      "ResizeObserver",
      class {
        observe() {}
        disconnect() {}
      },
    );
  });

  afterEach(() => {
    // Unmount (and unlisten) while the IPC mocks still exist.
    cleanup();
    clearMocks();
    vi.unstubAllGlobals();
    vi.useRealTimers();
  });

  it("renders the compact pill with the battery", async () => {
    render(<Island />);
    expect(screen.getByRole("status", { name: ru.appName })).toBeInTheDocument();
    expect(await screen.findByRole("img", { name: ru.battery.label(64) })).toBeInTheDocument();
  });

  it("expands on edge push and shows the tabs", async () => {
    render(<Island />);
    await act(async () => {
      await emit("island://edge-push");
    });
    expect(await screen.findByRole("tablist", { name: ru.tabsLabel })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: ru.tabs.home })).toHaveAttribute(
      "aria-selected",
      "true",
    );
  });

  it("collapses on Escape", async () => {
    useIslandStore.setState({ state: { mode: "expanded", tab: "home" } });
    render(<Island />);
    act(() => {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    });
    expect(useIslandStore.getState().state).toEqual({ mode: "compact" });
    expect(await screen.findByRole("status", { name: ru.appName })).toBeInTheDocument();
  });
});
