import { describe, expect, it } from "vitest";
import { formatRelative, moveSelection, oneLine } from "./model";

describe("formatRelative", () => {
  const now = new Date(2026, 10, 14, 15, 30).getTime();
  const at = (h: number, m: number, day = 14) => new Date(2026, 10, day, h, m).getTime();

  it("formats recent times", () => {
    expect(formatRelative(now - 20_000, now)).toBe("только что");
    expect(formatRelative(now - 2 * 60_000, now)).toBe("2 мин назад");
    expect(formatRelative(at(12, 10), now)).toBe("3 ч назад");
  });

  it("formats yesterday and older dates", () => {
    expect(formatRelative(at(23, 50, 13), now)).toBe("вчера");
    expect(formatRelative(at(10, 0, 3), now)).toMatch(/^3 нояб/);
  });

  it("never shows the future", () => {
    expect(formatRelative(now + 5000, now)).toBe("только что");
  });
});

describe("oneLine", () => {
  it("collapses whitespace and truncates", () => {
    expect(oneLine("  a\n\tb   c ", 10)).toBe("a b c");
    expect(oneLine("abcdefghijkl", 6)).toBe("abcde…");
  });
});

describe("moveSelection", () => {
  it("clamps to the list", () => {
    expect(moveSelection(0, -1, 3)).toBe(0);
    expect(moveSelection(1, 1, 3)).toBe(2);
    expect(moveSelection(2, 1, 3)).toBe(2);
    expect(moveSelection(5, 1, 0)).toBe(0);
  });
});
