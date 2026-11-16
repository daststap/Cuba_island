import { useIslandStore } from "../../app/store";
import { ru } from "../../i18n/ru";
import { downloadsReveal, reportError, type Download } from "../../ipc/commands";
import { describe, formatBytes, overallProgress } from "./format";
import { useDownloadsStore, useVisibleDownloads } from "./store";

const BLUE = "#0a84ff";

function ArrowIcon({ size = 16 }: { size?: number }) {
  return (
    <svg
      viewBox="0 0 24 24"
      width={size}
      height={size}
      fill="none"
      stroke="currentColor"
      strokeWidth="2.2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      <path d="M12 4v11M7 10.5l5 5 5-5M5 19.5h14" />
    </svg>
  );
}

function GamepadIcon({ size = 16 }: { size?: number }) {
  return (
    <svg
      viewBox="0 0 24 24"
      width={size}
      height={size}
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      <path d="M7 8h10a4 4 0 014 4v1.5a3 3 0 01-5.4 1.8L14.5 14h-5l-1.1 1.3A3 3 0 013 13.5V12a4 4 0 014-4z" />
      <path d="M8 10.5v3M6.5 12h3" />
      <circle cx="16" cy="11.5" r=".6" fill="currentColor" />
      <circle cx="17.5" cy="13" r=".6" fill="currentColor" />
    </svg>
  );
}

const SourceIcon = ({ item, size }: { item: Download; size: number }) =>
  item.source === "steam" ? <GamepadIcon size={size} /> : <ArrowIcon size={size} />;

/** Small progress ring; spins when the size is unknown. */
function MiniRing({ progress }: { progress: number | null }) {
  const size = 18;
  const stroke = 2.5;
  const r = (size - stroke) / 2;
  const length = 2 * Math.PI * r;
  return (
    <svg
      width={size}
      height={size}
      className={progress === null ? "animate-spin" : "-rotate-90"}
      aria-hidden
    >
      <circle
        cx={size / 2}
        cy={size / 2}
        r={r}
        fill="none"
        stroke="rgb(255 255 255 / 0.18)"
        strokeWidth={stroke}
      />
      <circle
        cx={size / 2}
        cy={size / 2}
        r={r}
        fill="none"
        stroke={BLUE}
        strokeWidth={stroke}
        strokeLinecap="round"
        strokeDasharray={length}
        strokeDashoffset={length * (1 - (progress ?? 0.3))}
        style={{ transition: "stroke-dashoffset 600ms linear" }}
      />
    </svg>
  );
}

/** The pill while something downloads: icon on the left, ring and percent on the right. */
export function DownloadsCompact({ items }: { items: Download[] }) {
  const progress = overallProgress(items);
  const steamOnly = items.every((i) => i.source === "steam");
  return (
    <div
      role="progressbar"
      aria-label={ru.downloads.progress}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={progress === null ? undefined : Math.round(progress * 100)}
      className="flex h-full items-center justify-between px-4"
      style={{ color: BLUE }}
    >
      {steamOnly ? <GamepadIcon /> : <ArrowIcon />}
      <span className="flex items-center gap-1.5 text-[12px] font-semibold tabular-nums">
        {progress !== null
          ? `${String(Math.floor(progress * 100))}%`
          : items.some((i) => i.speed)
            ? describeSpeed(items)
            : null}
        <MiniRing progress={progress} />
      </span>
    </div>
  );
}

/** Combined speed: «2,1 МБ/с». */
function describeSpeed(items: Download[]): string {
  const speed = items.reduce((sum, i) => sum + (i.speed ?? 0), 0);
  return ru.downloads.perSecond(formatBytes(speed));
}

/** Downloads in progress on the home tab. */
export function DownloadsList() {
  const items = useVisibleDownloads();
  if (items.length === 0) return null;
  return (
    <ul aria-label={ru.downloads.list} className="flex w-full flex-col gap-2">
      {items.map((item) => {
        const progress = item.total ? Math.min(1, item.received / item.total) : null;
        return (
          <li key={item.id} className="flex items-center gap-3 text-xs">
            <span className="text-[#0a84ff]">
              <SourceIcon item={item} size={18} />
            </span>
            <div className="min-w-0 flex-1">
              <div className="flex items-baseline justify-between gap-3">
                <span className="truncate font-medium">{item.name}</span>
                <span className="shrink-0 text-[var(--island-muted)] tabular-nums">
                  {describe(item)}
                </span>
              </div>
              <div className="mt-1 h-1 overflow-hidden rounded-full bg-white/12">
                <div
                  className={`h-full rounded-full bg-[#0a84ff] ${progress === null ? "w-1/3 animate-pulse" : ""}`}
                  style={progress === null ? undefined : { width: `${String(progress * 100)}%` }}
                />
              </div>
            </div>
          </li>
        );
      })}
    </ul>
  );
}

/** «Загружено: file.zip»; a click shows the file in Explorer. */
export function DownloadPeek() {
  const finished = useDownloadsStore((s) => s.finished);
  if (!finished) return null;
  const steam = finished.source === "steam";
  const reveal = () => {
    useIslandStore.getState().dispatch({ type: "endPeek" });
    if (finished.path)
      downloadsReveal(finished.path).catch((error: unknown) => {
        reportError("downloads_reveal", error);
      });
  };
  return (
    <button
      type="button"
      onClick={reveal}
      className="flex h-full w-full items-center gap-3 px-5 text-left"
    >
      <span className="text-[#30d158]">
        {steam ? <GamepadIcon size={20} /> : <ArrowIcon size={20} />}
      </span>
      <span className="min-w-0 flex-1">
        <span className="block truncate text-sm font-semibold">
          {steam ? ru.downloads.gameReady : ru.downloads.finished}: {finished.name}
        </span>
        {finished.path && (
          <span className="block text-[11px] text-[var(--island-muted)]">
            {ru.downloads.reveal}
          </span>
        )}
      </span>
    </button>
  );
}
