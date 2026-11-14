import { useEffect } from "react";
import { create } from "zustand";
import { useIslandStore } from "../../app/store";
import { claudeStatus, reportError, type ClaudeSession } from "../../ipc/commands";
import { onClaudeAttention, onClaudeStatus } from "../../ipc/events";

interface ClaudeStore {
  sessions: ClaudeSession[];
  /** The session that most recently started waiting for the user (for the peek). */
  attention: ClaudeSession | null;
}

export const useClaudeStore = create<ClaudeStore>()(() => ({ sessions: [], attention: null }));

/** The state the pill badge shows: waiting beats working; idle shows nothing. */
export function badgeState(sessions: ClaudeSession[]): "waiting" | "working" | null {
  if (sessions.some((s) => s.state === "waiting")) return "waiting";
  if (sessions.some((s) => s.state === "working")) return "working";
  return null;
}

/** Follows Claude Code sessions and peeks when one waits for the user. Mount once. */
export function useClaudeSync(): void {
  useEffect(() => {
    let active = true;
    let gotEvent = false;
    const unsubscribe = [
      onClaudeStatus((sessions) => {
        gotEvent = true;
        useClaudeStore.setState({ sessions });
      }),
      onClaudeAttention((session) => {
        if (!session) return;
        useClaudeStore.setState({ attention: session });
        useIslandStore.getState().peek("claude");
      }),
    ];
    claudeStatus()
      .then((sessions) => {
        // An event that already arrived is newer than this snapshot.
        if (active && !gotEvent) useClaudeStore.setState({ sessions });
      })
      .catch((error: unknown) => {
        reportError("claude_status", error);
      });
    return () => {
      active = false;
      unsubscribe.forEach((fn) => {
        fn();
      });
    };
  }, []);
}
