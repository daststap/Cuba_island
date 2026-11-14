import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useMediaStore } from "../features/media/store";
import { ru } from "../i18n/ru";
import type { ClipItem, MediaState } from "../ipc/commands";
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

const CLIPS: ClipItem[] = [
  {
    id: 1,
    kind: "text",
    text: "первый текст",
    thumbPath: null,
    pinned: false,
    createdAt: Date.now(),
    usedAt: Date.now(),
  },
  {
    id: 2,
    kind: "text",
    text: "второй текст",
    thumbPath: null,
    pinned: true,
    createdAt: Date.now(),
    usedAt: Date.now() - 120_000,
  },
];

let mediaOnStart: MediaState | null = null;
const commands: unknown[] = [];
const calls: { cmd: string; args: unknown }[] = [];

describe("Island", () => {
  beforeEach(() => {
    useIslandStore.setState({ state: initialIslandState, inputMode: false });
    useMediaStore.setState({ media: null });
    mediaOnStart = null;
    commands.length = 0;
    calls.length = 0;
    mockIPC(
      (cmd, args) => {
        calls.push({ cmd, args });
        if (cmd === "get_battery") return { percent: 64, charging: false, pluggedIn: false };
        if (cmd === "media_get_state") return mediaOnStart;
        if (cmd === "media_command") commands.push(args);
        if (cmd === "clipboard_list") return CLIPS;
        return null;
      },
      { shouldMockEvents: true },
    );
    Element.prototype.scrollIntoView = () => {};
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

  describe("clipboard", () => {
    const openByHotkey = async () => {
      render(<Island />);
      await act(async () => {
        await emit("hotkey://clipboard");
      });
      return screen.findByRole("searchbox", { name: ru.clipboard.search });
    };

    it("opens the history with the search focused on Ctrl+Alt+V", async () => {
      const search = await openByHotkey();
      expect(useIslandStore.getState().state).toEqual({ mode: "expanded", tab: "clipboard" });
      expect(useIslandStore.getState().inputMode).toBe(true);
      expect(search).toHaveFocus();
      expect(await screen.findByText("первый текст")).toBeInTheDocument();
      expect(screen.getByRole("img", { name: ru.clipboard.pinned })).toBeInTheDocument();
    });

    it("pastes the selected clip with Enter after moving down", async () => {
      const search = await openByHotkey();
      await screen.findByText("второй текст");
      fireEvent.keyDown(search, { key: "ArrowDown" });
      fireEvent.keyDown(search, { key: "Enter" });
      expect(useIslandStore.getState().state.mode).toBe("compact");
      await vi.waitFor(() => {
        expect(calls).toContainEqual({ cmd: "clipboard_paste", args: { id: 2 } });
      });
      // Rust returns focus itself; the frontend must not ask for it separately.
      expect(calls.some((c) => c.cmd === "set_input_mode")).toBe(false);
    });

    it("pins and deletes with the keyboard", async () => {
      const search = await openByHotkey();
      await screen.findByText("первый текст");
      fireEvent.keyDown(search, { key: "p", code: "KeyP", ctrlKey: true });
      fireEvent.keyDown(search, { key: "Delete" });
      await vi.waitFor(() => {
        expect(calls).toContainEqual({ cmd: "clipboard_pin", args: { id: 1, pinned: true } });
        expect(calls).toContainEqual({ cmd: "clipboard_delete", args: { id: 1 } });
      });
    });

    it("copies on click and gives focus back on close", async () => {
      await openByHotkey();
      fireEvent.click(await screen.findByText("первый текст"));
      await vi.waitFor(() => {
        expect(calls).toContainEqual({ cmd: "clipboard_copy", args: { id: 1 } });
        expect(useIslandStore.getState().state.mode).toBe("compact");
      });
      expect(calls).toContainEqual({ cmd: "set_input_mode", args: { enabled: false } });
    });

    it("closes on a second hotkey press", async () => {
      await openByHotkey();
      await act(async () => {
        await emit("hotkey://clipboard");
      });
      expect(useIslandStore.getState().state.mode).toBe("compact");
      expect(useIslandStore.getState().inputMode).toBe(false);
    });

    it("asks for focus when the search field is clicked in hover mode", async () => {
      useIslandStore.setState({ state: { mode: "expanded", tab: "clipboard" } });
      render(<Island />);
      fireEvent.mouseDown(await screen.findByRole("searchbox", { name: ru.clipboard.search }));
      expect(useIslandStore.getState().inputMode).toBe(true);
      expect(calls).toContainEqual({ cmd: "set_input_mode", args: { enabled: true } });
    });

    it("peeks 'Скопировано' with the start of the text", async () => {
      render(<Island />);
      await act(async () => {
        await emit("clipboard://added", { ...CLIPS[0], text: "только что скопированный текст" });
      });
      expect(useIslandStore.getState().state).toEqual({ mode: "peek", kind: "clipboard" });
      expect(await screen.findByText(ru.clipboard.copied)).toBeInTheDocument();
      expect(screen.getByText("только что скопированный текст")).toBeInTheDocument();
    });
  });
});
