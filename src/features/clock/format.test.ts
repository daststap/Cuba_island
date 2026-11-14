import { describe, expect, it } from "vitest";
import { formatLongDate, formatTime, msUntilNextMinute } from "./format";

describe("clock format", () => {
  const date = new Date(2026, 10, 14, 9, 5, 30, 250);

  it("formats 24h time with leading zeros", () => {
    expect(formatTime(date)).toBe("09:05");
    expect(formatTime(new Date(2026, 0, 1, 0, 0))).toBe("00:00");
  });

  it("formats 12h time", () => {
    expect(formatTime(new Date(2026, 0, 1, 21, 7), true)).toBe("9:07 PM");
  });

  it("formats the long Russian date", () => {
    expect(formatLongDate(date)).toBe("суббота, 14 ноября");
  });

  it("computes the delay to the next minute", () => {
    expect(msUntilNextMinute(date)).toBe(29_750);
  });
});
