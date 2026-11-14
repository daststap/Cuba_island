import { useEffect } from "react";
import { create } from "zustand";
import { useIslandStore } from "../../app/store";
import { reportError, shelfList, type ShelfItem } from "../../ipc/commands";
import { onShelfChanged } from "../../ipc/events";

/** Must match `MESSAGE` in `src-tauri/src/shelf/mod.rs`. */
export const SHELF_DROP_MESSAGE = "escape://shelf-drop";

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

export interface FileDrag {
  type: "enter" | "leave" | "drop";
}

/** Steers the UI while files are dragged over the island. */
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

/** WebView2's bridge for handing dropped files (with their paths) to Rust. */
interface WebView2Bridge {
  postMessageWithAdditionalObjects?: (message: string, objects: FileList) => void;
}

function sendToRust(files: FileList): void {
  const bridge = (window as { chrome?: { webview?: WebView2Bridge } }).chrome?.webview;
  if (!bridge?.postMessageWithAdditionalObjects) {
    console.warn("[shelf] WebView2 file bridge is not available");
    return;
  }
  bridge.postMessageWithAdditionalObjects(SHELF_DROP_MESSAGE, files);
}

function carriesFiles(e: DragEvent): boolean {
  return e.dataTransfer?.types.includes("Files") ?? false;
}

/**
 * Files dragged from Explorer land in the page (WebView2 owns the drop target), so
 * the shelf listens to plain DOM drag events. Returns a cleanup function.
 */
export function listenForFileDrops(): () => void {
  // dragenter/dragleave also fire when moving between child elements.
  let depth = 0;
  const onEnter = (e: DragEvent) => {
    if (!carriesFiles(e)) return;
    e.preventDefault();
    depth += 1;
    if (depth === 1) handleFileDrag({ type: "enter" });
  };
  const onOver = (e: DragEvent) => {
    // Always cancel: otherwise WebView2 would navigate to a dropped file.
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = carriesFiles(e) ? "copy" : "none";
  };
  const onLeave = (e: DragEvent) => {
    if (!carriesFiles(e)) return;
    depth = Math.max(0, depth - 1);
    if (depth === 0) handleFileDrag({ type: "leave" });
  };
  const onDrop = (e: DragEvent) => {
    e.preventDefault();
    depth = 0;
    handleFileDrag({ type: "drop" });
    const files = e.dataTransfer?.files;
    if (files && files.length > 0) sendToRust(files);
  };
  window.addEventListener("dragenter", onEnter);
  window.addEventListener("dragover", onOver);
  window.addEventListener("dragleave", onLeave);
  window.addEventListener("drop", onDrop);
  return () => {
    window.removeEventListener("dragenter", onEnter);
    window.removeEventListener("dragover", onOver);
    window.removeEventListener("dragleave", onLeave);
    window.removeEventListener("drop", onDrop);
  };
}

/** Keeps the shelf in sync and accepts files dropped on the island. Mount once. */
export function useShelfSync(): void {
  useEffect(() => {
    reload();
    const unsubscribe = [onShelfChanged(reload), listenForFileDrops()];
    return () => {
      unsubscribe.forEach((fn) => {
        fn();
      });
    };
  }, []);
}
