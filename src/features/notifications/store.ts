import { useEffect } from "react";
import { create } from "zustand";
import { useIslandStore } from "../../app/store";
import {
  notificationsDismiss,
  notificationsGet,
  notificationsOpen,
  reportError,
  type NotificationItem,
  type NotificationsState,
} from "../../ipc/commands";
import { onNotification, onNotificationsList } from "../../ipc/events";

interface NotificationsStore extends NotificationsState {
  /** The notification the peek shows. */
  latest: NotificationItem | null;
}

const NO_ITEMS: NotificationItem[] = [];
const NO_APPS: string[] = [];

export const useNotificationsStore = create<NotificationsStore>()(() => ({
  access: "allowed",
  items: NO_ITEMS,
  apps: NO_APPS,
  latest: null,
}));

function fail(context: string) {
  return (error: unknown) => {
    reportError(context, error);
  };
}

/** Opens the sending app (and clears it from the Notification Center). */
export function openNotification(id: number): void {
  const island = useIslandStore.getState();
  if (island.inputMode) useIslandStore.setState({ inputMode: false });
  island.dispatch({ type: "collapse" });
  notificationsOpen(id).catch(fail("notifications_open"));
}

export function dismissNotification(id: number): void {
  notificationsDismiss(id).catch(fail("notifications_dismiss"));
}

/** Keeps the list in sync and peeks on new notifications. Mount once. */
export function useNotificationsSync(): void {
  useEffect(() => {
    notificationsGet()
      .then((state) => {
        // Merge only a real answer: zustand would replace the store with a null.
        if (state) useNotificationsStore.setState(state);
      })
      .catch(fail("notifications_get"));
    const unsubscribe = [
      onNotificationsList((state) => {
        if (state) useNotificationsStore.setState(state);
      }),
      onNotification((item) => {
        useNotificationsStore.setState({ latest: item });
        // Over the pill only: an open panel lists it on the home tab anyway.
        useIslandStore.getState().peek("notification");
      }),
    ];
    return () => {
      unsubscribe.forEach((fn) => {
        fn();
      });
    };
  }, []);
}
