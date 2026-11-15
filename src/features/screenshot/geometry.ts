/** Geometry of the screenshot editor (pure; all values in image pixels). */

export type Point = readonly [number, number];

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

/** Rectangle spanned by two corners, in any drag direction. */
export function rectFrom(a: Point, b: Point): Rect {
  return {
    x: Math.min(a[0], b[0]),
    y: Math.min(a[1], b[1]),
    w: Math.abs(a[0] - b[0]),
    h: Math.abs(a[1] - b[1]),
  };
}

/** `r` clipped to `bounds` (rounded to whole pixels). */
export function clampRect(r: Rect, bounds: Rect): Rect {
  const x = Math.max(bounds.x, Math.round(r.x));
  const y = Math.max(bounds.y, Math.round(r.y));
  const right = Math.min(bounds.x + bounds.w, Math.round(r.x + r.w));
  const bottom = Math.min(bounds.y + bounds.h, Math.round(r.y + r.h));
  return { x, y, w: Math.max(0, right - x), h: Math.max(0, bottom - y) };
}

export function contains(r: Rect, [px, py]: Point): boolean {
  return px >= r.x && px <= r.x + r.w && py >= r.y && py <= r.y + r.h;
}

/** A drag shorter than this (in screen pixels) is a click. */
export const CLICK_SLOP = 4;

export function isClick(a: Point, b: Point, scale = 1): boolean {
  return Math.hypot(a[0] - b[0], a[1] - b[1]) < CLICK_SLOP * scale;
}

/**
 * Where the toolbar goes: under the selection, right-aligned; above it when
 * there is no room below; inside the bottom edge when neither fits.
 */
export function toolbarPosition(
  sel: Rect,
  bar: { w: number; h: number },
  view: { w: number; h: number },
  gap = 8,
): { x: number; y: number } {
  const x = Math.min(Math.max(0, sel.x + sel.w - bar.w), view.w - bar.w);
  if (sel.y + sel.h + gap + bar.h <= view.h) return { x, y: sel.y + sel.h + gap };
  if (sel.y - gap - bar.h >= 0) return { x, y: sel.y - gap - bar.h };
  return { x, y: sel.y + sel.h - bar.h - gap };
}

/** Arrowhead triangle at `to`, plus where the shaft should end so it stays inside. */
export function arrowHead(
  from: Point,
  to: Point,
  width: number,
): { tip: Point; left: Point; right: Point; shaftEnd: Point } {
  const angle = Math.atan2(to[1] - from[1], to[0] - from[0]);
  const length = Math.max(width * 4, 14);
  const spread = Math.PI / 7;
  const back = (a: number): Point => [
    to[0] - length * Math.cos(angle - a),
    to[1] - length * Math.sin(angle - a),
  ];
  const inset = length * 0.6;
  return {
    tip: to,
    left: back(spread),
    right: back(-spread),
    shaftEnd: [to[0] - inset * Math.cos(angle), to[1] - inset * Math.sin(angle)],
  };
}
