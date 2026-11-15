import { useEffect, useRef, useState, type KeyboardEvent, type PointerEvent } from "react";

/** Row height of a wheel, px. */
export const WHEEL_ROW = 28;
const VISIBLE_ROWS = 5;
export const WHEEL_HEIGHT = WHEEL_ROW * VISIBLE_ROWS;
/** Touchpad scroll that adds up to one step. */
const SCROLL_STEP_PX = 40;
/** One mouse-wheel notch, px (Chromium on Windows). */
const NOTCH_PX = 100;
/** Pointer travel below which a press counts as a click. */
const CLICK_SLOP_PX = 4;

interface WheelProps {
  value: number;
  max: number;
  label: string;
  unit: string;
  onChange: (value: number) => void;
}

const clamp = (v: number, max: number) => Math.min(max, Math.max(0, v));

/**
 * An iOS-style picker drum: drag it, turn the mouse wheel (one notch = one
 * step) or click a number. Keyboard works like a spin button.
 */
export function Wheel({ value, max, label, unit, onChange }: WheelProps) {
  const ref = useRef<HTMLDivElement>(null);
  const [drag, setDrag] = useState<{ startY: number; dy: number } | null>(null);
  const latest = useRef({ value, max, onChange });
  useEffect(() => {
    latest.current = { value, max, onChange };
  });

  // React wheel listeners are passive; this one must stop the event.
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    let acc = 0;
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      acc += e.deltaY;
      if (Math.abs(acc) < SCROLL_STEP_PX) return;
      const { value: v, max: m, onChange: change } = latest.current;
      const steps = Math.max(1, Math.round(Math.abs(acc) / NOTCH_PX));
      const next = clamp(v + Math.sign(acc) * steps, m);
      acc = 0;
      if (next === v) return;
      // Fast spins fire several events before React re-renders.
      latest.current = { ...latest.current, value: next };
      change(next);
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => {
      el.removeEventListener("wheel", onWheel);
    };
  }, []);

  const set = (next: number) => {
    const v = clamp(next, max);
    if (v !== value) onChange(v);
  };

  const onPointerDown = (e: PointerEvent<HTMLDivElement>) => {
    e.currentTarget.setPointerCapture(e.pointerId);
    setDrag({ startY: e.clientY, dy: 0 });
  };
  const onPointerMove = (e: PointerEvent<HTMLDivElement>) => {
    if (drag) setDrag({ ...drag, dy: e.clientY - drag.startY });
  };
  const onPointerUp = (e: PointerEvent<HTMLDivElement>) => {
    if (!drag) return;
    setDrag(null);
    if (Math.abs(drag.dy) < CLICK_SLOP_PX) {
      const rect = e.currentTarget.getBoundingClientRect();
      set(value + Math.round((e.clientY - rect.top - WHEEL_HEIGHT / 2) / WHEEL_ROW));
    } else {
      set(Math.round(value - drag.dy / WHEEL_ROW));
    }
  };

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const step = { ArrowUp: 1, ArrowDown: -1, PageUp: 5, PageDown: -5 }[e.key];
    if (step !== undefined) set(value + step);
    else if (e.key === "Home") set(0);
    else if (e.key === "End") set(max);
    else return;
    e.preventDefault();
  };

  // Continuous position (px) while dragging, with a little give past the ends.
  const offset = Math.min(
    max * WHEEL_ROW + WHEEL_ROW,
    Math.max(-WHEEL_ROW, value * WHEEL_ROW - (drag?.dy ?? 0)),
  );

  return (
    <div
      ref={ref}
      role="spinbutton"
      tabIndex={0}
      aria-label={label}
      aria-valuenow={value}
      aria-valuemin={0}
      aria-valuemax={max}
      aria-valuetext={`${String(value)} ${unit}`}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={() => {
        setDrag(null);
      }}
      onKeyDown={onKeyDown}
      className="relative w-16 cursor-grab touch-none overflow-hidden outline-none select-none active:cursor-grabbing"
      style={{ height: WHEEL_HEIGHT }}
    >
      <div
        aria-hidden
        className="absolute inset-x-0 top-0"
        style={{
          transform: `translateY(${String(WHEEL_HEIGHT / 2 - WHEEL_ROW / 2 - offset)}px)`,
          transition: drag ? "none" : "transform 180ms ease-out",
        }}
      >
        {Array.from({ length: max + 1 }, (_, i) => {
          const distance = Math.abs(i * WHEEL_ROW - offset) / WHEEL_ROW;
          if (distance > VISIBLE_ROWS / 2 + 1) return <div key={i} style={{ height: WHEEL_ROW }} />;
          return (
            <div
              key={i}
              className="w-7 text-right text-xl tabular-nums"
              style={{
                height: WHEEL_ROW,
                lineHeight: `${String(WHEEL_ROW)}px`,
                opacity: Math.max(0.2, 1 - distance * 0.35),
                transform: `scale(${String(1 - Math.min(distance, 2.5) * 0.06)})`,
              }}
            >
              {i}
            </div>
          );
        })}
      </div>
      <span
        aria-hidden
        className="pointer-events-none absolute left-8 text-xs font-semibold"
        style={{ top: WHEEL_HEIGHT / 2 - WHEEL_ROW / 2, lineHeight: `${String(WHEEL_ROW)}px` }}
      >
        {unit}
      </span>
    </div>
  );
}
