/**
 * Dev-only demo scenes for README screenshots: `http://localhost:1420/?demo=home`.
 * Fakes the Rust side with Tauri's IPC mocks, so no real user data is ever shown.
 * Imported dynamically from `main.tsx` only in dev builds.
 */
import { mockIPC } from "@tauri-apps/api/mocks";
import { useIslandStore } from "../app/store";
import type { ClipItem, LaunchResult, MediaState, Settings } from "../ipc/commands";

export const SCENES = [
  "compact",
  "music",
  "peek",
  "home",
  "clipboard",
  "launcher",
  "settings",
] as const;
export type Scene = (typeof SCENES)[number];

function cover(): string {
  const canvas = document.createElement("canvas");
  canvas.width = canvas.height = 320;
  const ctx = canvas.getContext("2d");
  if (!ctx) return "";
  const gradient = ctx.createLinearGradient(0, 0, 320, 320);
  gradient.addColorStop(0, "#ff5f6d");
  gradient.addColorStop(1, "#ffc371");
  ctx.fillStyle = gradient;
  ctx.fillRect(0, 0, 320, 320);
  ctx.fillStyle = "rgba(0,0,0,0.25)";
  ctx.beginPath();
  ctx.arc(160, 160, 90, 0, Math.PI * 2);
  ctx.fill();
  ctx.fillStyle = "#fff";
  ctx.beginPath();
  ctx.arc(160, 160, 16, 0, Math.PI * 2);
  ctx.fill();
  return canvas.toDataURL("image/jpeg");
}

const SETTINGS: Settings = {
  monitor: null,
  autostart: true,
  hideInFullscreen: true,
  hour12: false,
  clipboardHotkey: "Ctrl+Alt+V",
  launcherHotkey: "Alt+Space",
  historyLimit: 500,
  copyPeek: true,
  opacity: 1,
  onboarded: true,
};

const minutesAgo = (m: number) => Date.now() - m * 60_000;

const CLIPS: ClipItem[] = [
  {
    id: 1,
    kind: "text",
    text: 'git commit -m "feat(island): spring animations"',
    thumbPath: null,
    pinned: true,
    createdAt: minutesAgo(90),
    usedAt: minutesAgo(90),
  },
  {
    id: 2,
    kind: "text",
    text: "https://tauri.app/start/",
    thumbPath: null,
    pinned: false,
    createdAt: minutesAgo(0),
    usedAt: minutesAgo(0),
  },
  {
    id: 3,
    kind: "text",
    text: "Встреча перенесена на четверг, 15:00",
    thumbPath: null,
    pinned: false,
    createdAt: minutesAgo(4),
    usedAt: minutesAgo(4),
  },
  {
    id: 4,
    kind: "text",
    text: "cargo clippy --all-targets -- -D warnings",
    thumbPath: null,
    pinned: false,
    createdAt: minutesAgo(27),
    usedAt: minutesAgo(27),
  },
  {
    id: 5,
    kind: "text",
    text: "#30d158",
    thumbPath: null,
    pinned: false,
    createdAt: minutesAgo(180),
    usedAt: minutesAgo(180),
  },
];

const LAUNCH: LaunchResult[] = [
  { id: "calc:6", kind: "calc", title: "6", iconPath: null },
  { id: "web:2+2*2", kind: "web", title: "2+2*2", iconPath: null },
];

export async function setupDemo(scene: Scene): Promise<void> {
  const withMusic = scene !== "compact" && scene !== "launcher" && scene !== "clipboard";
  const media: MediaState | null = withMusic
    ? {
        trackId: "demo",
        title: "Northern Lights",
        artist: "Aurora Lane",
        album: "Polar Nights",
        appId: "Spotify.exe",
        isPlaying: true,
        positionMs: 74_000,
        durationMs: 212_000,
        positionUpdatedAt: Date.now(),
        cover: cover(),
      }
    : null;

  mockIPC(
    (cmd) => {
      switch (cmd) {
        case "media_get_state":
          return media;
        case "get_battery":
          return { percent: 78, charging: false, pluggedIn: false };
        case "settings_get":
          return SETTINGS;
        case "hotkeys_status":
          return { clipboard: "Ctrl+Alt+V", launcher: "Alt+Space", launcherFallback: false };
        case "clipboard_list":
          return CLIPS;
        case "launcher_search":
          return LAUNCH;
        case "monitors_list":
          return [];
        case "update_check":
          return null;
        default:
          return null;
      }
    },
    { shouldMockEvents: true },
  );

  // A calm backdrop instead of whatever is on the screen.
  document.body.style.background =
    "radial-gradient(circle at 30% 0%, #3b3f73 0%, #1b1d33 45%, #0e0f1c 100%)";

  const store = useIslandStore;
  switch (scene) {
    case "compact":
    case "music":
      break;
    case "peek":
      store.setState({ state: { mode: "peek", kind: "media" } });
      break;
    case "home":
      store.setState({ state: { mode: "expanded", tab: "home" } });
      break;
    case "clipboard":
    case "launcher":
    case "settings":
      store.setState({ state: { mode: "expanded", tab: scene }, inputMode: true });
      break;
  }

  if (scene === "launcher") {
    // Type into the command line once it is on screen.
    setTimeout(() => {
      const input = document.querySelector<HTMLInputElement>('input[role="combobox"]');
      if (!input) return;
      // Call the native setter so React notices the change.
      Reflect.set(HTMLInputElement.prototype, "value", "2+2*2", input);
      input.dispatchEvent(new Event("input", { bubbles: true }));
    }, 300);
  }
  await Promise.resolve();
}
