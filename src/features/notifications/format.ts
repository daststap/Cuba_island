import { ru } from "../../i18n/ru";

/** «сейчас», «5 мин назад», «2 ч назад». */
export function formatAgo(time: number, now: number): string {
  const minutes = Math.floor((now - time) / 60_000);
  if (minutes < 1) return ru.notifications.now;
  if (minutes < 60) return ru.notifications.minutesAgo(minutes);
  return ru.notifications.hoursAgo(Math.floor(minutes / 60));
}
