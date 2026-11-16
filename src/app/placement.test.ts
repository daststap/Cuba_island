import { describe, expect, it } from "vitest";
import { islandBox } from "./placement";

const PILL = { width: 200, height: 36, radius: 18 };
const PANEL = { width: 640, height: 360, radius: 32 };
const WIDE = { w: 800, h: 560 };
const TALL = { w: 680, h: 800 };

describe("island placement", () => {
  it("hangs from the top edge like before", () => {
    const box = islandBox(PILL, { edge: "top", anchor: 400, dragging: false }, WIDE, true);
    expect([box.left, box.top, box.width, box.height]).toEqual([300, 0, 200, 36]);
    expect([box.borderTopLeftRadius, box.borderBottomLeftRadius]).toEqual([0, 18]);
    // Before Rust says where, it centres itself.
    expect(islandBox(PILL, null, WIDE, true).left).toBe(300);
  });

  it("sits on the bottom edge with the round side up", () => {
    const box = islandBox(PANEL, { edge: "bottom", anchor: 400, dragging: false }, WIDE, false);
    expect([box.left, box.top]).toEqual([80, 200]);
    expect([box.borderTopLeftRadius, box.borderBottomRightRadius]).toEqual([32, 0]);
  });

  it("stands the pill upright on a side and turns its content", () => {
    const box = islandBox(PILL, { edge: "left", anchor: 400, dragging: false }, TALL, true);
    expect([box.left, box.top, box.width, box.height]).toEqual([0, 300, 36, 200]);
    expect(box.rotate).toBe(-90);
    const right = islandBox(PILL, { edge: "right", anchor: 400, dragging: false }, TALL, true);
    expect([right.left, right.rotate, right.borderTopRightRadius]).toEqual([644, 90, 0]);
  });

  it("slides the open panel out sideways without turning it", () => {
    const box = islandBox(PANEL, { edge: "right", anchor: 400, dragging: false }, TALL, false);
    expect([box.left, box.top, box.width, box.rotate]).toEqual([40, 220, 640, 0]);
  });

  it("keeps the island inside the window near a corner", () => {
    const box = islandBox(PANEL, { edge: "top", anchor: 96, dragging: false }, WIDE, false);
    expect(box.left).toBe(0);
    const end = islandBox(PANEL, { edge: "left", anchor: 790, dragging: false }, TALL, false);
    expect(end.top).toBe(440);
  });

  it("floats as a round pill in the middle while dragged", () => {
    const box = islandBox(PILL, { edge: "left", anchor: 0, dragging: true }, TALL, true);
    expect([box.left, box.top, box.width, box.rotate]).toEqual([240, 382, 200, 0]);
    expect(box.borderTopLeftRadius).toBe(18);
  });
});
