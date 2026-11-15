import { useIslandStore } from "../../app/store";
import { ru } from "../../i18n/ru";
import { reportError, screenshotStart } from "../../ipc/commands";

/** Folds the island and starts a region screenshot (Rust waits for the fold). */
function takeScreenshot(): void {
  const island = useIslandStore.getState();
  if (island.inputMode) island.leaveInput();
  island.dispatch({ type: "collapse" });
  screenshotStart().catch((error: unknown) => {
    reportError("screenshot_start", error);
  });
}

/** Small camera button in the corner of the open panel. */
export function ScreenshotButton() {
  return (
    <button
      type="button"
      title={ru.screenshot.take}
      aria-label={ru.screenshot.take}
      onClick={takeScreenshot}
      className="flex h-7 w-7 items-center justify-center rounded-full text-[var(--island-muted)] transition-colors hover:bg-white/10 hover:text-[var(--island-fg)]"
    >
      <svg
        viewBox="0 0 24 24"
        width="16"
        height="16"
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
        aria-hidden
      >
        <path d="M4 8.5A1.5 1.5 0 015.5 7h2l1.5-2h6l1.5 2h2A1.5 1.5 0 0120 8.5v9a1.5 1.5 0 01-1.5 1.5h-13A1.5 1.5 0 014 17.5z" />
        <circle cx="12" cy="13" r="3.2" />
      </svg>
    </button>
  );
}
