import { describe, expect, it } from "vitest";
import { arrowHead, clampRect, contains, isClick, rectFrom, toolbarPosition } from "./geometry";

describe("screenshot geometry", () => {
  it("builds a rectangle from a drag in any direction", () => {
    expect(rectFrom([50, 40], [10, 100])).toEqual({ x: 10, y: 40, w: 40, h: 60 });
  });

  it("clips and rounds to the screen", () => {
    const screen = { x: 0, y: 0, w: 100, h: 80 };
    expect(clampRect({ x: -5.4, y: 10.6, w: 200, h: 20 }, screen)).toEqual({
      x: 0,
      y: 11,
      w: 100,
      h: 20,
    });
  });

  it("tells clicks from drags", () => {
    expect(isClick([10, 10], [12, 11])).toBe(true);
    expect(isClick([10, 10], [20, 10])).toBe(false);
    // On a 2× screen the slop is in screen pixels.
    expect(isClick([10, 10], [16, 10], 2)).toBe(true);
    expect(contains({ x: 0, y: 0, w: 10, h: 10 }, [10, 5])).toBe(true);
  });

  it("puts the toolbar below, above or inside the selection", () => {
    const bar = { w: 100, h: 40 };
    const view = { w: 800, h: 600 };
    expect(toolbarPosition({ x: 100, y: 100, w: 300, h: 200 }, bar, view)).toEqual({
      x: 300,
      y: 308,
    });
    expect(toolbarPosition({ x: 100, y: 300, w: 300, h: 290 }, bar, view)).toEqual({
      x: 300,
      y: 252,
    });
    expect(toolbarPosition({ x: 0, y: 0, w: 800, h: 600 }, bar, view)).toEqual({
      x: 700,
      y: 552,
    });
  });

  it("points the arrowhead at the end of the drag", () => {
    const head = arrowHead([0, 0], [100, 0], 4);
    expect(head.tip).toEqual([100, 0]);
    expect(head.left[0]).toBeLessThan(100);
    expect(head.left[1]).toBeCloseTo(-head.right[1]);
    expect(head.shaftEnd[0]).toBeLessThan(100);
  });
});
