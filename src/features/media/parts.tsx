import { useEffect, useState } from "react";
import { mediaCommand, reportError, type MediaCommand, type MediaState } from "../../ipc/commands";
import { ru } from "../../i18n/ru";
import { currentPositionMs, formatDuration } from "./model";

const EQ_BARS = [0.82, 0.61, 0.95, 0.7];

export function Equalizer({ playing, className }: { playing: boolean; className?: string }) {
  return (
    <span
      className={`eq flex h-3.5 items-end gap-[2px] ${className ?? ""}`}
      data-playing={playing}
      aria-hidden
    >
      {EQ_BARS.map((duration, i) => (
        <span
          key={i}
          className="eq-bar block h-full w-[3px] rounded-full bg-current"
          style={{ ["--eq-duration" as string]: `${String(duration)}s` }}
        />
      ))}
    </span>
  );
}

export function Cover({
  media,
  size,
  radius,
}: {
  media: MediaState;
  size: number;
  radius: number;
}) {
  const style = { width: size, height: size, borderRadius: radius };
  return media.cover ? (
    <img src={media.cover} alt={ru.media.cover} className="shrink-0 object-cover" style={style} />
  ) : (
    <span
      className="flex shrink-0 items-center justify-center bg-white/10 text-[var(--island-muted)]"
      style={style}
      role="img"
      aria-label={ru.media.cover}
    >
      <svg width={size / 2.5} height={size / 2.5} viewBox="0 0 24 24" aria-hidden>
        <path
          fill="currentColor"
          d="M9 18.5a2.5 2.5 0 1 1-2.5-2.5c.36 0 .7.08 1 .21V5l12-2v12.5a2.5 2.5 0 1 1-2.5-2.5c.36 0 .7.08 1 .21V7.4l-9 1.5z"
        />
      </svg>
    </span>
  );
}

/**
 * Progress bar that re-renders only itself, 4 times a second while playing.
 * A stale `now` (older than the last report) simply shows the reported position.
 */
export function Progress({ media }: { media: MediaState }) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!media.isPlaying) return;
    const timer = setInterval(() => {
      setNow(Date.now());
    }, 250);
    return () => {
      clearInterval(timer);
    };
  }, [media]);

  if (media.durationMs <= 0) return null;
  const position = currentPositionMs(media, now);
  const fraction = position / media.durationMs;

  return (
    <div className="w-full">
      <div
        className="h-1 w-full overflow-hidden rounded-full bg-white/15"
        role="progressbar"
        aria-label={ru.media.progress}
        aria-valuemin={0}
        aria-valuemax={Math.round(media.durationMs / 1000)}
        aria-valuenow={Math.round(position / 1000)}
      >
        <div
          className="h-full rounded-full bg-[var(--island-fg)] transition-[width] duration-200 ease-linear"
          style={{ width: `${String(fraction * 100)}%` }}
        />
      </div>
      <div className="mt-1.5 flex justify-between text-[11px] text-[var(--island-muted)] tabular-nums">
        <span>{formatDuration(position)}</span>
        <span>−{formatDuration(media.durationMs - position)}</span>
      </div>
    </div>
  );
}

function send(cmd: MediaCommand) {
  mediaCommand(cmd).catch((error: unknown) => {
    reportError(`media_command ${cmd}`, error);
  });
}

const ICONS = {
  prev: "M6 6h2v12H6zm3.5 6 8.5 6V6z",
  next: "M6 18l8.5-6L6 6zm10-12v12h2V6z",
  play: "M8 5v14l11-7z",
  pause: "M6 5h4v14H6zm8 0h4v14h-4z",
} as const;

function ControlButton({
  label,
  icon,
  big,
  onClick,
}: {
  label: string;
  icon: keyof typeof ICONS;
  big?: boolean;
  onClick: () => void;
}) {
  const size = big ? 30 : 24;
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      onClick={onClick}
      className="flex items-center justify-center rounded-full p-2 text-[var(--island-fg)] transition hover:bg-white/10 active:scale-90"
    >
      <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden>
        <path fill="currentColor" d={ICONS[icon]} />
      </svg>
    </button>
  );
}

export function Controls({ media }: { media: MediaState }) {
  return (
    <div className="flex items-center justify-center gap-4">
      <ControlButton
        label={ru.media.prev}
        icon="prev"
        onClick={() => {
          send("prev");
        }}
      />
      <ControlButton
        big
        label={media.isPlaying ? ru.media.pause : ru.media.play}
        icon={media.isPlaying ? "pause" : "play"}
        onClick={() => {
          send("toggle");
        }}
      />
      <ControlButton
        label={ru.media.next}
        icon="next"
        onClick={() => {
          send("next");
        }}
      />
    </div>
  );
}
