import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useMediaStore } from "../features/media/store";
import { ru } from "../i18n/ru";
import type { MediaState } from "../ipc/commands";
import { Island } from "./Island";
import { initialIslandState } from "./islandMachine";
import { useIslandStore } from "./store";

const SONG: MediaState = {
  trackId: "t1",
  title: "Первая песня",
  artist: "Исполнитель",
  album: "Альбом",
  appId: "Spotify.exe",
  isPlaying: true,
  positionMs: 30_000,
  durationMs: 180_000,
  positionUpdatedAt: Date.now(),
  cover: null,
};

let mediaOnStart: MediaState | null = null;
const commands: unknown[] = [];

describe("Island", () => {
  beforeEach(() => {
    useIslandStore.setState({ state: initialIslandState });
    useMediaStore.setState({ media: null });
    mediaOnStart = null;
    commands.length = 0;
    mockIPC(
      (cmd, args) => {
        if (cmd === "get_battery") return { percent: 64, charging: false, pluggedIn: false };
        if (cmd === "media_get_state") return mediaOnStart;
        if (cmd === "media_command") commands.push(args);
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

  it("shows the playing track without a peek on startup", async () => {
    mediaOnStart = SONG;
    render(<Island />);
    expect(await screen.findByRole("img", { name: ru.media.cover })).toBeInTheDocument();
    expect(useIslandStore.getState().state.mode).toBe("compact");
  });

  it("peeks with title and artist when the track changes", async () => {
    render(<Island />);
    await act(async () => {
      await emit("media://changed", SONG);
    });
    expect(useIslandStore.getState().state).toEqual({ mode: "peek", kind: "media" });
    expect(await screen.findByText(SONG.title)).toBeInTheDocument();
    expect(screen.getByText(SONG.artist)).toBeInTheDocument();
  });

  it("shows the player on the home tab and sends commands", async () => {
    mediaOnStart = SONG;
    useIslandStore.setState({ state: { mode: "expanded", tab: "home" } });
    render(<Island />);
    const pause = await screen.findByRole("button", { name: ru.media.pause });
    expect(screen.getByRole("progressbar", { name: ru.media.progress })).toBeInTheDocument();
    fireEvent.click(pause);
    fireEvent.click(screen.getByRole("button", { name: ru.media.next }));
    await vi.waitFor(() => {
      expect(commands).toEqual([{ cmd: "toggle" }, { cmd: "next" }]);
    });
  });

  it("hides the player when the session ends", async () => {
    mediaOnStart = SONG;
    render(<Island />);
    await screen.findByRole("img", { name: ru.media.cover });
    await act(async () => {
      await emit("media://changed", null);
    });
    expect(screen.queryByRole("img", { name: ru.media.cover })).not.toBeInTheDocument();
  });
});
