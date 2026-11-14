import { ru } from "../../i18n/ru";

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;

const shortDate = new Intl.DateTimeFormat("ru-RU", { day: "numeric", month: "short" });

function startOfDay(ms: number): number {
  const d = new Date(ms);
  d.setHours(0, 0, 0, 0);
  return d.getTime();
}

/** "только что", "2 мин назад", "3 ч назад", "вчера", "12 нояб." */
export function formatRelative(thenMs: number, nowMs: number): string {
  const diff = Math.max(0, nowMs - thenMs);
  if (diff < MINUTE) return ru.clipboard.justNow;
  if (diff < HOUR) return ru.clipboard.minutesAgo(Math.floor(diff / MINUTE));
  if (thenMs >= startOfDay(nowMs)) return ru.clipboard.hoursAgo(Math.floor(diff / HOUR));
  if (thenMs >= startOfDay(nowMs) - 24 * HOUR) return ru.clipboard.yesterday;
  return shortDate.format(thenMs);
}

/** One-line preview: whitespace collapsed, cut to `max` characters with an ellipsis. */
export function oneLine(text: string, max: number): string {
  const flat = text.replace(/\s+/g, " ").trim();
  return flat.length > max ? `${flat.slice(0, max - 1).trimEnd()}…` : flat;
}

/** Index after moving the selection by `delta`, clamped to the list. */
export function moveSelection(index: number, delta: number, length: number): number {
  if (length === 0) return 0;
  return Math.min(length - 1, Math.max(0, index + delta));
}
