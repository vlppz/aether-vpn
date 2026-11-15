import { useEffect, useRef } from "react";
import { formatSpeed } from "../lib/format";
import { isWindowActive, onWindowActiveChange } from "../lib/windowActive";
import type { TrafficSample } from "../store/reducers";

interface Props {
  samples: TrafficSample[];
  active: boolean;
}

const WINDOW = 60_000;
/** render one tick behind real time so new points slide in instead of popping */
const DELAY = 1_100;
const MIN_SCALE = 64 * 1024;
/** Never faster than this, even on 120 Hz displays. */
const MAX_FPS = 30;
const IDLE_FRAME_MS = 250;

interface Pt {
  x: number;
  y: number;
}

/** Monotone cubic (Fritsch–Carlson) path — smooth without overshooting below zero. */
function tracePath(ctx: CanvasRenderingContext2D, pts: Pt[]) {
  const n = pts.length;
  if (n === 0) return;
  ctx.moveTo(pts[0].x, pts[0].y);
  if (n < 3) {
    for (let i = 1; i < n; i++) ctx.lineTo(pts[i].x, pts[i].y);
    return;
  }
  const dx: number[] = [];
  const m: number[] = [];
  for (let i = 0; i < n - 1; i++) {
    dx[i] = pts[i + 1].x - pts[i].x;
    m[i] = dx[i] === 0 ? 0 : (pts[i + 1].y - pts[i].y) / dx[i];
  }
  const tan: number[] = [m[0]];
  for (let i = 1; i < n - 1; i++) {
    if (m[i - 1] * m[i] <= 0) tan[i] = 0;
    else {
      const w1 = 2 * dx[i] + dx[i - 1];
      const w2 = dx[i] + 2 * dx[i - 1];
      tan[i] = (w1 + w2) / (w1 / m[i - 1] + w2 / m[i]);
    }
  }
  tan[n - 1] = m[n - 2];
  for (let i = 0; i < n - 1; i++) {
    const h = dx[i] / 3;
    ctx.bezierCurveTo(pts[i].x + h, pts[i].y + tan[i] * h, pts[i + 1].x - h, pts[i + 1].y - tan[i + 1] * h, pts[i + 1].x, pts[i + 1].y);
  }
}

function niceCeil(v: number): number {
  const exp = Math.pow(10, Math.floor(Math.log10(v)));
  const f = v / exp;
  return (f <= 1 ? 1 : f <= 2 ? 2 : f <= 5 ? 5 : 10) * exp;
}

function readColors() {
  const cs = getComputedStyle(document.documentElement);
  const v = (n: string) => cs.getPropertyValue(n).trim();
  return { ink: v("--ink"), accent: v("--accent"), line: v("--line"), ink3: v("--ink-3") };
}

/**
 * Live 60-second traffic chart. Canvas + rAF; scrolls continuously between
 * 1 Hz ticks. Grid and labels live on a cached offscreen layer that is only
 * rebuilt when its text, size or colours change, so a frame is one blit plus
 * two paths. Frames are only drawn when the plot has moved a whole device
 * pixel (≈20 fps at full width, 4 fps without traffic), and not at all while
 * the window is in the background — the loop stops and resumes on focus.
 */
