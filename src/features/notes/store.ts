import { useEffect } from "react";
import { create } from "zustand";
import { useIslandStore } from "../../app/store";
import { ru } from "../../i18n/ru";
import {
  notesList,
  reportError,
  timerGet,
  timerSilence,
  type Note,
  type TimerState,
} from "../../ipc/commands";
import { onNotesChanged, onReminder, onTimer, onTimerDone } from "../../ipc/events";

/** What the alert peek is showing. */
export type Alert = { kind: "reminder"; note: Note } | { kind: "timer" };

interface NotesStore {
  notes: Note[];
  timer: TimerState;
  alert: Alert | null;
  /** Last duration set on the wheels, kept like the iPhone does. */
  pickedMs: number;
}

export const IDLE_TIMER: TimerState = { durationMs: 0, endsAt: null, pausedRemainingMs: null };
export const DEFAULT_PICKED_MS = 5 * 60_000;

export const useNotesStore = create<NotesStore>()(() => ({
  notes: [],
  timer: IDLE_TIMER,
  alert: null,
  pickedMs: DEFAULT_PICKED_MS,
}));

/** Whether a timer is running or paused. */
export const useTimerActive = () =>
  useNotesStore((s) => s.timer.endsAt !== null || s.timer.pausedRemainingMs !== null);

/** Remaining ms of a timer at `now` (pure; the countdown re-renders only itself). */
export function remainingMs(timer: TimerState, now: number): number {
  if (timer.endsAt !== null) return Math.max(0, timer.endsAt - now);
  return timer.pausedRemainingMs ?? 0;
}

/** «4:05», «1:02:05» */
export function formatCountdown(ms: number): string {
  const total = Math.ceil(ms / 1000);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = String(total % 60).padStart(2, "0");
  return h > 0 ? `${String(h)}:${String(m).padStart(2, "0")}:${s}` : `${String(m)}:${s}`;
}

/** Shows an alert: a lasting peek, or a banner if the panel is open. */
function raise(alert: Alert) {
  useNotesStore.setState({ alert });
  const island = useIslandStore.getState();
  if (island.state.mode === "expanded") {
    island.showToast(alert.kind === "timer" ? ru.notes.timerDone : `⏰ ${alert.note.text}`);
  } else {
    island.peek("alert");
  }
}

/** Forgets the alert; a ringing timer goes quiet. */
function clearAlert() {
  const { alert } = useNotesStore.getState();
  if (!alert) return;
  useNotesStore.setState({ alert: null });
  if (alert.kind === "timer") {
    timerSilence().catch((error: unknown) => {
      reportError("timer_silence", error);
    });
  }
}

// Opening the panel (hovering the alert opens the notes tab) also counts as a reaction.
useIslandStore.subscribe((island) => {
  if (island.state.mode === "expanded") clearAlert();
});

/** Clears the alert (the user reacted). */
export function dismissAlert(): void {
  clearAlert();
  useIslandStore.getState().dispatch({ type: "endPeek" });
}

function reloadNotes() {
  notesList()
    .then((notes) => {
      useNotesStore.setState({ notes });
    })
    .catch((error: unknown) => {
      reportError("notes_list", error);
    });
}

/** Keeps notes and the timer in sync and raises alerts. Mount once. */
export function useNotesSync(): void {
  useEffect(() => {
    reloadNotes();
    timerGet()
      .then((timer) => {
        useNotesStore.setState({ timer });
      })
      .catch((error: unknown) => {
        reportError("timer_get", error);
      });
    const unsubscribe = [
      onNotesChanged(reloadNotes),
      onTimer((timer) => {
        useNotesStore.setState({ timer });
      }),
      onReminder((note) => {
        reloadNotes();
        raise({ kind: "reminder", note });
      }),
      onTimerDone(() => {
        raise({ kind: "timer" });
      }),
    ];
    return () => {
      unsubscribe.forEach((fn) => {
        fn();
      });
    };
  }, []);
}
