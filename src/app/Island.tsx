import { AnimatePresence, motion, type MotionStyle, type Transition } from "motion/react";
import { useRef, type ReactNode } from "react";
import { BatteryIndicator } from "../features/battery/BatteryIndicator";
import { useClaudeStore, useClaudeSync } from "../features/claude/store";
import { ClaudeBadge, ClaudePeek, ClaudeSessions } from "../features/claude/views";
import { ClipboardPeek, ClipboardTab } from "../features/clipboard/ClipboardTab";
import { useClipboardSync } from "../features/clipboard/store";
import { Clock, LongDate } from "../features/clock/Clock";
import { LauncherTab } from "../features/launcher/LauncherTab";
import { useMediaStore, useMediaSync } from "../features/media/store";
import { useCoverColor } from "../features/media/useCoverColor";
import { MediaCompact, MediaPeek, MediaPlayer } from "../features/media/views";
import { Onboarding, SettingsTab } from "../features/settings/SettingsTab";
import { ShelfBadge, ShelfTab } from "../features/shelf/ShelfTab";
import { useShelfSync } from "../features/shelf/store";
import { AlertPeek, NotesTab } from "../features/notes/NotesTab";
import { TimerCompact } from "../features/notes/Timer";
import { useNotesStore, useNotesSync, useTimerActive } from "../features/notes/store";
import { useSettingsStore, useSettingsSync } from "../features/settings/store";
import { ru } from "../i18n/ru";
import type { MediaState, UpdateInfo } from "../ipc/commands";
import { TABS, type IslandState, type Tab } from "./islandMachine";
import { useIslandStore } from "./store";
import { useIslandEvents } from "./useIslandEvents";
import { useIslandRect } from "./useIslandRect";

/** Island shapes. The island sits flush against the top edge: only bottom corners round. */
const SHAPES = {
  compact: { width: 200, height: 36, radius: 18 },
  peek: { width: 360, height: 68, radius: 28 },
  clipboardPeek: { width: 320, height: 44, radius: 22 },
  toastPeek: { width: 440, height: 44, radius: 22 },
  claudePeek: { width: 420, height: 56, radius: 28 },
  expanded: { width: 640, height: 360, radius: 32 },
} as const;

const SPRING: Transition = { type: "spring", stiffness: 420, damping: 34, mass: 0.9 };
const FADE: Transition = { duration: 0.14 };

/** Tells the user once that a newer version is out. */
function announceUpdate(update: UpdateInfo) {
  useIslandStore.getState().showToast(ru.settings.updateAvailable(update.version));
}

export function Island() {
  useIslandEvents();
  useMediaSync();
  useClipboardSync();
  useClaudeSync();
  useShelfSync();
  useNotesSync();
  useSettingsSync(announceUpdate);
  const state = useIslandStore((s) => s.state);
  const dispatch = useIslandStore((s) => s.dispatch);
  const media = useMediaStore((s) => s.media);
  const toast = useIslandStore((s) => s.toast);
  const claudeAttention = useClaudeStore((s) => s.attention);
  const alert = useNotesStore((s) => s.alert);
  const opacity = useSettingsStore((s) => s.settings?.opacity ?? 1);
  const onboarding = useSettingsStore((s) => s.settings !== null && !s.settings.onboarded);
  const glow = useCoverColor(media?.cover ?? null);
  const ref = useRef<HTMLDivElement>(null);
  useIslandRect(ref);

  const peekKind = state.mode === "peek" ? state.kind : null;
  // A peek whose content is gone (player closed, toast expired) shows as compact.
  const stale =
    (peekKind === "media" && !media) ||
    (peekKind === "toast" && !toast) ||
    (peekKind === "claude" && !claudeAttention) ||
    (peekKind === "alert" && !alert);
  const mode = stale ? "compact" : state.mode;
  const shape =
    mode === "expanded"
      ? SHAPES.expanded
      : mode === "peek"
        ? peekKind === "clipboard"
          ? SHAPES.clipboardPeek
          : peekKind === "toast"
            ? SHAPES.toastPeek
            : peekKind === "claude" || peekKind === "alert"
              ? SHAPES.claudePeek
              : SHAPES.peek
        : SHAPES.compact;
  const glowAlpha = media?.isPlaying ? 0.55 : 0.3;
  const shadow = glow
    ? `0 6px 30px -6px rgb(${glow.join(" ")} / ${String(glowAlpha)}), var(--island-shadow)`
    : "var(--island-shadow)";

  let content: ReactNode;
  if (state.mode === "expanded") {
    content = (
      <>
        <ExpandedView
          tab={state.tab}
          media={media}
          onSelect={(tab) => {
            dispatch({ type: "selectTab", tab });
          }}
        />
        {onboarding && <Onboarding />}
      </>
    );
  } else if (mode === "peek" && peekKind === "toast" && toast) {
    content = (
      <div
        role="alert"
        className="flex h-full items-center justify-center px-5 text-center text-xs"
      >
        {toast}
      </div>
    );
  } else if (mode === "peek" && peekKind === "claude") {
    content = <ClaudePeek />;
  } else if (mode === "peek" && peekKind === "alert") {
    content = <AlertPeek />;
  } else if (mode === "peek" && peekKind === "clipboard") {
    content = <ClipboardPeek />;
  } else if (mode === "peek" && media) {
    content = <MediaPeek media={media} />;
  } else {
    content = <CompactView mode={state.mode} media={media} />;
  }

  return (
    <div className="flex h-full justify-center">
      <motion.div
        ref={ref}
        data-mode={mode}
        className="relative overflow-hidden bg-[var(--island-bg)] text-[var(--island-fg)] transition-shadow duration-700"
        style={
          {
            boxShadow: shadow,
            "--island-bg": `rgb(0 0 0 / ${String(opacity)})`,
          } as MotionStyle
        }
        initial={false}
        animate={{
          width: shape.width,
          height: shape.height,
          borderBottomLeftRadius: shape.radius,
          borderBottomRightRadius: shape.radius,
        }}
        transition={SPRING}
      >
        <AnimatePresence mode="popLayout" initial={false}>
          <motion.div
            key={mode === "peek" && peekKind ? `peek-${peekKind}` : mode}
            className="absolute inset-0"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1, transition: { ...FADE, delay: 0.07 } }}
            exit={{ opacity: 0, transition: FADE }}
          >
            {content}
          </motion.div>
        </AnimatePresence>
        <AnimatePresence>
          {mode === "expanded" && toast && (
            <motion.div
              key="toast"
              role="alert"
              className="absolute inset-x-6 bottom-3 rounded-xl bg-[#3a1d1d] px-3 py-2 text-center text-xs text-[var(--island-fg)]"
              initial={{ opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: 8 }}
            >
              {toast}
            </motion.div>
          )}
        </AnimatePresence>
      </motion.div>
    </div>
  );
}

