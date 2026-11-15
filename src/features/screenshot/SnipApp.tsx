import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type PointerEvent,
  type ReactNode,
} from "react";
import { ru } from "../../i18n/ru";
import {
  reportError,
  screenshotClose,
  screenshotCopy,
  screenshotFrame,
  screenshotReady,
} from "../../ipc/commands";
import {
  COLORS,
  TOOLS,
  isMeaningful,
  nextNumber,
  packImage,
  render,
  textSize,
  unpackImage,
  type ColorName,
  type Shape,
  type Tool,
} from "./draw";
import {
  clampRect,
  contains,
  isClick,
  rectFrom,
  toolbarPosition,
  type Point,
  type Rect,
} from "./geometry";

interface Frame {
  width: number;
  height: number;
  canvas: HTMLCanvasElement;
}

interface Drag {
  mode: "select" | "draw";
  start: Point;
  current: Point;
}

const fail = (context: string) => (error: unknown) => {
  reportError(context, error);
};

function useViewport() {
  const [view, setView] = useState(() => ({ w: window.innerWidth, h: window.innerHeight }));
  useEffect(() => {
    const onResize = () => {
      setView({ w: window.innerWidth, h: window.innerHeight });
    };
    window.addEventListener("resize", onResize);
    return () => {
      window.removeEventListener("resize", onResize);
    };
  }, []);
  return view;
}

const ICONS: Record<Tool | "undo" | "cancel" | "copy", ReactNode> = {
  pen: <path d="M4 20l4.5-1L19 8.5 15.5 5 5 15.5z" />,
  marker: <path d="M8 16l-4 4h6M8 16l8.5-8.5 3.5 3.5L11.5 19.5z" />,
  arrow: (
    <>
      <path d="M5 19L18 6" />
      <path d="M10 5.5h8.5V14" />
    </>
  ),
  rect: <rect x="4" y="6" width="16" height="12" rx="2" />,
  text: <path d="M5 6h14M12 6v13" />,
  blur: (
    <path
      fill="currentColor"
      stroke="none"
      d="M4 4h5v5H4zM15 4h5v5h-5zM9.5 9.5h5v5h-5zM4 15h5v5H4zM15 15h5v5h-5z"
    />
  ),
  number: (
    <>
      <circle cx="12" cy="12" r="8.5" />
      <path d="M10.5 9.5L12.5 8v8" />
    </>
  ),
  undo: <path d="M9 14L4 9l5-5M4 9h10.5a5.5 5.5 0 010 11H11" />,
  cancel: <path d="M6 6l12 12M18 6L6 18" />,
  copy: <path d="M5 12.5l4.5 4.5L19.5 7" />,
};

function Icon({ name }: { name: keyof typeof ICONS }) {
  return (
    <svg
      viewBox="0 0 24 24"
      width="18"
      height="18"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      {ICONS[name]}
    </svg>
  );
}

function BarButton({
  label,
  pressed,
  onClick,
  className = "",
  children,
}: {
  label: string;
  pressed?: boolean;
  onClick: () => void;
  className?: string;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      title={label}
      aria-label={label}
      aria-pressed={pressed}
      onClick={onClick}
      className={`flex h-8 w-8 items-center justify-center rounded-lg transition-colors ${
        pressed ? "bg-white/20 text-white" : "text-white/75 hover:bg-white/10 hover:text-white"
      } ${className}`}
    >
      {children}
    </button>
  );
}

