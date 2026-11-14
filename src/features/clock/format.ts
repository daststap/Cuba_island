const LOCALE = "ru-RU";

const time24 = new Intl.DateTimeFormat(LOCALE, {
  hour: "2-digit",
  minute: "2-digit",
  hourCycle: "h23",
});
const time12 = new Intl.DateTimeFormat("en-US", {
  hour: "numeric",
  minute: "2-digit",
  hour12: true,
});
const longDate = new Intl.DateTimeFormat(LOCALE, {
  weekday: "long",
  day: "numeric",
  month: "long",
});

/** "09:05" (24h) or "9:05 AM" (12h). The 12h switch arrives with settings. */
export function formatTime(date: Date, hour12 = false): string {
  return (hour12 ? time12 : time24).format(date);
}

/** "пятница, 14 ноября" */
export function formatLongDate(date: Date): string {
  return longDate.format(date);
}

/** Milliseconds until the next minute starts, so the clock ticks exactly on time. */
export function msUntilNextMinute(date: Date): number {
  return 60_000 - (date.getSeconds() * 1000 + date.getMilliseconds());
}
