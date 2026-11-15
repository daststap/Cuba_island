/** Annotation shapes and how they are drawn (the preview canvas is the result). */
import { arrowHead, rectFrom, type Point, type Rect } from "./geometry";

export type Tool = "pen" | "marker" | "arrow" | "rect" | "text" | "blur" | "number";

export const TOOLS: readonly Tool[] = ["pen", "marker", "arrow", "rect", "text", "blur", "number"];

export const COLORS = {
  red: "#ff3b30",
  yellow: "#ffd60a",
  green: "#30d158",
  blue: "#0a84ff",
  white: "#ffffff",
} as const;

export type ColorName = keyof typeof COLORS;

/** Coordinates are relative to the selection, in image pixels. */
export type Shape =
  | { kind: "pen" | "marker"; color: string; points: Point[] }
  | { kind: "arrow" | "rect"; color: string; from: Point; to: Point }
  | { kind: "blur"; from: Point; to: Point }
  | { kind: "text"; color: string; at: Point; text: string }
  | { kind: "number"; color: string; at: Point; n: number };

/** Line widths and sizes at 1× scale; multiplied by the screen scale. */
const SIZE = {
  pen: 3,
  marker: 18,
  arrow: 4,
  rect: 3,
  text: 22,
  number: 14,
  blurBlock: 10,
} as const;

export const textSize = (scale: number) => SIZE.text * scale;

/** Next number for the numbering tool. */
export function nextNumber(shapes: readonly Shape[]): number {
  return shapes.filter((s) => s.kind === "number").length + 1;
}

/** Whether a finished drag is worth keeping (not a stray click). */
export function isMeaningful(shape: Shape, scale: number): boolean {
  switch (shape.kind) {
    case "pen":
    case "marker":
      return shape.points.length > 1;
    case "arrow":
    case "rect":
    case "blur": {
      const r = rectFrom(shape.from, shape.to);
      return Math.max(r.w, r.h) >= 4 * scale;
    }
    case "text":
      return shape.text.trim().length > 0;
    case "number":
      return true;
  }
}

function contrastOn(color: string): string {
  return color === COLORS.yellow || color === COLORS.white ? "#000000" : "#ffffff";
}

function strokePath(ctx: CanvasRenderingContext2D, points: readonly Point[]) {
  const [first, ...rest] = points;
  if (!first) return;
  ctx.beginPath();
  ctx.moveTo(first[0], first[1]);
  for (const p of rest) ctx.lineTo(p[0], p[1]);
  ctx.stroke();
}

/** Pixelates a region of what is already drawn (used to hide secrets). */
function pixelate(ctx: CanvasRenderingContext2D, r: Rect, block: number) {
  if (r.w < 1 || r.h < 1) return;
  const small = document.createElement("canvas");
  small.width = Math.max(1, Math.ceil(r.w / block));
  small.height = Math.max(1, Math.ceil(r.h / block));
  const sctx = small.getContext("2d");
  if (!sctx) return;
  sctx.drawImage(ctx.canvas, r.x, r.y, r.w, r.h, 0, 0, small.width, small.height);
  const smoothing = ctx.imageSmoothingEnabled;
  ctx.imageSmoothingEnabled = false;
  ctx.drawImage(small, 0, 0, small.width, small.height, r.x, r.y, r.w, r.h);
  ctx.imageSmoothingEnabled = smoothing;
}

function drawShape(ctx: CanvasRenderingContext2D, shape: Shape, scale: number) {
  ctx.save();
  ctx.lineCap = "round";
  ctx.lineJoin = "round";
  switch (shape.kind) {
    case "pen":
      ctx.strokeStyle = shape.color;
      ctx.lineWidth = SIZE.pen * scale;
      strokePath(ctx, shape.points);
      break;
    case "marker":
      ctx.globalAlpha = 0.4;
      ctx.strokeStyle = shape.color;
      ctx.lineWidth = SIZE.marker * scale;
      ctx.lineCap = "square";
      strokePath(ctx, shape.points);
      break;
    case "rect": {
      const r = rectFrom(shape.from, shape.to);
      ctx.strokeStyle = shape.color;
      ctx.lineWidth = SIZE.rect * scale;
      ctx.beginPath();
      ctx.roundRect(r.x, r.y, r.w, r.h, 4 * scale);
      ctx.stroke();
      break;
    }
    case "arrow": {
      const width = SIZE.arrow * scale;
      const head = arrowHead(shape.from, shape.to, width);
      ctx.strokeStyle = shape.color;
      ctx.fillStyle = shape.color;
      ctx.lineWidth = width;
      strokePath(ctx, [shape.from, head.shaftEnd]);
      ctx.beginPath();
      ctx.moveTo(head.tip[0], head.tip[1]);
      ctx.lineTo(head.left[0], head.left[1]);
      ctx.lineTo(head.right[0], head.right[1]);
      ctx.closePath();
      ctx.fill();
      break;
    }
    case "blur":
      pixelate(ctx, rectFrom(shape.from, shape.to), SIZE.blurBlock * scale);
      break;
    case "text": {
      ctx.font = `600 ${String(textSize(scale))}px "Segoe UI", system-ui, sans-serif`;
      ctx.textBaseline = "top";
      // A soft outline keeps text readable on any background.
      ctx.lineWidth = 3 * scale;
      ctx.strokeStyle =
        contrastOn(shape.color) === "#000000" ? "rgb(0 0 0 / 0.55)" : "rgb(255 255 255 / 0.55)";
      ctx.strokeText(shape.text, shape.at[0], shape.at[1]);
      ctx.fillStyle = shape.color;
      ctx.fillText(shape.text, shape.at[0], shape.at[1]);
      break;
    }
    case "number": {
      const radius = SIZE.number * scale;
      ctx.fillStyle = shape.color;
      ctx.beginPath();
      ctx.arc(shape.at[0], shape.at[1], radius, 0, Math.PI * 2);
      ctx.fill();
      ctx.fillStyle = contrastOn(shape.color);
      ctx.font = `700 ${String(radius * 1.2)}px "Segoe UI", system-ui, sans-serif`;
      ctx.textAlign = "center";
      ctx.textBaseline = "middle";
      ctx.fillText(String(shape.n), shape.at[0], shape.at[1] + scale);
      break;
    }
  }
  ctx.restore();
}

/** Draws the selected part of the frozen screen and every shape on top, in order. */
export function render(
  ctx: CanvasRenderingContext2D,
  source: CanvasImageSource,
  sel: Rect,
  shapes: readonly Shape[],
  scale: number,
): void {
  ctx.clearRect(0, 0, sel.w, sel.h);
  ctx.drawImage(source, sel.x, sel.y, sel.w, sel.h, 0, 0, sel.w, sel.h);
  for (const shape of shapes) drawShape(ctx, shape, scale);
}

/** `[width, height]` little-endian header + RGBA, the format Rust expects. */
export function packImage(width: number, height: number, rgba: Uint8ClampedArray): Uint8Array<ArrayBuffer> {
  const out = new Uint8Array(8 + rgba.length);
  const view = new DataView(out.buffer);
  view.setUint32(0, width, true);
  view.setUint32(4, height, true);
  out.set(rgba, 8);
  return out;
}

/** Reads the frame sent by Rust. */
export function unpackImage(buffer: ArrayBuffer): {
  width: number;
  height: number;
  rgba: Uint8ClampedArray<ArrayBuffer>;
} {
  const view = new DataView(buffer);
  const width = view.getUint32(0, true);
  const height = view.getUint32(4, true);
  return { width, height, rgba: new Uint8ClampedArray(buffer, 8, width * height * 4) };
}
