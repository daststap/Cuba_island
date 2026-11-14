import { useEffect, useState } from "react";
import { getBattery, reportError, type Battery } from "../../ipc/commands";
import { onBattery } from "../../ipc/events";
import { ru } from "../../i18n/ru";

function useBattery(): Battery | null {
  const [battery, setBattery] = useState<Battery | null>(null);
  useEffect(() => {
    let active = true;
    getBattery()
      .then((value) => {
        if (active) setBattery(value);
      })
      .catch((error: unknown) => {
        reportError("get_battery", error);
      });
    const unsubscribe = onBattery(setBattery);
    return () => {
      active = false;
      unsubscribe();
    };
  }, []);
  return battery;
}

const LOW_PERCENT = 20;

/** Battery icon with percentage; renders nothing on machines without a battery. */
export function BatteryIndicator({ showPercent = true }: { showPercent?: boolean }) {
  const battery = useBattery();
  if (!battery) return null;

  const { percent, charging, pluggedIn } = battery;
  const fill =
    charging || pluggedIn ? "#30d158" : percent <= LOW_PERCENT ? "#ff453a" : "currentColor";
  const status = charging ? ru.battery.charging : pluggedIn ? ru.battery.pluggedIn : null;
  const label = status ? `${ru.battery.label(percent)}, ${status}` : ru.battery.label(percent);

  return (
    <span className="flex items-center gap-1 tabular-nums" role="img" aria-label={label}>
      {showPercent && <span aria-hidden>{percent}%</span>}
      <svg width="22" height="11" viewBox="0 0 22 11" aria-hidden>
        <rect
          x="0.5"
          y="0.5"
          width="18"
          height="10"
          rx="2.5"
          fill="none"
          stroke="currentColor"
          strokeOpacity="0.45"
        />
        <rect
          x="19.5"
          y="3.5"
          width="1.5"
          height="4"
          rx="0.75"
          fill="currentColor"
          opacity="0.45"
        />
        <rect
          x="2"
          y="2"
          width={Math.max(1, (15 * percent) / 100)}
          height="7"
          rx="1.5"
          fill={fill}
        />
        {(charging || pluggedIn) && (
          <path
            d="M10.5 1.5 7 6h2.5L8.5 9.5 12 5H9.5z"
            fill="#fff"
            stroke="#000"
            strokeWidth="0.5"
          />
        )}
      </svg>
    </span>
  );
}
