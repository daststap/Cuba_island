import { useState, type KeyboardEvent, type ReactNode } from "react";
import { useIslandStore } from "../../app/store";
import { ru } from "../../i18n/ru";
import { notesAdd, notesDelete, notesSetDone, reportError, type Note } from "../../ipc/commands";
import { formatReminder, parseReminder } from "./reminder";
import { dismissAlert, useNotesStore } from "./store";
import { TimerPanel } from "./Timer";
import { useNow } from "./useNow";

function fail(context: string) {
  return (error: unknown) => {
    reportError(context, error);
  };
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

/** The "Заметки" tab: notes with reminders on the left, a timer on the right. */
export function NotesTab() {
  const notes = useNotesStore((s) => s.notes);
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
      <TimerPanel />
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
