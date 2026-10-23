import { useEffect, useRef, useState } from "react";

export const fmt = new Intl.NumberFormat("en-US");

/** Number that tweens towards its target value. */
export function Ticker({ value, format = (v: number) => fmt.format(Math.round(v)), className }: { value: number; format?: (v: number) => string; className?: string }) {
  const [shown, setShown] = useState(value);
  const shownRef = useRef(value);
  shownRef.current = shown;

  useEffect(() => {
    const from = shownRef.current;
    if (from === value) return;
    if (typeof window !== "undefined" && window.matchMedia?.("(prefers-reduced-motion: reduce)").matches) {
      setShown(value);
      return;
    }
    const start = performance.now();
    const dur = 450;
    let raf = 0;
    const step = (now: number) => {
      const t = Math.min((now - start) / dur, 1);
      const e = 1 - Math.pow(1 - t, 3);
      setShown(from + (value - from) * e);
      if (t < 1) raf = requestAnimationFrame(step);
    };
    raf = requestAnimationFrame(step);
    return () => cancelAnimationFrame(raf);
  }, [value]);

  return <span className={`tnum ${className ?? ""}`}>{format(shown)}</span>;
}

export function ProgressRing({ value, max, size = 132, phase }: { value: number; max: number; size?: number; phase: string }) {
  const stroke = 8;
  const r = (size - stroke) / 2;
  const c = 2 * Math.PI * r;
  const pct = max ? Math.min(value / max, 1) : 0;
  const spinning = phase === "connecting" || phase === "discovering";
  return (
    <div className="cr-ring" style={{ width: size, height: size }}>
      <svg width={size} height={size} className={spinning ? "cr-spin" : ""}>
        <defs>
          <linearGradient id="cr-ring-grad" x1="0" y1="0" x2="1" y2="1">
            <stop offset="0%" stopColor="var(--accent, #5eead4)" />
            <stop offset="100%" stopColor="var(--accent-2, #818cf8)" />
          </linearGradient>
        </defs>
        <circle cx={size / 2} cy={size / 2} r={r} stroke="var(--line, #1c2740)" strokeWidth={stroke} fill="none" />
        <circle
          cx={size / 2}
          cy={size / 2}
          r={r}
          stroke="url(#cr-ring-grad)"
          strokeWidth={stroke}
          fill="none"
          strokeLinecap="round"
          strokeDasharray={c}
          strokeDashoffset={spinning ? c * 0.78 : c * (1 - pct)}
          transform={`rotate(-90 ${size / 2} ${size / 2})`}
          style={{ transition: "stroke-dashoffset 500ms cubic-bezier(.2,.8,.2,1)", filter: "drop-shadow(0 0 6px rgba(94,234,212,.55))" }}
        />
      </svg>
      <div className="cr-ring-c">
        <Ticker value={value} className="cr-ring-n" />
        <span>of {fmt.format(max)} pages</span>
      </div>
    </div>
  );
}
