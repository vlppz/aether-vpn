import { useEffect, useState } from "react";

/** Current time (unix ms), re-rendering every `interval` ms while `active`. */
export function useNow(interval = 1000, active = true): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!active) return;
    const tick = () => setNow(Date.now());
    const id = setInterval(tick, interval);
    const raf = requestAnimationFrame(tick);
    return () => {
      clearInterval(id);
      cancelAnimationFrame(raf);
    };
  }, [interval, active]);
  return now;
}
