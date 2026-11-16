import { useEffect, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { useIslandStore } from "../../app/store";
import { ru } from "../../i18n/ru";
import { launcherRun, launcherSearch, reportError, type LaunchResult } from "../../ipc/commands";
import { sendMessage } from "../chat/store";
import { moveSelection } from "../clipboard/model";

/** Latest results for `query`; stale responses are dropped. */
function useResults(query: string): LaunchResult[] {
  const [results, setResults] = useState<{ query: string; items: LaunchResult[] }>({
    query: "",
    items: [],
  });
  useEffect(() => {
    if (!query.trim()) return;
    let active = true;
    launcherSearch(query)
      .then((items) => {
        if (active) setResults({ query, items });
      })
      .catch((error: unknown) => {
        reportError("launcher_search", error);
      });
    return () => {
      active = false;
    };
  }, [query]);
  return query.trim() && results.query === query ? results.items : [];
}

function label(result: LaunchResult): { title: string; subtitle: string } {
  switch (result.kind) {
    case "app":
      return { title: result.title, subtitle: ru.launcher.app };
    case "calc":
      return { title: `= ${result.title}`, subtitle: ru.launcher.calc };
    case "web":
      return { title: ru.launcher.webTitle(result.title), subtitle: ru.launcher.webSubtitle };
    case "command":
      return { title: ru.launcher.commands[result.id] ?? result.id, subtitle: ru.launcher.command };
    case "ask":
      return { title: ru.launcher.askTitle(result.title), subtitle: ru.launcher.askSubtitle };
  }
}

const GLYPHS: Record<Exclude<LaunchResult["kind"], "app">, string> = {
  calc: "M7 2h10a2 2 0 0 1 2 2v16a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2m0 2v4h10V4zm0 6v2h2v-2zm4 0v2h2v-2zm4 0v2h2v-2zm-8 4v2h2v-2zm4 0v2h2v-2zm4 0v6h2v-6zm-8 4v2h2v-2zm4 0v2h2v-2z",
  ask: "M12 2l1.9 5.6L19.5 9.5l-5.6 1.9L12 17l-1.9-5.6L4.5 9.5l5.6-1.9zm7 11l.95 2.55L22.5 16.5l-2.55.95L19 20l-.95-2.55L15.5 16.5l2.55-.95z",
  web: "M9.5 3a6.5 6.5 0 0 1 5.25 10.33l5.46 5.46-1.42 1.42-5.46-5.46A6.5 6.5 0 1 1 9.5 3m0 2a4.5 4.5 0 1 0 0 9 4.5 4.5 0 0 0 0-9",
  command:
    "M12 15.5A3.5 3.5 0 0 1 8.5 12 3.5 3.5 0 0 1 12 8.5a3.5 3.5 0 0 1 3.5 3.5 3.5 3.5 0 0 1-3.5 3.5m7.43-2.53a7.8 7.8 0 0 0 0-1.94l2.11-1.65-2-3.46-2.49 1a7.3 7.3 0 0 0-1.69-.98L15 3.3h-4l-.36 2.64a7.3 7.3 0 0 0-1.69.98l-2.49-1-2 3.46 2.11 1.65a7.8 7.8 0 0 0 0 1.94l-2.11 1.65 2 3.46 2.49-1c.52.39 1.08.72 1.69.98L11 20.7h4l.36-2.64a7.3 7.3 0 0 0 1.69-.98l2.49 1 2-3.46z",
};

function Icon({ result }: { result: LaunchResult }): ReactNode {
  if (result.kind === "app") {
    return result.icon ? (
      <img src={result.icon} alt="" className="h-8 w-8 shrink-0" />
    ) : (
      <span className="h-8 w-8 shrink-0 rounded-lg bg-white/10" />
    );
  }
  return (
    <span className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-white/10 text-[var(--island-muted)]">
      <svg width="18" height="18" viewBox="0 0 24 24" aria-hidden>
        <path fill="currentColor" d={GLYPHS[result.kind]} />
      </svg>
    </span>
  );
}

/** Runs a result and closes the island (PLAN.md stage 4). */
function run(result: LaunchResult) {
  const store = useIslandStore.getState();
  if (result.id === "cmd:settings") {
    store.dispatch({ type: "selectTab", tab: "settings" });
    return;
  }
  // The question goes to the chat tab; focus stays in the island to read the answer.
  if (result.kind === "ask") {
    store.dispatch({ type: "selectTab", tab: "chat" });
    sendMessage(result.title);
    return;
  }
  // Apps, web search and lock take focus themselves: Rust leaves input mode
  // without giving focus back. The calculator only copies, so focus returns.
  if (result.kind !== "calc") useIslandStore.setState({ inputMode: false });
  store.dispatch({ type: "collapse" });
  launcherRun(result.id).catch((error: unknown) => {
    reportError("launcher_run", error);
  });
}

/** The "Команды" tab: launcher, calculator and web search. */
export function LauncherTab() {
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(0);
  const results = useResults(query);
  const inputMode = useIslandStore((s) => s.inputMode);
  const inputRef = useRef<HTMLInputElement>(null);
  const current = results[Math.min(selected, results.length - 1)];

  useEffect(() => {
    if (inputMode) inputRef.current?.focus();
  }, [inputMode]);

  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      setSelected((i) => moveSelection(i, e.key === "ArrowDown" ? 1 : -1, results.length));
    } else if (e.key === "Enter" && current) {
      e.preventDefault();
      run(current);
    }
  };

  return (
    <div className="flex h-full min-h-0 w-full flex-col gap-2">
      <input
        ref={inputRef}
        type="text"
        role="combobox"
        aria-expanded={results.length > 0}
        aria-controls="launcher-results"
        aria-label={ru.launcher.input}
        placeholder={ru.launcher.input}
        value={query}
        spellCheck={false}
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

      {results.length === 0 ? (
        <div className="flex flex-1 items-center justify-center px-8 text-center text-sm text-[var(--island-muted)]">
          {ru.launcher.empty}
        </div>
      ) : (
        <ul
          id="launcher-results"
          role="listbox"
          aria-label={ru.launcher.results}
          className="-mx-1 min-h-0 flex-1 overflow-y-auto px-1"
        >
          {results.map((result, index) => {
            const { title, subtitle } = label(result);
            const isSelected = result === current;
            return (
              <li
                key={result.id}
                role="option"
                aria-selected={isSelected}
                onMouseEnter={() => {
                  setSelected(index);
                }}
                onClick={() => {
                  run(result);
                }}
                className={`flex cursor-default items-center gap-3 rounded-xl px-3 py-1.5 ${
                  isSelected ? "bg-white/12" : ""
                }`}
              >
                <Icon result={result} />
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-sm">{title}</span>
                  <span className="block truncate text-[11px] text-[var(--island-muted)]">
                    {subtitle}
                  </span>
                </span>
              </li>
            );
          })}
        </ul>
      )}

      <div className="text-center text-[10px] text-[var(--island-muted)]">{ru.launcher.hint}</div>
    </div>
  );
}
