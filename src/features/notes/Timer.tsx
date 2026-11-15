import type { ReactNode } from "react";
import { ru } from "../../i18n/ru";
import { reportError, timerControl, timerStart, type TimerState } from "../../ipc/commands";
import { formatCountdown, remainingMs, useNotesStore, useTimerActive } from "./store";
import { useNow } from "./useNow";
import { WHEEL_HEIGHT, WHEEL_ROW, Wheel } from "./Wheel";

const ORANGE = "#ff9f0a";
const HOUR = 3_600_000;
const MINUTE = 60_000;
const SECOND = 1000;

function fail(context: string) {
  return (error: unknown) => {
    reportError(context, error);
  };
}

type Tone = "gray" | "green" | "orange";

const TONES: Record<Tone, string> = {
  gray: "bg-white/15 text-white hover:bg-white/25",
  green: "bg-[#30d158]/20 text-[#30d158] hover:bg-[#30d158]/30",
  orange: "bg-[#ff9f0a]/20 text-[#ff9f0a] hover:bg-[#ff9f0a]/30",
};

/** Round button with the iPhone's double ring. */
function RoundButton({
  tone,
  disabled = false,
  onClick,
  children,
}: {
  tone: Tone;
  disabled?: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      disabled={disabled}
      onClick={onClick}
      className={`h-[60px] w-[60px] rounded-full text-[13px] font-medium ring-1 ring-current/20 ring-offset-2 ring-offset-black transition-colors disabled:opacity-40 ${TONES[tone]}`}
    >
      {children}
    </button>
  );
}

/** Same height as the wheels, so the buttons stay put when the timer starts. */
const RING_SIZE = WHEEL_HEIGHT;
const RING_STROKE = 6;

/** Orange ring that empties as the timer runs, with the countdown inside. */
function Ring({ progress, children }: { progress: number; children: ReactNode }) {
  const r = (RING_SIZE - RING_STROKE) / 2;
  const length = 2 * Math.PI * r;
  return (
    <div className="relative" style={{ width: RING_SIZE, height: RING_SIZE }}>
      <svg width={RING_SIZE} height={RING_SIZE} className="-rotate-90" aria-hidden>
        <circle
          cx={RING_SIZE / 2}
          cy={RING_SIZE / 2}
          r={r}
          fill="none"
          stroke="rgb(255 255 255 / 0.12)"
          strokeWidth={RING_STROKE}
        />
        <circle
          cx={RING_SIZE / 2}
          cy={RING_SIZE / 2}
          r={r}
          fill="none"
          stroke={ORANGE}
          strokeWidth={RING_STROKE}
          strokeLinecap="round"
          strokeDasharray={length}
          strokeDashoffset={length * (1 - progress)}
          style={{ transition: "stroke-dashoffset 250ms linear" }}
        />
      </svg>
      <div className="absolute inset-0 flex items-center justify-center">{children}</div>
    </div>
  );
}

function Picker() {
  const picked = useNotesStore((s) => s.pickedMs);
  const h = Math.floor(picked / HOUR);
  const m = Math.floor((picked % HOUR) / MINUTE);
  const s = Math.floor((picked % MINUTE) / SECOND);
  const pick = (hours: number, minutes: number, seconds: number) => {
    useNotesStore.setState({ pickedMs: hours * HOUR + minutes * MINUTE + seconds * SECOND });
  };
  return (
    <div
      role="group"
      aria-label={ru.notes.duration}
      className="relative flex justify-center"
      style={{
        height: WHEEL_HEIGHT,
        maskImage: "linear-gradient(transparent, black 30%, black 70%, transparent)",
      }}
    >
      <div
        aria-hidden
        className="pointer-events-none absolute inset-x-0 rounded-lg bg-white/10"
        style={{ top: WHEEL_HEIGHT / 2 - WHEEL_ROW / 2, height: WHEEL_ROW }}
      />
      <Wheel
        value={h}
        max={23}
        label={ru.notes.hours}
        unit={ru.notes.unitHours}
        onChange={(v) => {
          pick(v, m, s);
        }}
      />
      <Wheel
        value={m}
        max={59}
        label={ru.notes.minutes}
        unit={ru.notes.unitMinutes}
        onChange={(v) => {
          pick(h, v, s);
        }}
      />
      <Wheel
        value={s}
        max={59}
        label={ru.notes.seconds}
        unit={ru.notes.unitSeconds}
        onChange={(v) => {
          pick(h, m, v);
        }}
      />
    </div>
  );
}

function Countdown({ timer }: { timer: TimerState }) {
  const running = timer.endsAt !== null;
  const now = useNow(running);
  const left = remainingMs(timer, now);
  const progress = timer.durationMs > 0 ? left / timer.durationMs : 0;
  return (
    <div className="flex justify-center">
      <Ring progress={progress}>
        <span
          role="timer"
          aria-label={ru.notes.left}
          className={`text-3xl font-light tabular-nums ${running ? "" : "text-white/50"}`}
        >
          {formatCountdown(left)}
        </span>
      </Ring>
    </div>
  );
}

/** The timer column of the notes tab: wheels when idle, a ring while counting. */
export function TimerPanel() {
  const timer = useNotesStore((s) => s.timer);
  const picked = useNotesStore((s) => s.pickedMs);
  const running = timer.endsAt !== null;
  const active = useTimerActive();

  return (
    <section
      aria-label={ru.notes.timer}
      className="flex w-56 shrink-0 flex-col gap-3 border-l border-white/10 pl-4"
    >
      <div className="text-xs text-[var(--island-muted)]">{ru.notes.timer}</div>
      {active ? <Countdown timer={timer} /> : <Picker />}
      <div className="flex justify-between px-1">
        <RoundButton
          tone="gray"
          disabled={!active}
          onClick={() => {
            timerControl("cancel").catch(fail("timer_control"));
          }}
        >
          {ru.notes.cancel}
        </RoundButton>
        {!active ? (
          <RoundButton
            tone="green"
            disabled={picked === 0}
            onClick={() => {
              timerStart(picked).catch(fail("timer_start"));
            }}
          >
            {ru.notes.start}
          </RoundButton>
        ) : running ? (
          <RoundButton
            tone="orange"
            onClick={() => {
              timerControl("pause").catch(fail("timer_control"));
            }}
          >
            {ru.notes.pause}
          </RoundButton>
        ) : (
          <RoundButton
            tone="green"
            onClick={() => {
              timerControl("resume").catch(fail("timer_control"));
            }}
          >
            {ru.notes.resume}
          </RoundButton>
        )}
      </div>
    </section>
  );
}

function TimerIcon() {
  return (
    <svg
      viewBox="0 0 24 24"
      width={18}
      height={18}
      fill="none"
      stroke="currentColor"
      strokeWidth={2.2}
      strokeLinecap="round"
      aria-hidden
    >
      <circle cx={12} cy={13.5} r={8} />
      <path d="M12 9.5v4l2.5 2M9.5 2.5h5" />
    </svg>
  );
}

/** The pill while a timer runs: icon on the left, countdown on the right (Live Activity). */
export function TimerCompact() {
  const timer = useNotesStore((s) => s.timer);
  const running = timer.endsAt !== null;
  const now = useNow(running, 500);
  return (
    <div
      className={`flex h-full items-center justify-between px-4 ${running ? "" : "opacity-50"}`}
      style={{ color: ORANGE }}
    >
      <TimerIcon />
      <span
        role="timer"
        aria-label={ru.notes.timer}
        className="text-[14px] font-semibold tabular-nums"
      >
        {formatCountdown(remainingMs(timer, now))}
      </span>
    </div>
  );
}
