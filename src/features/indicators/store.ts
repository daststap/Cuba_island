import { useEffect } from "react";
import { create } from "zustand";
import { useIslandStore } from "../../app/store";
import { indicatorsGet, reportError, type Sensors } from "../../ipc/commands";
import { onCapsLock, onSensors } from "../../ipc/events";
import { useSettingsStore } from "../settings/store";

interface IndicatorsStore {
  sensors: Sensors;
  capsLock: boolean;
}

export const NO_SENSORS: Sensors = { microphone: [], camera: [] };

export const useIndicatorsStore = create<IndicatorsStore>()(() => ({
  sensors: NO_SENSORS,
  capsLock: false,
}));

/** Keeps microphone/camera use and Caps Lock in sync. Mount once. */
export function useIndicatorsSync(): void {
  useEffect(() => {
    indicatorsGet()
      .then(({ sensors, capsLock }) => {
        useIndicatorsStore.setState({ sensors, capsLock });
      })
      .catch((error: unknown) => {
        reportError("indicators_get", error);
      });
    const unsubscribe = [
      onSensors((sensors) => {
        useIndicatorsStore.setState({ sensors });
      }),
      onCapsLock((capsLock) => {
        useIndicatorsStore.setState({ capsLock });
        const island = useIslandStore.getState();
        const { state } = island;
        // Only over the pill: an open panel or another peek is not interrupted.
        const free = state.mode === "compact" || (state.mode === "peek" && state.kind === "caps");
        if (free && useSettingsStore.getState().settings?.capsPeek !== false) {
          island.peek("caps");
        }
      }),
    ];
    return () => {
      unsubscribe.forEach((fn) => {
        fn();
      });
    };
  }, []);
}