/** The screenshot overlay: frozen screen, region selection, drawing, copy. */
export function SnipApp() {
  const view = useViewport();
  const [frame, setFrame] = useState<Frame | null>(null);
  const [sel, setSel] = useState<Rect | null>(null);
  const [drag, setDrag] = useState<Drag | null>(null);
  const [shapes, setShapes] = useState<Shape[]>([]);
  const [draft, setDraft] = useState<Shape | null>(null);
  const [tool, setTool] = useState<Tool>("arrow");
  const [color, setColor] = useState<ColorName>("red");
  const [text, setText] = useState<{ at: Point; value: string } | null>(null);
  const [bar, setBar] = useState({ w: 0, h: 0 });
  const background = useRef<HTMLCanvasElement>(null);
  const annotations = useRef<HTMLCanvasElement>(null);
  const toolbar = useRef<HTMLDivElement>(null);
  const busy = useRef(false);
  /** Where a text label opens once the click ends (opening it on press loses focus). */
  const pendingText = useRef<Point | null>(null);

  /** Image pixels per CSS pixel (the screen's scale). */
  const scale = frame && view.w > 0 ? frame.width / view.w : 1;
  const toImage = (e: { clientX: number; clientY: number }): Point => [
    e.clientX * scale,
    e.clientY * scale,
  ];

  // Plain black page without margins or scrollbars.
  useEffect(() => {
    const { style } = document.body;
    style.margin = "0";
    style.overflow = "hidden";
    style.background = "#000";
  }, []);

  // Load the frozen screen, then let Rust show the window.
  useEffect(() => {
    screenshotFrame()
      .then((buffer) => {
        const { width, height, rgba } = unpackImage(buffer);
        const canvas = document.createElement("canvas");
        canvas.width = width;
        canvas.height = height;
        canvas.getContext("2d")?.putImageData(new ImageData(rgba, width, height), 0, 0);
        setFrame({ width, height, canvas });
      })
      .catch((error: unknown) => {
        reportError("screenshot_frame", error);
        screenshotClose().catch(fail("screenshot_close"));
      });
  }, []);

  useEffect(() => {
    if (!frame) return;
    background.current?.getContext("2d")?.drawImage(frame.canvas, 0, 0);
    screenshotReady().catch(fail("screenshot_ready"));
  }, [frame]);

  // Redraw the selection with its shapes (this canvas is the final image).
  useEffect(() => {
    if (!frame || !sel) return;
    const ctx = annotations.current?.getContext("2d");
    if (!ctx) return;
    render(ctx, frame.canvas, sel, draft ? [...shapes, draft] : shapes, scale);
  }, [frame, sel, shapes, draft, scale]);

  // Measure the toolbar to place it next to the selection.
  useLayoutEffect(() => {
    const el = toolbar.current;
    if (el && (el.offsetWidth !== bar.w || el.offsetHeight !== bar.h)) {
      setBar({ w: el.offsetWidth, h: el.offsetHeight });
    }
  }, [bar.w, bar.h, sel, drag?.mode]);

  const close = useCallback(() => {
    if (busy.current) return;
    busy.current = true;
    screenshotClose().catch(fail("screenshot_close"));
  }, []);

  const copy = useCallback(() => {
    const ctx = annotations.current?.getContext("2d");
    if (!sel || !ctx || busy.current) return;
    busy.current = true;
    const pixels = ctx.getImageData(0, 0, sel.w, sel.h);
    screenshotCopy(packImage(sel.w, sel.h, pixels.data)).catch((error: unknown) => {
      busy.current = false;
      reportError("screenshot_copy", error);
    });
  }, [sel]);

  const commitText = useCallback(() => {
    if (!text) return;
    const shape: Shape = { kind: "text", color: COLORS[color], at: text.at, text: text.value };
    if (isMeaningful(shape, scale)) setShapes((s) => [...s, shape]);
    setText(null);
  }, [text, color, scale]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (text) return; // the text field handles its own keys
      if (e.key === "Escape") {
        e.preventDefault();
        close();
      } else if (e.key === "Enter" && sel) {
        e.preventDefault();
        copy();
      } else if (e.key.toLowerCase() === "z" && e.ctrlKey) {
        e.preventDefault();
        setShapes((s) => s.slice(0, -1));
      }
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
    };
  }, [text, sel, close, copy]);

  const onPointerDown = (e: PointerEvent<HTMLDivElement>) => {
    if (!frame || busy.current) return;
    if (e.button === 2) {
      close();
      return;
    }
    if (e.button !== 0) return;
    // An open label is finished by its field losing focus (this click).
    if (text) return;
    const p = toImage(e);
    if (sel && contains(sel, p)) {
      const at: Point = [p[0] - sel.x, p[1] - sel.y];
      const paint = COLORS[color];
      if (tool === "text") {
        pendingText.current = at;
        return;
      }
      if (tool === "number") {
        setShapes((s) => [...s, { kind: "number", color: paint, at, n: nextNumber(s) }]);
        return;
      }
      e.currentTarget.setPointerCapture(e.pointerId);
      setDrag({ mode: "draw", start: p, current: p });
      setDraft(
        tool === "pen" || tool === "marker"
          ? { kind: tool, color: paint, points: [at] }
          : tool === "blur"
            ? { kind: "blur", from: at, to: at }
            : { kind: tool, color: paint, from: at, to: at },
      );
    } else if (!sel || shapes.length === 0) {
      // A fresh selection (allowed until something is drawn).
      e.currentTarget.setPointerCapture(e.pointerId);
      setSel(null);
      setDrag({ mode: "select", start: p, current: p });
    }
  };

  const onPointerMove = (e: PointerEvent<HTMLDivElement>) => {
    if (!drag) return;
    const p = toImage(e);
    setDrag({ ...drag, current: p });
    if (drag.mode !== "draw" || !draft || !sel) return;
    const at: Point = [p[0] - sel.x, p[1] - sel.y];
    if (draft.kind === "pen" || draft.kind === "marker") {
      setDraft({ ...draft, points: [...draft.points, at] });
    } else if (draft.kind === "blur") {
      setDraft({ ...draft, to: at });
    } else if (draft.kind === "arrow" || draft.kind === "rect") {
      setDraft({ ...draft, to: at });
    }
  };

  const onPointerUp = () => {
    if (pendingText.current) {
      setText({ at: pendingText.current, value: "" });
      pendingText.current = null;
      return;
    }
    if (!drag || !frame) return;
    const bounds: Rect = { x: 0, y: 0, w: frame.width, h: frame.height };
    if (drag.mode === "select") {
      const picked = isClick(drag.start, drag.current, scale)
        ? bounds
        : clampRect(rectFrom(drag.start, drag.current), bounds);
      setSel(picked.w >= 2 && picked.h >= 2 ? picked : bounds);
    } else if (draft && isMeaningful(draft, scale)) {
      setShapes((s) => [...s, draft]);
    }
    setDrag(null);
    setDraft(null);
  };

  // The rectangle on screen: the live drag while selecting, else the selection.
  const shown = drag?.mode === "select" ? rectFrom(drag.start, drag.current) : sel;
  const css = (r: Rect) => ({
    left: r.x / scale,
    top: r.y / scale,
    width: r.w / scale,
    height: r.h / scale,
  });
  /** The selection being annotated (not while a new one is dragged out). */
  const editing = drag?.mode === "select" ? null : sel;
  const barAt = editing ? toolbarPosition(css2rect(css(editing)), bar, view) : null;

  return (
    <div
      className="fixed inset-0 cursor-crosshair overflow-hidden select-none"
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onContextMenu={(e) => {
        e.preventDefault();
      }}
    >
      {frame && (
        <canvas
          ref={background}
          width={frame.width}
          height={frame.height}
          className="absolute inset-0 h-full w-full"
        />
      )}

      {shown ? (
        <div
          className="pointer-events-none absolute outline outline-1 outline-white/80"
          style={{ ...css(shown), boxShadow: "0 0 0 9999px rgb(0 0 0 / 0.45)" }}
        />
      ) : (
        <div className="pointer-events-none absolute inset-0 bg-black/30" />
      )}

      {!shown && frame && (
        <div className="pointer-events-none absolute top-6 left-1/2 -translate-x-1/2 rounded-full bg-black/75 px-4 py-2 text-sm text-white">
          {ru.screenshot.hint}
        </div>
      )}

      {shown && (
        <div
          className="pointer-events-none absolute rounded-md bg-black/75 px-2 py-0.5 text-xs text-white tabular-nums"
          style={{ left: shown.x / scale, top: Math.max(0, shown.y / scale - 24) }}
        >
          {ru.screenshot.size(Math.round(shown.w), Math.round(shown.h))}
        </div>
      )}

      {editing && (
        <canvas
          ref={annotations}
          width={editing.w}
          height={editing.h}
          aria-label={ru.screenshot.take}
          className="absolute"
          style={css(editing)}
        />
      )}

      {editing && text && (
        <input
          autoFocus
          value={text.value}
          placeholder={ru.screenshot.textPlaceholder}
          aria-label={ru.screenshot.text}
          onPointerDown={(e) => {
            e.stopPropagation();
          }}
          onChange={(e) => {
            setText({ ...text, value: e.target.value });
          }}
          onKeyDown={(e) => {
            e.stopPropagation();
            if (e.key === "Enter") commitText();
            if (e.key === "Escape") setText(null);
          }}
          onBlur={commitText}
          className="absolute min-w-24 bg-transparent font-semibold outline-none placeholder:text-white/40"
          style={{
            left: (editing.x + text.at[0]) / scale,
            top: (editing.y + text.at[1]) / scale,
            fontSize: textSize(scale) / scale,
            lineHeight: 1,
            color: COLORS[color],
          }}
        />
      )}

      {editing && barAt && (
        <div
          ref={toolbar}
          role="toolbar"
          aria-label={ru.screenshot.tools}
          onPointerDown={(e) => {
            e.stopPropagation();
          }}
          className="absolute flex cursor-default items-center gap-0.5 rounded-xl bg-[#1c1c1e]/95 p-1.5 shadow-lg ring-1 ring-white/10"
          style={{ left: barAt.x, top: barAt.y }}
        >
          {TOOLS.map((t) => (
            <BarButton
              key={t}
              label={ru.screenshot[t]}
              pressed={tool === t}
              onClick={() => {
                setTool(t);
              }}
            >
              <Icon name={t} />
            </BarButton>
          ))}
          <span className="mx-1 h-5 w-px bg-white/15" aria-hidden />
          {(Object.keys(COLORS) as ColorName[]).map((c) => (
            <BarButton
              key={c}
              label={ru.screenshot.color(ru.screenshot.colors[c] ?? c)}
              pressed={color === c}
              onClick={() => {
                setColor(c);
              }}
            >
              <span
                className={`h-4 w-4 rounded-full ${color === c ? "ring-2 ring-white ring-offset-1 ring-offset-[#1c1c1e]" : ""}`}
                style={{ background: COLORS[c] }}
              />
            </BarButton>
          ))}
          <span className="mx-1 h-5 w-px bg-white/15" aria-hidden />
          <BarButton
            label={ru.screenshot.undo}
            onClick={() => {
              setShapes((s) => s.slice(0, -1));
            }}
          >
            <Icon name="undo" />
          </BarButton>
          <BarButton label={ru.screenshot.cancel} onClick={close}>
            <Icon name="cancel" />
          </BarButton>
          <BarButton
            label={ru.screenshot.copy}
            onClick={copy}
            className="bg-[#30d158] text-black hover:bg-[#30d158]/85 hover:text-black"
          >
            <Icon name="copy" />
          </BarButton>
        </div>
      )}
    </div>
  );
}

function css2rect(c: { left: number; top: number; width: number; height: number }): Rect {
  return { x: c.left, y: c.top, w: c.width, h: c.height };
}
