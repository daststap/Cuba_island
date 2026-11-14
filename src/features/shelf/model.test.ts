import { describe, expect, it } from "vitest";
import { formatSize, movedEnough } from "./model";

describe("formatSize", () => {
  it("picks a readable unit", () => {
    expect(formatSize(812)).toBe("812 Б");
    expect(formatSize(4300)).toBe("4,2 КБ");
    expect(formatSize(18 * 1024 * 1024)).toBe("18 МБ");
    expect(formatSize(1.3 * 1024 ** 3)).toBe("1,3 ГБ");
  });
});

describe("movedEnough", () => {
  it("ignores jitter but starts a drag on real movement", () => {
    expect(movedEnough(1, 2)).toBe(false);
    expect(movedEnough(3, 3)).toBe(true);
  });
});
