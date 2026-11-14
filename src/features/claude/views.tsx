import { useEffect, useState } from "react";
import { ru } from "../../i18n/ru";
import {
  claudeHooksSet,
  claudeHooksStatus,
  reportError,
  type ClaudeHooksStatus,
  type ClaudeSession,
} from "../../ipc/commands";
import { badgeState, useClaudeStore } from "./store";

const CLAUDE_ORANGE = "#d97757";
const SPARK =
  "M12 2.5l1.6 6.1 6.1-2.6-4.3 5 5.1 3.7-6.3.3 1 6.3-3.2-5.5-3.2 5.5 1-6.3-6.3-.3 5.1-3.7-4.3-5 6.1 2.6z";

function Spark({ size, className }: { size: number; className?: string }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden className={className}>
      <path fill={CLAUDE_ORANGE} d={SPARK} />
    </svg>
  );
}

/** Small mark in the pill: spinning while Claude works, pulsing while it waits. */
export function ClaudeBadge() {
  const state = useClaudeStore((s) => badgeState(s.sessions));
  if (!state) return null;
  const label = state === "waiting" ? ru.claude.waitingShort : ru.claude.workingShort;
  return (
    <span role="img" aria-label={label} title={label} className="flex items-center">
      <Spark
        size={16}
        className={state === "working" ? "animate-[spin_2.4s_linear_infinite]" : "animate-pulse"}
      />
    </span>
  );
}

/** Peek when a session needs the user. */
export function ClaudePeek() {
  const session = useClaudeStore((s) => s.attention);
  return (
    <div role="alert" className="flex h-full items-center gap-3 px-5">
      <Spark size={22} className="shrink-0 animate-pulse" />
      <div className="min-w-0 flex-1">
        <div className="truncate text-sm font-semibold">
          {ru.claude.waiting}
          {session && <span className="text-[var(--island-muted)]"> · {session.project}</span>}
        </div>
        {session?.message && (
          <div className="truncate text-xs text-[var(--island-muted)]">{session.message}</div>
        )}
      </div>
    </div>
  );
}

function describe(session: ClaudeSession): string {
  if (session.state === "waiting") return session.message ?? ru.claude.waiting;
  if (session.tool) return ru.claude.usingTool(session.tool);
  return ru.claude.working;
}

/** Active sessions on the home tab. */
export function ClaudeSessions() {
  // Select the stored array (stable) and filter here: a selector returning a new
  // array on every call would re-render forever.
  const sessions = useClaudeStore((s) => s.sessions).filter((x) => x.state !== "idle");
  if (sessions.length === 0) return null;
  return (
    <ul aria-label={ru.claude.sessions} className="flex w-full flex-col gap-1">
      {sessions.map((s) => (
        <li
          key={s.id}
          className="flex items-center gap-2 rounded-lg bg-white/6 px-3 py-1.5 text-xs"
        >
          <Spark
            size={14}
            className={
              s.state === "working" ? "animate-[spin_2.4s_linear_infinite]" : "animate-pulse"
            }
          />
          <span className="font-semibold">{s.project}</span>
          <span
            className={`min-w-0 truncate ${
              s.state === "waiting" ? "text-[#ffb37a]" : "text-[var(--island-muted)]"
            }`}
          >
            {describe(s)}
          </span>
        </li>
      ))}
    </ul>
  );
}

/** Settings row: connect or disconnect Claude Code hooks. */
export function ClaudeHooksSetting({ className }: { className: string }) {
  const [status, setStatus] = useState<ClaudeHooksStatus | null>(null);
  useEffect(() => {
    claudeHooksStatus()
      .then(setStatus)
      .catch((error: unknown) => {
        reportError("claude_hooks_status", error);
      });
  }, []);
  if (!status) return null;
  return (
    <div className="flex items-center gap-2">
      <span
        className="max-w-48 truncate text-xs text-[var(--island-muted)]"
        title={status.settingsPath}
      >
        {status.error ?? (status.installed ? ru.claude.connected : ru.claude.notConnected)}
      </span>
      <button
        type="button"
        className={className}
        title={status.settingsPath}
        onClick={() => {
          claudeHooksSet(!status.installed)
            .then(setStatus)
            .catch((error: unknown) => {
              reportError("claude_hooks_set", error);
            });
        }}
      >
        {status.installed ? ru.claude.disconnect : ru.claude.connect}
      </button>
    </div>
  );
}
