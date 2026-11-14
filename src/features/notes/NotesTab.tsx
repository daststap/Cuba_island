import { useEffect, useState, type KeyboardEvent, type ReactNode } from "react";
import { useIslandStore } from "../../app/store";
import { ru } from "../../i18n/ru";
import {
  notesAdd,
  notesDelete,
  notesSetDone,
  reportError,
  timerControl,
  timerStart,
  type Note,
  type TimerState,
} from "../../ipc/commands";
import { formatReminder, parseReminder } from "./reminder";
import { dismissAlert, formatCountdown, remainingMs, useNotesStore } from "./store";

const TIMER_PRESETS_MIN = [1, 5, 10, 25];

function fail(context: string) {
  return (error: unknown) => {
    reportError(context, error);
  };
}

/** Current time, ticking every `ms` only while `active`. */
function useNow(active: boolean, ms = 250): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!active) return;
    const id = setInterval(() => {
      setNow(Date.now());
    }, ms);
    return () => {
      clearInterval(id);
    };
  }, [active, ms]);
  return now;
}

/** Note text with the recognised time phrase highlighted. */
function Highlighted({ text, now }: { text: string; now: Date }): ReactNode {
  const parsed = parseReminder(text, now);
  if (!parsed) return text;
  return (
    <>
      {text.slice(0, parsed.start)}
      <span className="text-[#ffb37a]">{text.slice(parsed.start, parsed.end)}</span>
      {text.slice(parsed.end)}
    </>
  );
}

function NoteRow({ note, now }: { note: Note; now: number }) {
  const created = new Date(note.createdAt);
  return (
    <li className="group flex items-start gap-2 rounded-lg px-2 py-1.5 hover:bg-white/8">
      <input
        type="checkbox"
        checked={note.done}
        aria-label={ru.notes.markDone(note.text)}
        onChange={(e) => {
          notesSetDone(note.id, e.target.checked).catch(fail("notes_set_done"));
        }}
        className="mt-0.5 accent-[#30d158]"
      />
      <div className={`min-w-0 flex-1 text-sm ${note.done ? "text-white/35 line-through" : ""}`}>
        <div className="break-words">
          <Highlighted text={note.text} now={created} />
        </div>
        {note.remindAt !== null && !note.done && (
          <div className={`text-[11px] ${note.reminded ? "text-white/40" : "text-[#ffb37a]"}`}>
            ⏰ {formatReminder(new Date(note.remindAt), new Date(now))}
          </div>
        )}
      </div>
      <button
        type="button"
        aria-label={ru.notes.delete(note.text)}
        onClick={() => {
          notesDelete(note.id).catch(fail("notes_delete"));
        }}
        className="hidden h-5 w-5 shrink-0 items-center justify-center rounded-full text-xs text-white/50 group-hover:flex hover:bg-white/15 hover:text-white"
      >
        ×
      </button>
    </li>
  );
}

function NewNote() {
  const [text, setText] = useState("");
  const [now, setNow] = useState(() => new Date());
  const parsed = text ? parseReminder(text, now) : null;

  const submit = () => {
    const value = text.trim();
    if (!value) return;
    const reminder = parseReminder(value, new Date());
    notesAdd(value, reminder ? reminder.at.getTime() : null)
      .then(() => {
        setText("");
      })
      .catch(fail("notes_add"));
  };

  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter") {
      e.preventDefault();
      submit();
    }
  };

  return (
    <div>
      <input
        type="text"
        value={text}
        placeholder={ru.notes.placeholder}
        aria-label={ru.notes.new}
        onMouseDown={() => {
          useIslandStore.getState().enterInput();
        }}
        onChange={(e) => {
          setText(e.target.value);
          setNow(new Date());
        }}
        onKeyDown={onKeyDown}
        className="w-full rounded-xl bg-white/10 px-3 py-2 text-sm outline-none placeholder:text-[var(--island-muted)] focus:bg-white/15"
      />
      <div className="mt-1 h-4 px-1 text-[11px] text-[#ffb37a]" aria-live="polite">
        {parsed && ru.notes.willRemind(formatReminder(parsed.at, now))}
      </div>
    </div>
  );
}

