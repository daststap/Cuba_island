import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { emit } from "@tauri-apps/api/event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useMediaStore } from "../features/media/store";
import { ru } from "../i18n/ru";
import { useSettingsStore } from "../features/settings/store";
import type { ClipItem, HotkeyStatus, LaunchResult, MediaState, Settings } from "../ipc/commands";
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

const DEFAULT_HOTKEYS: HotkeyStatus = {
  clipboard: "Ctrl+Alt+V",
  launcher: "Alt+Space",
  launcherFallback: false,
};
const RESULTS: LaunchResult[] = [
  { id: "calc:6", kind: "calc", title: "6", iconPath: null },
  { id: "app:Chrome", kind: "app", title: "Google Chrome", iconPath: null },
  { id: "cmd:lock", kind: "command", title: "", iconPath: null },
  { id: "web:2+2*2", kind: "web", title: "2+2*2", iconPath: null },
];

const DEFAULT_SETTINGS: Settings = {
  monitor: null,
  autostart: false,
  hideInFullscreen: true,
  hour12: false,
  clipboardHotkey: "Ctrl+Alt+V",
  launcherHotkey: "Alt+Space",
  historyLimit: 500,
  copyPeek: true,
  opacity: 1,
  onboarded: true,
};

let settings = DEFAULT_SETTINGS;
let hotkeys = DEFAULT_HOTKEYS;
let launcherResults = RESULTS;
let failRun = false;
let mediaOnStart: MediaState | null = null;
const commands: unknown[] = [];
const calls: { cmd: string; args: unknown }[] = [];

