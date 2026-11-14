import { useEffect } from "react";
import { create } from "zustand";
import { useIslandStore } from "../../app/store";
import { reportError, shelfList, type ShelfItem } from "../../ipc/commands";
import { onFileDrag, onShelfChanged, type FileDrag } from "../../ipc/events";

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

/**
 * Reacts to files dragged over / dropped on the island. Rust stores dropped files
 * itself and then sends `shelf://changed`, so here we only steer the UI.
 */
export function handleFileDrag(event: FileDrag): void {
  switch (event.type) {
    case "enter":
      useShelfStore.setState({ dragging: true });
      // Open right on the shelf so there is a big target to drop on.
      useIslandStore.getState().dispatch({ type: "expand", tab: "shelf" });
      break;
    case "leave":
    case "drop":
      useShelfStore.setState({ dragging: false });
      break;
  }
}

/** Keeps the shelf in sync and follows file drags over the island. Mount once. */
export function useShelfSync(): void {
  useEffect(() => {
    reload();
    const unsubscribe = [onShelfChanged(reload), onFileDrag(handleFileDrag)];
    return () => {
      unsubscribe.forEach((fn) => {
        fn();
      });
    };
  }, []);
}