function CompactView({ mode, media }: { mode: IslandState["mode"]; media: MediaState | null }) {
  const timerActive = useTimerActive();
  return (
    <div role="status" aria-label={ru.appName} data-mode={mode} className="relative h-full">
      {timerActive ? (
        <TimerCompact />
      ) : media ? (
        <MediaCompact media={media} />
      ) : (
        <div className="flex h-full items-center justify-between px-5 text-[13px] font-semibold">
          <Clock />
          <BatteryIndicator showPercent={false} />
        </div>
      )}
      {/* Claude Code status sits in the free middle of the pill. */}
      <div className="pointer-events-none absolute inset-0 flex items-center justify-center gap-2">
        <ClaudeBadge />
        <ShelfBadge />
      </div>
    </div>
  );
}

function ExpandedView({
  tab,
  media,
  onSelect,
}: {
  tab: Tab;
  media: MediaState | null;
  onSelect: (tab: Tab) => void;
}) {
  const player = tab === "home" && media;
  return (
    <div className="flex h-full flex-col px-6 pt-3 pb-6">
      <div className="relative flex items-center justify-center">
        {player && (
          <div className="absolute left-0 flex items-baseline gap-2 text-xs text-[var(--island-muted)]">
            <Clock className="font-semibold text-[var(--island-fg)]" />
          </div>
        )}
        <div role="tablist" aria-label={ru.tabsLabel} className="flex gap-1">
          {TABS.map((t) => (
            <button
              key={t}
              role="tab"
              type="button"
              aria-selected={t === tab}
              onClick={() => {
                onSelect(t);
              }}
              className={`rounded-full px-3 py-1 text-xs font-medium transition-colors ${
                t === tab
                  ? "bg-white/15 text-[var(--island-fg)]"
                  : "text-[var(--island-muted)] hover:bg-white/8 hover:text-[var(--island-fg)]"
              }`}
            >
              {ru.tabs[t]}
            </button>
          ))}
        </div>
        {player && (
          <div className="absolute right-0 text-xs text-[var(--island-muted)]">
            <BatteryIndicator />
          </div>
        )}
      </div>

      <div
        role="tabpanel"
        className="flex min-h-0 flex-1 flex-col items-center justify-center pt-3"
      >
        {player ? (
          <>
            <MediaPlayer media={media} />
            <div className="mt-3 w-full">
              <ClaudeSessions />
            </div>
          </>
        ) : tab === "notes" ? (
          <NotesTab />
        ) : tab === "shelf" ? (
          <ShelfTab />
        ) : tab === "clipboard" ? (
          <ClipboardTab />
        ) : tab === "launcher" ? (
          <LauncherTab />
        ) : tab === "home" ? (
          <>
            <Clock className="text-7xl font-light tracking-tight" />
            <LongDate className="mt-1 text-base text-[var(--island-muted)] first-letter:uppercase" />
            <div className="mt-5 text-sm text-[var(--island-muted)]">
              <BatteryIndicator />
            </div>
            <div className="mt-4 w-full">
              <ClaudeSessions />
            </div>
          </>
        ) : (
          <SettingsTab />
        )}
      </div>
    </div>
  );
}