export function TrafficChart({ samples, active }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const data = useRef({ samples, active });

  useEffect(() => {
    data.current = { samples, active };
  }, [samples, active]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    let raf = 0;
    let yMax = MIN_SCALE;
    let w = 0;
    let h = 0;
    let dpr = 1;
    let colors = readColors();
    let frames = 0;
    const grid = document.createElement("canvas");
    const gctx = grid.getContext("2d");
    let gridKey = "";

    const resize = () => {
      dpr = Math.min(2, window.devicePixelRatio || 1);
      w = canvas.clientWidth;
      h = canvas.clientHeight;
      canvas.width = Math.round(w * dpr);
      canvas.height = Math.round(h * dpr);
    };
    const ro = new ResizeObserver(resize);
    ro.observe(canvas);
    resize();

    const draw = () => {
      // theme/accent can change at any time; re-read tokens a couple of times a second
      if (++frames % 20 === 0) colors = readColors();
      const s = data.current.samples;
      const c = colors;
      const now = Date.now();
      const right = now - DELAY;
      const left = right - WINDOW;
      const padTop = 14;
      const padBottom = 16;
      const padRight = 64; // room for axis labels
      const plotW = w - padRight;
      const plotH = h - padTop - padBottom;

      let peak = 0;
      for (const p of s) if (p.t >= left - 2000) peak = Math.max(peak, p.down, p.up);
      const target = niceCeil(Math.max(MIN_SCALE, peak * 1.15));
      yMax += (target - yMax) * 0.08;

      const labels = [1, 2, 3].map((i) => formatSpeed((yMax * i) / 3));
      const key = `${w}x${h}@${dpr}|${labels.join("|")}|${c.ink3}|${c.line}`;
      if (key !== gridKey && gctx) {
        gridKey = key;
        grid.width = canvas.width;
        grid.height = canvas.height;
        gctx.setTransform(dpr, 0, 0, dpr, 0, 0);
        gctx.clearRect(0, 0, w, h);
        gctx.font = "400 10px 'Geist Mono Variable', ui-monospace, monospace";
        gctx.fillStyle = c.ink3;
        gctx.strokeStyle = c.line;
        gctx.lineWidth = 1;
        gctx.textBaseline = "middle";
        gctx.textAlign = "left";
        for (let i = 0; i <= 3; i++) {
          const y = Math.round(padTop + plotH - (plotH * i) / 3) + 0.5;
          gctx.beginPath();
          gctx.moveTo(0, y);
          gctx.lineTo(plotW, y);
          gctx.stroke();
          if (i > 0) gctx.fillText(labels[i - 1], plotW + 10, y);
        }
        gctx.textBaseline = "alphabetic";
        gctx.fillText("−60 s", 0, h - 2);
        gctx.textAlign = "right";
        gctx.fillText("now", plotW, h - 2);
      }

      ctx.setTransform(1, 0, 0, 1, 0, 0);
      ctx.clearRect(0, 0, canvas.width, canvas.height);
      ctx.drawImage(grid, 0, 0);
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

      const toX = (t: number) => ((t - left) / WINDOW) * plotW;
      const toY = (v: number) => padTop + plotH - (Math.min(v, yMax * 1.05) / yMax) * plotH;

      const series = (key: "up" | "down", color: string) => {
        const vis = s.filter((p, i) => p.t >= left - 1500 || (s[i + 1] && s[i + 1].t >= left));
        if (vis.length < 2) return;
        const pts = vis.map((p) => ({ x: toX(p.t), y: toY(p[key]) }));
        ctx.save();
        ctx.beginPath();
        ctx.rect(0, 0, plotW, h);
        ctx.clip();
        ctx.beginPath();
        tracePath(ctx, pts);
        ctx.lineTo(pts[pts.length - 1].x, padTop + plotH);
        ctx.lineTo(pts[0].x, padTop + plotH);
        ctx.closePath();
        ctx.globalAlpha = 0.06;
        ctx.fillStyle = color;
        ctx.fill();
        ctx.globalAlpha = 1;
        ctx.beginPath();
        tracePath(ctx, pts);
        ctx.strokeStyle = color;
        ctx.lineWidth = 1.25;
        ctx.lineJoin = "round";
        ctx.stroke();
        ctx.restore();
      };
      series("up", c.accent);
      series("down", c.ink);
    };

    let last = 0;
    const loop = (t: number) => {
      if (!isWindowActive()) {
        raf = 0; // stop entirely; resumed by the focus listener below
        return;
      }
      raf = requestAnimationFrame(loop);
      // time for the plot to scroll one device pixel
      const pixelMs = WINDOW / Math.max(1, (w - 64) * dpr);
      const budget = data.current.active ? Math.max(1000 / MAX_FPS, pixelMs) : IDLE_FRAME_MS;
      // small tolerance so a 60 Hz display lands on every second frame
      if (t - last < budget - 4) return;
      last = t;
      draw();
    };
    const start = () => {
      if (!raf) raf = requestAnimationFrame(loop);
    };
    start();
    const off = onWindowActiveChange((on) => on && start());
    return () => {
      off();
      cancelAnimationFrame(raf);
      ro.disconnect();
    };
  }, []);

  return <canvas ref={canvasRef} className="block size-full" aria-hidden />;
}