function TimerPanel({ timer }: { timer: TimerState }) {
  const running = timer.endsAt !== null;
  const paused = timer.pausedRemainingMs !== null;
  const now = useNow(running);
  const [minutes, setMinutes] = useState("");

  const start = (min: number) => {
    if (min > 0) timerStart(min * 60_000).catch(fail("timer_start"));
  };

  return (
    <section
      aria-label={ru.notes.timer}
      className="flex w-44 shrink-0 flex-col gap-2 border-l border-white/10 pl-4"
    >
      <div className="text-xs text-[var(--island-muted)]">{ru.notes.timer}</div>
      <div className="text-4xl font-light tabular-nums" role="timer">
        {running || paused ? formatCountdown(remainingMs(timer, now)) : "0:00"}
      </div>
      {running || paused ? (
        <div className="flex gap-2">
          <button
            type="button"
            className="flex-1 rounded-lg bg-white/15 py-1.5 text-xs hover:bg-white/25"
            onClick={() => {
              timerControl(running ? "pause" : "resume").catch(fail("timer_control"));
            }}
          >
            {running ? ru.notes.pause : ru.notes.resume}
          </button>
          <button
            type="button"
            className="flex-1 rounded-lg bg-white/8 py-1.5 text-xs hover:bg-white/15"
            onClick={() => {
              timerControl("cancel").catch(fail("timer_control"));
            }}
          >
            {ru.notes.cancel}
          </button>
        </div>
      ) : (
        <>
          <div className="grid grid-cols-4 gap-1">
            {TIMER_PRESETS_MIN.map((min) => (
              <button
                key={min}
                type="button"
                aria-label={ru.notes.startMinutes(min)}
                className="rounded-lg bg-white/10 py-1 text-xs hover:bg-white/20"
                onClick={() => {
                  start(min);
                }}
              >
                {min}
              </button>
            ))}
          </div>
          <input
            type="number"
            min={1}
            max={1440}
            inputMode="numeric"
            value={minutes}
            placeholder={ru.notes.customMinutes}
            aria-label={ru.notes.customMinutes}
            onMouseDown={() => {
              useIslandStore.getState().enterInput();
            }}
            onChange={(e) => {
              setMinutes(e.target.value);
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                start(Number(minutes));
                setMinutes("");
              }
            }}
            className="w-full rounded-lg bg-white/10 px-2 py-1 text-xs outline-none placeholder:text-[var(--island-muted)]"
          />
        </>
      )}
    </section>
  );
}

/** The "Заметки" tab: notes with reminders on the left, a timer on the right. */
export function NotesTab() {
  const notes = useNotesStore((s) => s.notes);
  const timer = useNotesStore((s) => s.timer);
  const now = useNow(true, 30_000);

  return (
    <div className="flex h-full min-h-0 w-full gap-4">
      <div className="flex min-h-0 flex-1 flex-col">
        <NewNote />
        {notes.length === 0 ? (
          <div className="flex flex-1 items-center justify-center px-6 text-center text-sm text-[var(--island-muted)]">
            {ru.notes.empty}
          </div>
        ) : (
          <ul aria-label={ru.notes.list} className="-mx-2 min-h-0 flex-1 overflow-y-auto">
            {notes.map((note) => (
              <NoteRow key={note.id} note={note} now={now} />
            ))}
          </ul>
        )}
      </div>
      <TimerPanel timer={timer} />
    </div>
  );
}

/** Lasting peek for a due reminder or a finished timer; click to dismiss. */
export function AlertPeek() {
  const alert = useNotesStore((s) => s.alert);
  if (!alert) return null;
  const text = alert.kind === "timer" ? ru.notes.timerDone : alert.note.text;
  return (
    <button
      type="button"
      role="alert"
      onClick={dismissAlert}
      title={ru.notes.dismiss}
      className="flex h-full w-full items-center gap-3 px-5 text-left"
    >
      <span className="animate-pulse text-xl" aria-hidden>
        {alert.kind === "timer" ? "⏱" : "⏰"}
      </span>
      <span className="min-w-0 flex-1">
        <span className="block truncate text-sm font-semibold">{text}</span>
        <span className="block text-[11px] text-[var(--island-muted)]">{ru.notes.dismiss}</span>
      </span>
    </button>
  );
}

/** Countdown in the pill while the timer runs (or is paused). */
export function TimerBadge() {
  const timer = useNotesStore((s) => s.timer);
  const running = timer.endsAt !== null;
  const now = useNow(running, 500);
  if (!running && timer.pausedRemainingMs === null) return null;
  return (
    <span
      role="timer"
      aria-label={ru.notes.timer}
      className={`text-[12px] font-semibold tabular-nums ${running ? "text-[#ffb37a]" : "text-white/40"}`}
    >
      {formatCountdown(remainingMs(timer, now))}
    </span>
  );
}
