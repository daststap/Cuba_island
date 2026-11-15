import { describe, expect, it } from "vitest";
import { COLORS, isMeaningful, nextNumber, packImage, unpackImage, type Shape } from "./draw";

describe("screenshot drawing", () => {
  it("packs images in the format Rust expects", () => {
    const rgba = new Uint8ClampedArray([1, 2, 3, 4, 5, 6, 7, 8]);
    const packed = packImage(2, 1, rgba);
    expect(Array.from(packed.slice(0, 8))).toEqual([2, 0, 0, 0, 1, 0, 0, 0]);
    const back = unpackImage(packed.buffer);
    expect(back.width).toBe(2);
    expect(back.height).toBe(1);
    expect(Array.from(back.rgba)).toEqual(Array.from(rgba));
  });

  it("numbers steps in order", () => {
    const shapes: Shape[] = [
      { kind: "number", color: COLORS.red, at: [0, 0], n: 1 },
      { kind: "rect", color: COLORS.red, from: [0, 0], to: [9, 9] },
      { kind: "number", color: COLORS.red, at: [5, 5], n: 2 },
    ];
    expect(nextNumber(shapes)).toBe(3);
  });

  it("drops stray clicks and empty labels", () => {
    expect(isMeaningful({ kind: "arrow", color: "", from: [0, 0], to: [2, 1] }, 1)).toBe(false);
    expect(isMeaningful({ kind: "arrow", color: "", from: [0, 0], to: [20, 1] }, 1)).toBe(true);
    expect(isMeaningful({ kind: "pen", color: "", points: [[0, 0]] }, 1)).toBe(false);
    expect(isMeaningful({ kind: "text", color: "", at: [0, 0], text: "  " }, 1)).toBe(false);
  });
});