describe("Island", () => {
  beforeEach(() => {
    useIslandStore.setState({ state: initialIslandState, inputMode: false, toast: null });
    useMediaStore.setState({ media: null });
    hotkeys = DEFAULT_HOTKEYS;
    settings = DEFAULT_SETTINGS;
    useSettingsStore.setState({ settings: null, update: null });
    launcherResults = RESULTS;
    failRun = false;
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
        if (cmd === "hotkeys_status") return hotkeys;
        if (cmd === "settings_get") return settings;
        if (cmd === "settings_set") {
          settings = { ...settings, ...(args as { patch: Partial<Settings> }).patch };
          return settings;
        }
        if (cmd === "monitors_list") return [];
        if (cmd === "update_check") return null;
        if (cmd === "launcher_search") return launcherResults;
        if (cmd === "launcher_run" && failRun) {
          // Tauri rejects with the serialized AppError object, not an Error.
          // eslint-disable-next-line @typescript-eslint/prefer-promise-reject-errors
          return Promise.reject({ kind: "unavailable", message: "Не удалось" });
        }
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

  describe("launcher", () => {
    const openAndType = async (text: string) => {
      render(<Island />);
      await act(async () => {
        await emit("hotkey://launcher");
      });
      const input = await screen.findByRole("combobox", { name: ru.launcher.input });
      fireEvent.change(input, { target: { value: text } });
      await screen.findByText("= 6");
      return input;
    };

    it("opens on Alt+Space with the input focused and shows labelled results", async () => {
      const input = await openAndType("2+2*2");
      expect(input).toHaveFocus();
      expect(useIslandStore.getState().state).toEqual({ mode: "expanded", tab: "launcher" });
      expect(calls).toContainEqual({ cmd: "launcher_search", args: { query: "2+2*2" } });
      expect(screen.getByText("Google Chrome")).toBeInTheDocument();
      expect(screen.getByText(ru.launcher.commands["cmd:lock"] ?? "")).toBeInTheDocument();
      expect(screen.getByText(ru.launcher.webTitle("2+2*2"))).toBeInTheDocument();
    });

    it("launches the selected app with Enter without asking for focus back", async () => {
      const input = await openAndType("2+2*2");
      fireEvent.keyDown(input, { key: "ArrowDown" });
      fireEvent.keyDown(input, { key: "Enter" });
      expect(useIslandStore.getState().state.mode).toBe("compact");
      await vi.waitFor(() => {
        expect(calls).toContainEqual({ cmd: "launcher_run", args: { id: "app:Chrome" } });
      });
      expect(calls.some((c) => c.cmd === "set_input_mode")).toBe(false);
    });

    it("copies a calculator answer and returns focus", async () => {
      const input = await openAndType("2+2*2");
      fireEvent.keyDown(input, { key: "Enter" });
      await vi.waitFor(() => {
        expect(calls).toContainEqual({ cmd: "launcher_run", args: { id: "calc:6" } });
      });
      expect(calls).toContainEqual({ cmd: "set_input_mode", args: { enabled: false } });
    });

    it("shows a toast when a command fails", async () => {
      failRun = true;
      const input = await openAndType("2+2*2");
      fireEvent.keyDown(input, { key: "Enter" });
      await vi.waitFor(() => {
        expect(useIslandStore.getState().toast).toBe("Не удалось");
      });
      expect(useIslandStore.getState().state).toEqual({ mode: "peek", kind: "toast" });
      expect(await screen.findByRole("alert")).toHaveTextContent("Не удалось");
    });

    it("warns once when Alt+Space was taken", async () => {
      hotkeys = { ...DEFAULT_HOTKEYS, launcher: "Ctrl+Alt+Space", launcherFallback: true };
      render(<Island />);
      expect(await screen.findByRole("alert")).toHaveTextContent(
        ru.hotkeys.launcherFallback("Ctrl+Alt+Space"),
      );
    });
  });

  describe("settings", () => {
    const openSettings = async () => {
      render(<Island />);
      await act(async () => {
        await emit("tray://settings");
      });
      return screen.findByRole("switch", { name: ru.settings.autostart });
    };

    it("opens from the tray with focus and saves toggles", async () => {
      const autostart = await openSettings();
      expect(useIslandStore.getState().state).toEqual({ mode: "expanded", tab: "settings" });
      expect(calls).toContainEqual({ cmd: "set_input_mode", args: { enabled: true } });
      fireEvent.click(autostart);
      await vi.waitFor(() => {
        expect(calls).toContainEqual({ cmd: "settings_set", args: { patch: { autostart: true } } });
      });
      expect(await screen.findByRole("switch", { name: ru.settings.autostart })).toHaveAttribute(
        "aria-checked",
        "true",
      );
    });

    it("records a hotkey while our hotkeys are paused", async () => {
      await openSettings();
      const field = screen.getByRole("button", {
        name: `${ru.settings.launcherHotkey}: Alt+Space`,
      });
      fireEvent.click(field);
      expect(field).toHaveTextContent(ru.settings.recording);
      fireEvent.keyDown(field, { key: "Control", code: "ControlLeft", ctrlKey: true });
      fireEvent.keyDown(field, { key: "k", code: "KeyK", ctrlKey: true, shiftKey: true });
      await vi.waitFor(() => {
        expect(calls).toContainEqual({
          cmd: "settings_set",
          args: { patch: { launcherHotkey: "Ctrl+Shift+K" } },
        });
      });
      expect(calls).toContainEqual({ cmd: "hotkeys_pause", args: { paused: true } });
      expect(calls).toContainEqual({ cmd: "hotkeys_pause", args: { paused: false } });
    });

    it("shows 12h time when chosen", async () => {
      settings = { ...DEFAULT_SETTINGS, hour12: true };
      useIslandStore.setState({ state: { mode: "expanded", tab: "home" } });
      render(<Island />);
      await vi.waitFor(() => {
        expect(screen.getByText(/\d{1,2}:\d{2} (AM|PM)/)).toBeInTheDocument();
      });
    });

    it("asks about autostart on first run and remembers the answer", async () => {
      settings = { ...DEFAULT_SETTINGS, onboarded: false };
      render(<Island />);
      const yes = await screen.findByRole("button", { name: ru.onboarding.yes });
      expect(screen.getByRole("dialog", { name: ru.onboarding.question })).toBeInTheDocument();
      fireEvent.click(yes);
      await vi.waitFor(() => {
        expect(calls).toContainEqual({
          cmd: "settings_set",
          args: { patch: { autostart: true, onboarded: true } },
        });
        expect(useIslandStore.getState().state.mode).toBe("compact");
      });
    });

    it("skips the copy peek when turned off", async () => {
      settings = { ...DEFAULT_SETTINGS, copyPeek: false };
      render(<Island />);
      await vi.waitFor(() => {
        expect(useSettingsStore.getState().settings?.copyPeek).toBe(false);
      });
      await act(async () => {
        await emit("clipboard://added", CLIPS[0]);
      });
      expect(useIslandStore.getState().state.mode).toBe("compact");
    });
  });
});
