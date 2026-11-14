/**
 * Reminder times written in the note itself, in Russian:
 *   «чай через 15 мин», «через полчаса», «через 1 ч 30 мин»,
 *   «позвонить маме в 18:30», «в 9», «отчёт завтра в 9:15», «послезавтра».
 * Pure: `now` is passed in, the result is a local Date.
 */

export interface ParsedReminder {
  at: Date;
  /** Where the time phrase is in the text, for highlighting. */
  start: number;
  end: number;
}

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
/** A day word without a time means this hour. */
const DEFAULT_HOUR = 9;

// Not preceded by a letter or digit (JS \b does not know Cyrillic).
const NOT_AFTER_WORD = "(?<![\\p{L}\\d])";

const RELATIVE = new RegExp(
  `${NOT_AFTER_WORD}через\\s+(?:(полчаса)|(час)(?![\\p{L}])|(\\d+)\\s*(минут[уы]?|мин|м|час(?:а|ов)?|ч)(?![\\p{L}])(?:\\s+(\\d+)\\s*(?:минут[уы]?|мин|м)(?![\\p{L}]))?)`,
  "iu",
);
const AT_TIME = new RegExp(
  `${NOT_AFTER_WORD}(?:(сегодня|завтра|послезавтра)\\s+)?в\\s+(\\d{1,2})(?:[:.](\\d{2}))?(?![\\d\\p{L}])`,
  "iu",
);
const DAY_ONLY = new RegExp(`${NOT_AFTER_WORD}(завтра|послезавтра)(?![\\p{L}])`, "iu");

function daysFor(word: string | undefined): number {
  switch (word?.toLowerCase()) {
    case "завтра":
      return 1;
    case "послезавтра":
      return 2;
    default:
      return 0;
  }
}

function relative(text: string, now: Date): ParsedReminder | null {
  const m = RELATIVE.exec(text);
  if (!m) return null;
  const [, halfHour, hourWord, amount, unit, extraMinutes] = m;
  let ms: number;
  if (halfHour) ms = 30 * MINUTE;
  else if (hourWord) ms = HOUR;
  else {
    const n = Number(amount);
    const isHours = unit?.toLowerCase().startsWith("ч") ?? false;
    ms = n * (isHours ? HOUR : MINUTE) + Number(extraMinutes ?? 0) * MINUTE;
  }
  if (ms <= 0) return null;
  return { at: new Date(now.getTime() + ms), start: m.index, end: m.index + m[0].length };
}

function atTime(text: string, now: Date): ParsedReminder | null {
  const m = AT_TIME.exec(text);
  if (!m) return null;
  const [, dayWord, hours, minutes] = m;
  const h = Number(hours);
  const min = Number(minutes ?? 0);
  if (h > 23 || min > 59) return null;
  const at = new Date(now);
  at.setHours(h, min, 0, 0);
  at.setDate(at.getDate() + daysFor(dayWord));
  // "в 9" that has already passed today means tomorrow.
  if (!dayWord && at.getTime() <= now.getTime()) at.setDate(at.getDate() + 1);
  return { at, start: m.index, end: m.index + m[0].length };
}

function dayOnly(text: string, now: Date): ParsedReminder | null {
  const m = DAY_ONLY.exec(text);
  if (!m) return null;
  const at = new Date(now);
  at.setHours(DEFAULT_HOUR, 0, 0, 0);
  at.setDate(at.getDate() + daysFor(m[1]));
  return { at, start: m.index, end: m.index + m[0].length };
}

export function parseReminder(text: string, now: Date): ParsedReminder | null {
  return relative(text, now) ?? atTime(text, now) ?? dayOnly(text, now);
}

const timeFormat = new Intl.DateTimeFormat("ru-RU", { hour: "2-digit", minute: "2-digit" });
const dateFormat = new Intl.DateTimeFormat("ru-RU", {
  weekday: "short",
  day: "numeric",
  month: "short",
});

function startOfDay(d: Date): number {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
}

/** «сегодня 18:30», «завтра 09:00», «пт, 21 нояб. 09:00» */
export function formatReminder(at: Date, now: Date): string {
  const days = Math.round((startOfDay(at) - startOfDay(now)) / (24 * HOUR));
  const time = timeFormat.format(at);
  if (days === 0) return `сегодня ${time}`;
  if (days === 1) return `завтра ${time}`;
  return `${dateFormat.format(at)} ${time}`;
}
