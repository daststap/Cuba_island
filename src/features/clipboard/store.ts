import { useEffect } from "react";
import { create } from "zustand";
import { useIslandStore } from "../../app/store";
import type { ClipItem } from "../../ipc/commands";
import { onClipAdded } from "../../ipc/events";
import { useSettingsStore } from "../settings/store";

interface ClipboardStore {
  /** The most recent copy, shown in the "Скопировано" peek. */
  lastAdded: ClipItem | null;
  /** Bumped on every change so open lists know to reload. */
  revision: number;
  bump: () => void;
}

export const useClipboardStore = create<ClipboardStore>()((set) => ({
  lastAdded: null,
  revision: 0,
  bump: () => {
    set((s) => ({ revision: s.revision + 1 }));
  },
}));

/** Listens for new clips, peeks "Скопировано" and refreshes open lists. Mount once. */
export function useClipboardSync(): void {
  useEffect(
    () =>
      onClipAdded((item) => {
        useClipboardStore.setState((s) => ({ lastAdded: item, revision: s.revision + 1 }));
        if (useSettingsStore.getState().settings?.copyPeek !== false) {
          useIslandStore.getState().peek("clipboard");
        }
      }),
    [],
  );
}
