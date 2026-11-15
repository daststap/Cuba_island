import { ru } from "../../i18n/ru";
import { useSettingsStore } from "../settings/store";
import { useIndicatorsStore } from "./store";

const MIC = "#ff9f0a";
const CAMERA = "#30d158";

function useVisibleSensors() {
  const sensors = useIndicatorsStore((s) => s.sensors);
  const enabled = useSettingsStore((s) => s.settings?.privacyDots !== false);
  return enabled ? sensors : null;
}

/** iPhone-style dots at the right edge of the pill: orange = microphone, green = camera. */
export function PrivacyDots() {
  const sensors = useVisibleSensors();
  if (!sensors) return null;
  const mic = sensors.microphone.length > 0;
  const camera = sensors.camera.length > 0;
  if (!mic && !camera) return null;
  const label = [
    mic && ru.indicators.usedBy(ru.indicators.microphone, sensors.microphone),
    camera && ru.indicators.usedBy(ru.indicators.camera, sensors.camera),
  ]
    .filter(Boolean)
    .join("; ");
  return (
    <div
      role="img"
      aria-label={label}
      title={label}
      className="pointer-events-none absolute top-1/2 right-[7px] flex -translate-y-1/2 flex-col gap-[3px]"
    >
      {camera && <span className="block h-1.5 w-1.5 rounded-full" style={{ background: CAMERA }} />}
      {mic && <span className="block h-1.5 w-1.5 rounded-full" style={{ background: MIC }} />}
    </div>
  );
}

/** «Микрофон: Discord · Камера: Discord» on the home tab while something listens. */
export function SensorsLine() {
  const sensors = useVisibleSensors();
  if (!sensors) return null;
  const rows = [
    { color: MIC, device: ru.indicators.microphone, apps: sensors.microphone },
    { color: CAMERA, device: ru.indicators.camera, apps: sensors.camera },
  ].filter((r) => r.apps.length > 0);
  if (rows.length === 0) return null;
  return (
    <ul
      aria-label={ru.indicators.inUse}
      className="flex flex-wrap justify-center gap-x-4 gap-y-1 text-xs text-[var(--island-muted)]"
    >
      {rows.map((r) => (
        <li key={r.device} className="flex items-center gap-1.5">
          <span className="h-1.5 w-1.5 rounded-full" style={{ background: r.color }} aria-hidden />
          {ru.indicators.usedBy(r.device, r.apps)}
        </li>
      ))}
    </ul>
  );
}

/** Short peek when Caps Lock is toggled. */
export function CapsPeek() {
  const on = useIndicatorsStore((s) => s.capsLock);
  return (
    <div role="status" className="flex h-full items-center justify-between px-4">
      <span className="flex items-center gap-2 text-sm font-semibold">
        <svg
          width="18"
          height="18"
          viewBox="0 0 24 24"
          aria-hidden
          className={on ? "text-[#30d158]" : "text-white/40"}
        >
          <path
            fill="currentColor"
            d="M12 3.5 3.5 12.5H8V17h8v-4.5h4.5zM8 19h8v2H8z"
            fillOpacity={on ? 1 : 0.9}
          />
        </svg>
        {ru.indicators.capsLock}
      </span>
      <span className={`text-sm font-semibold ${on ? "text-[#30d158]" : "text-white/50"}`}>
        {on ? ru.indicators.on : ru.indicators.off}
      </span>
    </div>
  );
}
