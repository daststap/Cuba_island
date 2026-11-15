import { useEffect, useState } from "react";

/** Current time, ticking every `ms` only while `active`. */
export function useNow(active: boolean, ms = 250): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!active) return;
    const id = setInterval(() => {
      setNow(Date.now());
    }, ms);
    return () => {
      clearInterval(id);
    };
  }, [active, ms]);
  return now;
}
