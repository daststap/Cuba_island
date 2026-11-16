import { useEffect, type RefObject } from "react";
import { reportError, setIslandRect, type IslandRect } from "../ipc/commands";

/** How long a move (another edge or position) animates, so the rect follows it. */
const MOVE_MS = 700;

/**
 * Reports the island element's rectangle to Rust whenever its size changes — and,
 * for `MOVE_MS` after `moveKey` changes, every frame while it slides to a new
 * place — so the cursor tracker knows where clicks must not pass through
 * (PLAN.md §4.2). Updates are coalesced to one per frame and skipped when unchanged.
 */
export function useIslandRect(ref: RefObject<HTMLElement | null>, moveKey = ""): void {
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

    // A pure move does not resize the element: follow it frame by frame for a while.
    const until = performance.now() + MOVE_MS;
    let follow: number | undefined;
    const step = () => {
      send();
      follow = performance.now() < until ? requestAnimationFrame(step) : undefined;
    };
    follow = requestAnimationFrame(step);

    return () => {
      observer.disconnect();
      if (frame !== undefined) cancelAnimationFrame(frame);
      if (follow !== undefined) cancelAnimationFrame(follow);
    };
  }, [ref, moveKey]);
}
