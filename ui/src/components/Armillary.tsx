import { useReducedMotion } from "motion/react";
import type { ConnectionState } from "../lib/types";
import { cn } from "../lib/cn";
import { useT } from "../i18n";
import { useWindowActive } from "../hooks/useWindowActive";

interface Props {
  state: ConnectionState;
  onToggle: () => void;
  disabled?: boolean;
  className?: string;
}

const C = 120; // centre of the 240×240 viewBox
const DIAL = 112;
// orbit A is the one the satellite travels on; drawn in accent when connected
const ORBIT_A = { rx: 94, ry: 30, tilt: -22 };
const ORBIT_B = { rx: 94, ry: 58, tilt: 38 };
const ORBIT_A_PATH = `M ${C - ORBIT_A.rx} ${C} a ${ORBIT_A.rx} ${ORBIT_A.ry} 0 1 0 ${ORBIT_A.rx * 2} 0 a ${ORBIT_A.rx} ${ORBIT_A.ry} 0 1 0 ${-ORBIT_A.rx * 2} 0`;

const TICKS = Array.from({ length: 72 }, (_, i) => {
  const a = (i * 5 * Math.PI) / 180;
  const major = i % 18 === 0;
  const mid = i % 6 === 0;
  const inner = DIAL - (major ? 9 : mid ? 5.5 : 3);
  return {
    x1: C + Math.cos(a) * DIAL,
    y1: C + Math.sin(a) * DIAL,
    x2: C + Math.cos(a) * inner,
    y2: C + Math.sin(a) * inner,
    major: major || mid,
  };
});

/**
 * The armillary mark on Home: hairline dial, two tilted orbits and a central body.
 * Idle: hollow body. Connecting: a satellite travels orbit A. Connected: body and
 * orbit A in accent, the rings turn very slowly. Error: body in danger, static.
 */
export function Armillary({ state, onToggle, disabled, className }: Props) {
  const t = useT();
  const reduce = useReducedMotion() ?? false;
  const windowActive = useWindowActive();
  const connected = state === "connected";
  const busy = state === "connecting" || state === "disconnecting";
  const error = state === "error";
  const label = connected ? t("orb.disconnect") : busy ? t("orb.cancel") : t("orb.connect");

  return (
    <button
      type="button"
      onClick={onToggle}
      disabled={disabled}
      aria-label={label}
      aria-pressed={connected}
      className={cn("group relative rounded-full outline-offset-4 transition-opacity disabled:opacity-60", className)}
    >
      {/* Three stacked layers so only the orbits move. Their slow turn runs on
          an HTML layer the system compositor rotates at display rate without
          repainting the SVG; it is paused while the window is in the background. */}
      <svg viewBox="0 0 240 240" className="absolute inset-0 block size-full" aria-hidden fill="none">
        {/* dial with degree ticks */}
        <g className={cn("transition-colors duration-300", busy || connected ? "text-ink-3" : "text-ink-3/70 group-hover:text-ink-3")} stroke="currentColor">
          <circle cx={C} cy={C} r={DIAL} strokeWidth="0.75" />
          {TICKS.map((k, i) => (
            <line key={i} x1={k.x1} y1={k.y1} x2={k.x2} y2={k.y2} strokeWidth={k.major ? 0.9 : 0.6} opacity={k.major ? 1 : 0.6} />
          ))}
        </g>
      </svg>
      <div
        className="absolute inset-0"
        style={{
          animation: connected && !reduce ? "orbit-turn 96s linear infinite" : undefined,
          // frozen in place (not reset) while the window is in the background
          animationPlayState: windowActive ? "running" : "paused",
          willChange: connected && !reduce && windowActive ? "transform" : undefined,
        }}
      >
        <svg viewBox="0 0 240 240" className="block size-full" aria-hidden fill="none">
          {/* orbits — turn once a minute and a half while connected */}
          <g className={cn("transition-colors duration-300", busy ? "text-ink-2" : connected ? "text-ink-3" : "text-ink-3/80 group-hover:text-ink-2")} stroke="currentColor">
            <circle cx={C} cy={C} r={60} strokeWidth="0.75" strokeDasharray="1.5 3.5" />
            <ellipse cx={C} cy={C} rx={ORBIT_B.rx} ry={ORBIT_B.ry} transform={`rotate(${ORBIT_B.tilt} ${C} ${C})`} strokeWidth="0.9" />
          </g>
          <g transform={`rotate(${ORBIT_A.tilt} ${C} ${C})`}>
            <ellipse
              cx={C}
              cy={C}
              rx={ORBIT_A.rx}
              ry={ORBIT_A.ry}
              strokeWidth={connected ? 1.25 : 0.9}
              className={cn("transition-colors duration-300", connected ? "stroke-accent" : busy ? "stroke-ink-2" : "stroke-ink-3/80 group-hover:stroke-ink-2")}
            />
            {busy && (
              <circle r="3.4" className="fill-ink" cx={reduce || !windowActive ? C + ORBIT_A.rx : undefined} cy={reduce || !windowActive ? C : undefined}>
                {!reduce && windowActive && <animateMotion dur="2.4s" repeatCount="indefinite" path={ORBIT_A_PATH} />}
              </circle>
            )}
            {connected && <circle cx={C + ORBIT_A.rx * Math.cos(-0.9)} cy={C + ORBIT_A.ry * Math.sin(-0.9)} r="3.4" className="fill-accent" />}
          </g>
        </svg>
      </div>
      <svg viewBox="0 0 240 240" className="relative block size-full" aria-hidden fill="none">
        {/* central body; re-keyed per state for a single soft fade */}
        <g key={state} style={{ transformOrigin: `${C}px ${C}px`, animation: reduce ? undefined : "body-in 300ms ease-out" }}>
          <circle
            cx={C}
            cy={C}
            r="12"
            strokeWidth="1.25"
            className={cn(
              connected && "fill-accent stroke-accent",
              error && "fill-danger stroke-danger",
              busy && "fill-bg stroke-ink",
              state === "disconnected" && "fill-bg stroke-ink-2 group-hover:stroke-ink",
            )}
          />
        </g>
      </svg>
    </button>
  );
}
