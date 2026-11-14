import { useEffect, useState } from "react";
import { formatLongDate, formatTime, msUntilNextMinute } from "./format";

/** Current time, re-rendering only this subtree once a minute, on the minute. */
function useMinute(): Date {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const timer = setTimeout(() => {
      setNow(new Date());
    }, msUntilNextMinute(now));
    return () => {
      clearTimeout(timer);
    };
  }, [now]);
  return now;
}

export function Clock({ className }: { className?: string }) {
  const now = useMinute();
  return (
    <time className={`tabular-nums ${className ?? ""}`} dateTime={now.toISOString()}>
      {formatTime(now)}
    </time>
  );
}

export function LongDate({ className }: { className?: string }) {
  const now = useMinute();
  return <span className={className}>{formatLongDate(now)}</span>;
}
