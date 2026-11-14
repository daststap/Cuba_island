import { useEffect } from "react";
import { create } from "zustand";
import { useIslandStore } from "../../app/store";
import { mediaGetState, reportError, type MediaState } from "../../ipc/commands";
import { onMediaChanged } from "../../ipc/events";
import { shouldPeek } from "./model";

interface MediaStore {
  media: MediaState | null;
}

export const useMediaStore = create<MediaStore>()(() => ({ media: null }));

function apply(next: MediaState | null, isInitial: boolean): void {
  const prev = useMediaStore.getState().media;
  useMediaStore.setState({ media: next });
  if (shouldPeek(prev, next, isInitial)) useIslandStore.getState().peek("media");
}

/** Keeps `useMediaStore` in sync with Rust and peeks on track changes. Mount once. */
export function useMediaSync(): void {
  useEffect(() => {
    let active = true;
    let gotEvent = false;
    const unsubscribe = onMediaChanged((media) => {
      gotEvent = true;
      apply(media, false);
    });
    mediaGetState()
      .then((media) => {
        // An event that already arrived is newer than this snapshot.
        if (active && !gotEvent) apply(media, true);
      })
      .catch((error: unknown) => {
        reportError("media_get_state", error);
      });
    return () => {
      active = false;
      unsubscribe();
    };
  }, []);
}
