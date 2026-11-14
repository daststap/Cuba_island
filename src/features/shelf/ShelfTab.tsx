import { convertFileSrc } from "@tauri-apps/api/core";
import { useRef, type PointerEvent } from "react";
import { ru } from "../../i18n/ru";
import {
  reportError,
  shelfClear,
  shelfDragOut,
  shelfOpen,
  shelfRemove,
  type ShelfItem,
} from "../../ipc/commands";
import { formatSize, movedEnough } from "./model";
import { useShelfStore } from "./store";

const FOLDER_ICON = "M10 4H4a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-8z";
const FILE_ICON = "M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8zm-1 7V3.5L18.5 9z";

function Preview({ item }: { item: ShelfItem }) {
  if (item.imagePath) {
    return (
      <img
        src={convertFileSrc(item.imagePath)}
        alt=""
        draggable={false}
        className="h-12 w-12 object-contain"
      />
    );
  }
  return (
    <svg width="40" height="40" viewBox="0 0 24 24" aria-hidden className="text-white/40">
      <path fill="currentColor" d={item.isDir ? FOLDER_ICON : FILE_ICON} />
    </svg>
  );
}

function fail(context: string) {
  return (error: unknown) => {
    reportError(context, error);
  };
}

function Tile({ item }: { item: ShelfItem }) {
  const press = useRef<{ x: number; y: number } | null>(null);

  const onPointerDown = (e: PointerEvent) => {
    if (e.button === 0 && !item.missing) press.current = { x: e.clientX, y: e.clientY };
  };
  const onPointerMove = (e: PointerEvent) => {
    const start = press.current;
    if (!start || !movedEnough(e.clientX - start.x, e.clientY - start.y)) return;
    press.current = null;
    // Hand over to the system drag (the mouse button is still down).
    shelfDragOut([item.id]).catch(fail("shelf_drag_out"));
  };
  const subtitle = item.missing
    ? ru.shelf.missing
    : item.isDir
      ? ru.shelf.folder
      : item.size !== null
        ? formatSize(item.size)
        : "";

  return (
    <li
      title={item.path}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={() => {
        press.current = null;
      }}
      onDoubleClick={() => {
        if (!item.missing) shelfOpen(item.id).catch(fail("shelf_open"));
      }}
      className={`group relative flex w-24 cursor-grab flex-col items-center gap-1 rounded-xl p-2 select-none hover:bg-white/10 ${
        item.missing ? "opacity-40" : ""
      }`}
    >
      <Preview item={item} />
      <span className="w-full truncate text-center text-[11px]">{item.name}</span>
      <span className="text-[10px] text-[var(--island-muted)]">{subtitle}</span>
      <button
        type="button"
        aria-label={ru.shelf.remove(item.name)}
        title={ru.shelf.remove(item.name)}
        onPointerDown={(e) => {
          e.stopPropagation();
        }}
        onClick={() => {
          shelfRemove([item.id]).catch(fail("shelf_remove"));
        }}
        className="absolute top-1 right-1 hidden h-5 w-5 items-center justify-center rounded-full bg-white/20 text-xs leading-none group-hover:flex hover:bg-white/35"
      >
        ×
      </button>
    </li>
  );
}

/** The "Полка" tab: files parked on the island (PLAN.md stage 6). */
export function ShelfTab() {
  const items = useShelfStore((s) => s.items);
  const dragging = useShelfStore((s) => s.dragging);

  return (
    <div className="relative flex h-full min-h-0 w-full flex-col gap-2">
      {items.length === 0 ? (
        <div className="flex flex-1 items-center justify-center rounded-2xl border border-dashed border-white/20 px-10 text-center text-sm text-[var(--island-muted)]">
          {ru.shelf.empty}
        </div>
      ) : (
        <>
          <ul
            aria-label={ru.shelf.label}
            className="flex min-h-0 flex-1 flex-wrap content-start gap-1 overflow-y-auto"
          >
            {items.map((item) => (
              <Tile key={item.id} item={item} />
            ))}
          </ul>
          <div className="flex items-center justify-between text-[10px] text-[var(--island-muted)]">
            <span>{ru.shelf.hint}</span>
            <button
              type="button"
              onClick={() => {
                shelfClear().catch(fail("shelf_clear"));
              }}
              className="rounded-lg px-2 py-1 hover:bg-white/10 hover:text-[var(--island-fg)]"
            >
              {ru.shelf.clear}
            </button>
          </div>
        </>
      )}
      {dragging && (
        <div className="pointer-events-none absolute inset-0 flex items-center justify-center rounded-2xl border-2 border-dashed border-[#0a84ff] bg-[#0a84ff]/15 text-sm font-semibold">
          {ru.shelf.dropHere}
        </div>
      )}
    </div>
  );
}

/** Count badge for the pill while files wait on the shelf. */
export function ShelfBadge() {
  const count = useShelfStore((s) => s.items.length);
  if (count === 0) return null;
  return (
    <span
      role="img"
      aria-label={ru.shelf.count(count)}
      title={ru.shelf.count(count)}
      className="flex items-center gap-0.5 text-[11px] font-semibold text-[var(--island-muted)]"
    >
      <svg width="12" height="12" viewBox="0 0 24 24" aria-hidden>
        <path fill="currentColor" d={FOLDER_ICON} />
      </svg>
      {count}
    </span>
  );
}
