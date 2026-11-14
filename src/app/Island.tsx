import { ru } from "../i18n/ru";

/** Root island component. Stage 0: a static placeholder pill. */
export function Island() {
  return (
    <div className="flex h-full items-start justify-center pt-2">
      <div
        role="status"
        className="flex h-9 items-center gap-2 rounded-full bg-[var(--island-bg)] px-4 text-sm text-[var(--island-fg)] shadow-[var(--island-shadow)]"
      >
        <span className="font-semibold">{ru.appName}</span>
        <span className="text-[var(--island-muted)]">{ru.placeholder}</span>
      </div>
    </div>
  );
}
