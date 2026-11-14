import { convertFileSrc } from "@tauri-apps/api/core";
import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { useIslandStore } from "../../app/store";
import { ru } from "../../i18n/ru";
import {
  clipboardCopy,
  clipboardDelete,
  clipboardList,
  clipboardPaste,
  clipboardPin,
  reportError,
  type ClipItem,
} from "../../ipc/commands";
import { formatRelative, moveSelection, oneLine } from "./model";
import { useClipboardStore } from "./store";

const PAGE_SIZE = 100;

/** The history for `query`, plus the time it was loaded (for "2 мин назад"). */
function useHistory(query: string): { items: ClipItem[]; loadedAt: number } {
  const revision = useClipboardStore((s) => s.revision);
  const [history, setHistory] = useState<{ items: ClipItem[]; loadedAt: number }>({
    items: [],
    loadedAt: 0,
  });
  useEffect(() => {
    let active = true;
    clipboardList(query, PAGE_SIZE)
      .then((items) => {
        if (active) setHistory({ items, loadedAt: Date.now() });
      })
      .catch((error: unknown) => {
        reportError("clipboard_list", error);
      });
    return () => {
      active = false;
    };
  }, [query, revision]);
  return history;
}

function run(context: string, action: Promise<void>, after?: () => void) {
  action.then(after).catch((error: unknown) => {
    reportError(context, error);
  });
}

/** The "Буфер" tab: searchable history (PLAN.md stage 3). */
export function ClipboardTab() {
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(0);
  const { items, loadedAt: now } = useHistory(query);
  const inputMode = useIslandStore((s) => s.inputMode);
  const bump = useClipboardStore((s) => s.bump);
  const inputRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLUListElement>(null);

  const current = items[Math.min(selected, items.length - 1)];

  // Opened by the hotkey: the search field gets the caret right away.
  useEffect(() => {
    if (inputMode) inputRef.current?.focus();
  }, [inputMode]);

  useEffect(() => {
    listRef.current?.querySelector('[aria-selected="true"]')?.scrollIntoView({ block: "nearest" });
  }, [selected, items]);

  const close = () => {
    useIslandStore.getState().dispatch({ type: "collapse" });
  };

  const copy = (item: ClipItem) => {
    run("clipboard_copy", clipboardCopy(item.id), close);
  };

  const paste = (item: ClipItem) => {
    // Rust leaves input mode itself (returning focus) before pressing Ctrl+V.
    useIslandStore.setState({ inputMode: false });
    close();
    run("clipboard_paste", clipboardPaste(item.id));
  };

  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      setSelected((i) => moveSelection(i, e.key === "ArrowDown" ? 1 : -1, items.length));
    } else if (e.key === "Enter" && current) {
      e.preventDefault();
      paste(current);
    } else if (e.ctrlKey && e.code === "KeyP" && current) {
      e.preventDefault();
      run("clipboard_pin", clipboardPin(current.id, !current.pinned), bump);
    } else if (e.key === "Delete" && current) {
      e.preventDefault();
      run("clipboard_delete", clipboardDelete(current.id), bump);
    }
  };

  return (
    <div className="flex h-full min-h-0 w-full flex-col gap-2">
      <input
        ref={inputRef}
        type="search"
        value={query}
        placeholder={ru.clipboard.search}
        aria-label={ru.clipboard.search}
        onMouseDown={() => {
          useIslandStore.getState().enterInput();
        }}
        onChange={(e) => {
          setQuery(e.target.value);
          setSelected(0);
        }}
        onKeyDown={onKeyDown}
        className="w-full rounded-xl bg-white/10 px-3 py-2 text-sm text-[var(--island-fg)] outline-none placeholder:text-[var(--island-muted)] focus:bg-white/15"
      />

      {items.length === 0 ? (
        <div className="flex flex-1 items-center justify-center text-sm text-[var(--island-muted)]">
          {query ? ru.clipboard.nothingFound : ru.clipboard.empty}
        </div>
      ) : (
        <ul
          ref={listRef}
          role="listbox"
          aria-label={ru.clipboard.history}
          className="-mx-1 min-h-0 flex-1 overflow-y-auto px-1"
        >
          {items.map((item) => {
            const isSelected = item === current;
            return (
              <li
                key={item.id}
                role="option"
                aria-selected={isSelected}
                title={ru.clipboard.clickToCopy}
                onMouseEnter={() => {
                  setSelected(items.indexOf(item));
                }}
                onClick={() => {
                  copy(item);
                }}
                className={`flex cursor-default items-center gap-3 rounded-xl px-3 py-2 ${
                  isSelected ? "bg-white/12" : ""
                }`}
              >
                {item.kind === "image" && item.thumbPath ? (
                  <img
                    src={convertFileSrc(item.thumbPath)}
                    alt={ru.clipboard.image}
                    className="h-10 max-w-40 rounded-md object-contain"
                  />
                ) : (
                  <span className="min-w-0 flex-1 truncate text-sm">
                    {oneLine(item.text ?? "", 160)}
                  </span>
                )}
                <span className="ml-auto flex shrink-0 items-center gap-2 text-[11px] text-[var(--island-muted)]">
                  {item.pinned && (
                    <span role="img" aria-label={ru.clipboard.pinned}>
                      <svg width="12" height="12" viewBox="0 0 24 24" aria-hidden>
                        <path
                          fill="currentColor"
                          d="M16 3l5 5-3 1-4 4 1 5-2 2-4-4-5 5-1-1 5-5-4-4 2-2 5 1 4-4z"
                        />
                      </svg>
                    </span>
                  )}
                  {formatRelative(item.usedAt, now)}
                </span>
              </li>
            );
          })}
        </ul>
      )}

      <div className="text-center text-[10px] text-[var(--island-muted)]">{ru.clipboard.hint}</div>
    </div>
  );
}

/** "Скопировано" peek: icon plus the start of the text or a thumbnail. */
export function ClipboardPeek() {
  const item = useClipboardStore((s) => s.lastAdded);
  return (
    <div className="flex h-full items-center gap-3 px-4" role="status">
      <svg
        width="18"
        height="18"
        viewBox="0 0 24 24"
        aria-hidden
        className="shrink-0 text-[#30d158]"
      >
        <path
          fill="currentColor"
          d="M16 1H4a2 2 0 0 0-2 2v14h2V3h12zm3 4H8a2 2 0 0 0-2 2v14c0 1.1.9 2 2 2h11a2 2 0 0 0 2-2V7a2 2 0 0 0-2-2m0 16H8V7h11z"
        />
      </svg>
      <span className="shrink-0 text-sm font-semibold">{ru.clipboard.copied}</span>
      {item?.kind === "image" && item.thumbPath ? (
        <img
          src={convertFileSrc(item.thumbPath)}
          alt={ru.clipboard.image}
          className="ml-auto h-8 max-w-24 rounded object-contain"
        />
      ) : (
        <span className="min-w-0 truncate text-xs text-[var(--island-muted)]">
          {oneLine(item?.text ?? "", 40)}
        </span>
      )}
    </div>
  );
}
