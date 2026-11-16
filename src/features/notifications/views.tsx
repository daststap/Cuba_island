import { convertFileSrc } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { ru } from "../../i18n/ru";
import type { NotificationItem } from "../../ipc/commands";
import { useSettingsStore } from "../settings/store";
import { formatAgo } from "./format";
import { dismissNotification, openNotification, useNotificationsStore } from "./store";

function useMinuteClock(): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const id = setInterval(() => {
      setNow(Date.now());
    }, 30_000);
    return () => {
      clearInterval(id);
    };
  }, []);
  return now;
}

function AppIcon({ item, size }: { item: NotificationItem; size: number }) {
  if (item.icon) {
    return (
      <img
        src={convertFileSrc(item.icon)}
        alt=""
        width={size}
        height={size}
        className="shrink-0 rounded-md"
      />
    );
  }
  return (
    <span
      aria-hidden
      className="flex shrink-0 items-center justify-center rounded-md bg-white/15 text-[11px] font-semibold"
      style={{ width: size, height: size }}
    >
      {item.app.slice(0, 1).toUpperCase()}
    </span>
  );
}

/** The peek for a new notification; a click opens the app. */
export function NotificationPeek() {
  const item = useNotificationsStore((s) => s.latest);
  if (!item) return null;
  return (
    <button
      type="button"
      onClick={() => {
        openNotification(item.id);
      }}
      aria-label={ru.notifications.open(item.app)}
      className="flex h-full w-full items-center gap-3 px-4 text-left"
    >
      <AppIcon item={item} size={28} />
      <span className="min-w-0 flex-1">
        <span className="block truncate text-[11px] text-[var(--island-muted)]">{item.app}</span>
        <span className="block truncate text-sm font-semibold">
          {item.title}
          {item.body && <span className="font-normal text-white/70"> — {item.body}</span>}
        </span>
      </span>
    </button>
  );
}

/** Recent notifications on the home tab. */
export function NotificationsList() {
  const items = useNotificationsStore((s) => s.items);
  const enabled = useSettingsStore((s) => s.settings?.notifications !== false);
  const now = useMinuteClock();
  if (!enabled || items.length === 0) return null;
  return (
    <ul aria-label={ru.notifications.list} className="flex w-full flex-col gap-1">
      {items.map((item) => (
        <li key={item.id} className="group flex items-center gap-2">
          <button
            type="button"
            onClick={() => {
              openNotification(item.id);
            }}
            aria-label={ru.notifications.open(item.app)}
            className="flex min-w-0 flex-1 items-center gap-2 rounded-lg px-1.5 py-1 text-left text-xs hover:bg-white/8"
          >
            <AppIcon item={item} size={20} />
            <span className="min-w-0 flex-1 truncate">
              <span className="font-medium">{item.title || item.app}</span>
              {item.body && <span className="text-[var(--island-muted)]"> {item.body}</span>}
            </span>
            <span className="shrink-0 text-[11px] text-[var(--island-muted)]">
              {formatAgo(item.time, now)}
            </span>
          </button>
          <button
            type="button"
            aria-label={ru.notifications.dismiss(item.app)}
            onClick={() => {
              dismissNotification(item.id);
            }}
            className="hidden h-5 w-5 shrink-0 items-center justify-center rounded-full text-xs text-white/50 group-hover:flex hover:bg-white/15 hover:text-white"
          >
            ×
          </button>
        </li>
      ))}
    </ul>
  );
}
