import { AnimatePresence, motion, type Transition } from "motion/react";
import { useRef } from "react";
import { BatteryIndicator } from "../features/battery/BatteryIndicator";
import { Clock, LongDate } from "../features/clock/Clock";
import { ru } from "../i18n/ru";
import { TABS, type IslandState, type Tab } from "./islandMachine";
import { useIslandStore } from "./store";
import { useIslandEvents } from "./useIslandEvents";
import { useIslandRect } from "./useIslandRect";

/** Island shapes. The island sits flush against the top edge: only bottom corners round. */
const SHAPES = {
  compact: { width: 200, height: 36, radius: 18 },
  expanded: { width: 640, height: 360, radius: 32 },
} as const;

const SPRING: Transition = { type: "spring", stiffness: 420, damping: 34, mass: 0.9 };
const FADE: Transition = { duration: 0.14 };

export function Island() {
  useIslandEvents();
  const state = useIslandStore((s) => s.state);
  const dispatch = useIslandStore((s) => s.dispatch);
  const ref = useRef<HTMLDivElement>(null);
  useIslandRect(ref);

  const shape = state.mode === "expanded" ? SHAPES.expanded : SHAPES.compact;

  return (
    <div className="flex h-full justify-center">
      <motion.div
        ref={ref}
        data-mode={state.mode}
        className="relative overflow-hidden bg-[var(--island-bg)] text-[var(--island-fg)] shadow-[var(--island-shadow)]"
        initial={false}
        animate={{
          width: shape.width,
          height: shape.height,
          borderBottomLeftRadius: shape.radius,
          borderBottomRightRadius: shape.radius,
        }}
        transition={SPRING}
      >
        <AnimatePresence mode="popLayout" initial={false}>
          {state.mode === "expanded" ? (
            <motion.div
              key="expanded"
              className="absolute inset-0"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1, transition: { ...FADE, delay: 0.06 } }}
              exit={{ opacity: 0, transition: FADE }}
            >
              <ExpandedView
                tab={state.tab}
                onSelect={(tab) => {
                  dispatch({ type: "selectTab", tab });
                }}
              />
            </motion.div>
          ) : (
            <motion.div
              key="compact"
              className="absolute inset-0"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1, transition: { ...FADE, delay: 0.08 } }}
              exit={{ opacity: 0, transition: FADE }}
            >
              <CompactView mode={state.mode} />
            </motion.div>
          )}
        </AnimatePresence>
      </motion.div>
    </div>
  );
}

function CompactView({ mode }: { mode: IslandState["mode"] }) {
  return (
    <div
      role="status"
      aria-label={ru.appName}
      data-mode={mode}
      className="flex h-full items-center justify-between px-5 text-[13px] font-semibold"
    >
      <Clock />
      <BatteryIndicator showPercent={false} />
    </div>
  );
}

function ExpandedView({ tab, onSelect }: { tab: Tab; onSelect: (tab: Tab) => void }) {
  return (
    <div className="flex h-full flex-col px-6 pt-3 pb-6">
      <div role="tablist" aria-label={ru.tabsLabel} className="flex justify-center gap-1">
        {TABS.map((t) => (
          <button
            key={t}
            role="tab"
            type="button"
            aria-selected={t === tab}
            onClick={() => {
              onSelect(t);
            }}
            className={`rounded-full px-3 py-1 text-xs font-medium transition-colors ${
              t === tab
                ? "bg-white/15 text-[var(--island-fg)]"
                : "text-[var(--island-muted)] hover:bg-white/8 hover:text-[var(--island-fg)]"
            }`}
          >
            {ru.tabs[t]}
          </button>
        ))}
      </div>

      <div role="tabpanel" className="flex flex-1 flex-col items-center justify-center">
        {tab === "home" ? (
          <>
            <Clock className="text-7xl font-light tracking-tight" />
            <LongDate className="mt-1 text-base text-[var(--island-muted)] first-letter:uppercase" />
            <div className="mt-5 text-sm text-[var(--island-muted)]">
              <BatteryIndicator />
            </div>
          </>
        ) : (
          <span className="text-sm text-[var(--island-muted)]">{ru.comingSoon}</span>
        )}
      </div>
    </div>
  );
}
