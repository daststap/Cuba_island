import { useEffect } from "react";
import { create } from "zustand";
import { useIslandStore } from "../../app/store";
import { ru } from "../../i18n/ru";
import {
  downloadsList,
  reportError,
  type Download,
  type FinishedDownload,
} from "../../ipc/commands";
import { onDownloadFinished, onDownloads } from "../../ipc/events";
import { useSettingsStore } from "../settings/store";

interface DownloadsStore {
  items: Download[];
  /** The last finished download, shown by the peek. */
  finished: FinishedDownload | null;
}

export const useDownloadsStore = create<DownloadsStore>()(() => ({
  items: [],
  finished: null,
}));

/** Downloads to show, or none when turned off in settings. */
export function useVisibleDownloads(): Download[] {
  const items = useDownloadsStore((s) => s.items);
  const enabled = useSettingsStore((s) => s.settings?.downloads !== false);
  return enabled ? items : NONE;
}

const NONE: Download[] = [];

function announce(item: FinishedDownload) {
  if (useSettingsStore.getState().settings?.downloads === false) return;
  useDownloadsStore.setState({ finished: item });
  const island = useIslandStore.getState();
  if (island.state.mode === "expanded") {
    const title = item.source === "steam" ? ru.downloads.gameReady : ru.downloads.finished;
    island.showToast(`${title}: ${item.name}`);
  } else {
    island.peek("download");
  }
}

/** Keeps the list in sync and announces finished downloads. Mount once. */
export function useDownloadsSync(): void {
  useEffect(() => {
    downloadsList()
      .then((items) => {
        useDownloadsStore.setState({ items });
      })
      .catch((error: unknown) => {
        reportError("downloads_list", error);
      });
    const unsubscribe = [
      onDownloads((items) => {
        useDownloadsStore.setState({ items });
      }),
      onDownloadFinished(announce),
    ];
    return () => {
      unsubscribe.forEach((fn) => {
        fn();
      });
    };
  }, []);
}
