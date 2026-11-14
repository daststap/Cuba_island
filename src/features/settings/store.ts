import { useEffect } from "react";
import { create } from "zustand";
import {
  reportError,
  settingsGet,
  settingsSet,
  updateCheck,
  type Settings,
  type UpdateInfo,
} from "../../ipc/commands";
import { onSettingsChanged } from "../../ipc/events";

interface SettingsStore {
  /** `null` until loaded from Rust. */
  settings: Settings | null;
  /** A newer version found on startup. */
  update: UpdateInfo | null;
}

export const useSettingsStore = create<SettingsStore>()(() => ({ settings: null, update: null }));

/** Saves a change; on failure (e.g. hotkey taken) shows the error and keeps the old value. */
export async function updateSettings(patch: Partial<Settings>): Promise<boolean> {
  try {
    useSettingsStore.setState({ settings: await settingsSet(patch) });
    return true;
  } catch (error: unknown) {
    reportError("settings_set", error);
    return false;
  }
}

/** Delay before the background update check, so startup stays quick. */
const UPDATE_CHECK_DELAY_MS = 5000;

/** Loads settings, follows changes and checks for updates once. Mount once. */
export function useSettingsSync(onUpdate: (update: UpdateInfo) => void): void {
  useEffect(() => {
    let active = true;
    const unsubscribe = onSettingsChanged((settings) => {
      useSettingsStore.setState({ settings });
    });
    settingsGet()
      .then((settings) => {
        if (active) useSettingsStore.setState({ settings });
      })
      .catch((error: unknown) => {
        reportError("settings_get", error);
      });
    const timer = setTimeout(() => {
      updateCheck()
        .then((update) => {
          if (!active || !update) return;
          useSettingsStore.setState({ update });
          onUpdate(update);
        })
        // No network or no release yet: not worth bothering the user.
        .catch((error: unknown) => {
          console.warn("[update_check]", error);
        });
    }, UPDATE_CHECK_DELAY_MS);
    return () => {
      active = false;
      clearTimeout(timer);
      unsubscribe();
    };
  }, [onUpdate]);
}
