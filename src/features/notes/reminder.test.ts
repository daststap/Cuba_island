import { describe, expect, it } from "vitest";
import { formatReminder, parseReminder } from "./reminder";

// Friday, 14 Nov 2026, 15:20 local time.
const now = new Date(2026, 10, 14, 15, 20, 0, 0);
const at = (text: string) => parseReminder(text, now)?.at;
const local = (day: number, h: number, m = 0) => new Date(2026, 10, day, h, m, 0, 0);

describe("parseReminder", () => {
  it("understands relative times", () => {
    expect(at("чай через 15 мин")).toEqual(new Date(now.getTime() + 15 * 60_000));
    expect(at("через 5 минут проверить")).toEqual(new Date(now.getTime() + 5 * 60_000));
    expect(at("через полчаса")).toEqual(new Date(now.getTime() + 30 * 60_000));
    expect(at("через час")).toEqual(new Date(now.getTime() + 60 * 60_000));
    expect(at("через 2 часа")).toEqual(new Date(now.getTime() + 2 * 60 * 60_000));
    expect(at("через 1 ч 30 мин")).toEqual(new Date(now.getTime() + 90 * 60_000));
  });

  it("understands clock times, rolling past ones to tomorrow", () => {
    expect(at("позвонить маме в 18:30")).toEqual(local(14, 18, 30));
    expect(at("встреча в 9")).toEqual(local(15, 9));
    expect(at("в 15.45 созвон")).toEqual(local(14, 15, 45));
    expect(at("сегодня в 21")).toEqual(local(14, 21));
  });

  it("understands days", () => {
    expect(at("отчёт завтра в 9:15")).toEqual(local(15, 9, 15));
    expect(at("послезавтра в 10")).toEqual(local(16, 10));
    expect(at("купить хлеб завтра")).toEqual(local(15, 9));
  });

  it("reports where the phrase is, for highlighting", () => {
    const text = "позвонить маме в 18:30";
    const parsed = parseReminder(text, now);
    expect(parsed && text.slice(parsed.start, parsed.end)).toBe("в 18:30");
  });

  it("ignores text without a time and impossible times", () => {
    expect(at("купить молоко")).toBeUndefined();
    expect(at("в 25:00")).toBeUndefined();
    expect(at("взвесить в 2кг")).toBeUndefined();
    expect(at("заварзавтра")).toBeUndefined();
    expect(at("через 0 мин")).toBeUndefined();
  });
});

describe("formatReminder", () => {
  it("says today / tomorrow or the date", () => {
    expect(formatReminder(local(14, 18, 30), now)).toBe("сегодня 18:30");
    expect(formatReminder(local(15, 9), now)).toBe("завтра 09:00");
    expect(formatReminder(local(21, 9), now)).toMatch(/21 нояб\.? 09:00$/);
  });
});
