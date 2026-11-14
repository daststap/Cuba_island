import { useEffect } from "react";
import { create } from "zustand";
import { useIslandStore } from "../../app/store";
import { reportError, shelfAdd, shelfList, type ShelfItem } from "../../ipc/commands";
import { onFileDrop, onShelfChanged, type FileDrop } from "../../ipc/events";

interface ShelfStore {
  items: ShelfItem[];
  /** Files are being dragged over the island. */
  dragging: boolean;
}

export const useShelfStore = create<ShelfStore>()(() => ({ items: [], dragging: false }));

function reload() {
  shelfList()
    .then((items) => {
      useShelfStore.setState({ items });
    })
    .catch((error: unknown) => {
      reportError("shelf_list", error);
    });
}

/** Reacts to files dragged over / dropped on the island. */
export function handleFileDrop(event: FileDrop): void {
  const island = useIslandStore.getState();
  switch (event.type) {
    case "enter":
      useShelfStore.setState({ dragging: true });
      // Open right on the shelf so there is a big target to drop on.
      island.dispatch({ type: "expand", tab: "shelf" });
      break;
    case "leave":
      useShelfStore.setState({ dragging: false });
      break;
    case "drop":
      useShelfStore.setState({ dragging: false });
      if (event.paths.length > 0) {
        shelfAdd(event.paths).catch((error: unknown) => {
          reportError("shelf_add", error);
        });
      }
      break;
    case "over":
      break;
  }
}

/** Keeps the shelf in sync and accepts files dropped on the island. Mount once. */
export function useShelfSync(): void {
  useEffect(() => {
    reload();
    const unsubscribe = [onShelfChanged(reload), onFileDrop(handleFileDrop)];
    return () => {
      unsubscribe.forEach((fn) => {
        fn();
      });
    };
  }, []);
}
