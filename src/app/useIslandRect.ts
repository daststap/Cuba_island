import { useEffect, type RefObject } from "react";
import { reportError, setIslandRect, type IslandRect } from "../ipc/commands";

/**
 * Reports the island element's rectangle to Rust whenever its size changes, so the
 * cursor tracker knows where clicks must not pass through (PLAN.md §4.2).
 * Updates are coalesced to one per animation frame and skipped when unchanged.
 */
export function useIslandRect(ref: RefObject<HTMLElement | null>): void {
  useEffect(() => {
    const element = ref.current;
    if (!element) return;

    let frame: number | undefined;
    let last: IslandRect | undefined;

    const send = () => {
      frame = undefined;
      const r = element.getBoundingClientRect();
      const rect: IslandRect = {
        x: Math.floor(r.left),
        y: Math.floor(r.top),
        w: Math.ceil(r.width),
        h: Math.ceil(r.height),
      };
      if (last && last.x === rect.x && last.y === rect.y && last.w === rect.w && last.h === rect.h)
        return;
      last = rect;
      setIslandRect(rect).catch((error: unknown) => {
        reportError("set_island_rect", error);
      });
    };
    const schedule = () => {
      frame ??= requestAnimationFrame(send);
    };

    const observer = new ResizeObserver(schedule);
    observer.observe(element);
    schedule();
    return () => {
      observer.disconnect();
      if (frame !== undefined) cancelAnimationFrame(frame);
    };
  }, [ref]);
}
