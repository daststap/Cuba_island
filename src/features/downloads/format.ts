/** Sizes, speeds and time left for downloads, in Russian (pure). */
import { ru } from "../../i18n/ru";
import type { Download } from "../../ipc/commands";

const number = new Intl.NumberFormat("ru-RU", { maximumFractionDigits: 1 });

/** «820 КБ», «12,3 МБ», «1,4 ГБ» (binary units, like Explorer). */
export function formatBytes(bytes: number): string {
  const { units } = ru.downloads;
  let value = Math.max(0, bytes);
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const shown = unit === 0 ? String(Math.round(value)) : number.format(value);
  return `${shown} ${units[unit] ?? ""}`;
}

/** «ещё 40 с», «ещё 12 мин», «ещё 1 ч 5 мин»; null when it cannot be known. */
export function formatTimeLeft(item: Download): string | null {
  if (item.total === null || !item.speed || item.speed <= 0) return null;
  const seconds = Math.ceil(Math.max(0, item.total - item.received) / item.speed);
  const { downloads } = ru;
  if (seconds < 60) return downloads.left(downloads.seconds(seconds));
  const minutes = Math.ceil(seconds / 60);
  if (minutes < 60) return downloads.left(downloads.minutes(minutes));
  return downloads.left(downloads.hours(Math.floor(minutes / 60), minutes % 60));
}

/** One line under a download: «12,3 из 40 МБ · 2,1 МБ/с · ещё 15 с». */
export function describe(item: Download): string {
  const done =
    item.total === null
      ? formatBytes(item.received)
      : ru.downloads.of(formatBytes(item.received), formatBytes(item.total));
  const parts = [done];
  if (item.speed) parts.push(ru.downloads.perSecond(formatBytes(item.speed)));
  const left = formatTimeLeft(item);
  if (left) parts.push(left);
  return parts.join(" · ");
}

/** Share done (0–1) over all downloads with a known size; null if none has one. */
export function overallProgress(items: readonly Download[]): number | null {
  const sized = items.filter((i) => i.total !== null && i.total > 0);
  if (sized.length === 0) return null;
  const total = sized.reduce((sum, i) => sum + (i.total ?? 0), 0);
  const done = sized.reduce((sum, i) => sum + Math.min(i.received, i.total ?? 0), 0);
  return done / total;
}
