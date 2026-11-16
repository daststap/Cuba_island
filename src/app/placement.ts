/** Where the island sits inside its window for each screen edge (pure). */
import type { IslandPlacement, ScreenEdge } from "../ipc/commands";

export interface Shape {
  width: number;
  height: number;
  radius: number;
}

export interface IslandBox {
  left: number;
  top: number;
  width: number;
  height: number;
  /** Corners touching the screen edge stay square. */
  borderTopLeftRadius: number;
  borderTopRightRadius: number;
  borderBottomLeftRadius: number;
  borderBottomRightRadius: number;
  /** The pill's content is turned to run along a side edge (degrees). */
  rotate: number;
}

export const isSide = (edge: ScreenEdge) => edge === "left" || edge === "right";

const clamp = (value: number, min: number, max: number) =>
  Math.min(Math.max(value, min), Math.max(min, max));

/**
 * The island's box in the window (logical px). On a side edge the collapsed pill
 * stands upright; peeks and the open panel keep their shape and slide out sideways.
 * While dragging, the pill floats in the middle of the window, under the cursor.
 */
export function islandBox(
  shape: Shape,
  placement: IslandPlacement | null,
  view: { w: number; h: number },
  compact: boolean,
): IslandBox {
  const edge = placement?.edge ?? "top";
  const r = shape.radius;
  if (placement?.dragging) {
    return {
      left: (view.w - shape.width) / 2,
      top: (view.h - shape.height) / 2,
      width: shape.width,
      height: shape.height,
      borderTopLeftRadius: r,
      borderTopRightRadius: r,
      borderBottomLeftRadius: r,
      borderBottomRightRadius: r,
      rotate: 0,
    };
  }
  const upright = compact && isSide(edge);
  const width = upright ? shape.height : shape.width;
  const height = upright ? shape.width : shape.height;
  const anchor = placement?.anchor ?? (isSide(edge) ? view.h : view.w) / 2;
  const alongX = clamp(anchor - width / 2, 0, view.w - width);
  const alongY = clamp(anchor - height / 2, 0, view.h - height);
  const corners = (tl: number, tr: number, bl: number, br: number) => ({
    borderTopLeftRadius: tl,
    borderTopRightRadius: tr,
    borderBottomLeftRadius: bl,
    borderBottomRightRadius: br,
  });
  switch (edge) {
    case "top":
      return { left: alongX, top: 0, width, height, ...corners(0, 0, r, r), rotate: 0 };
    case "bottom":
      return {
        left: alongX,
        top: view.h - height,
        width,
        height,
        ...corners(r, r, 0, 0),
        rotate: 0,
      };
    case "left":
      return {
        left: 0,
        top: alongY,
        width,
        height,
        ...corners(0, r, 0, r),
        rotate: upright ? -90 : 0,
      };
    case "right":
      return {
        left: view.w - width,
        top: alongY,
        width,
        height,
        ...corners(r, 0, r, 0),
        rotate: upright ? 90 : 0,
      };
  }
}
