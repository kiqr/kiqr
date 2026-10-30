// The one number that matters while Seemo builds: how far the whole build is,
// what it is doing in plain words, and how long is left.
import { motion, useReducedMotion } from "motion/react";
import { Check, TriangleAlert } from "lucide-react";
import type { Build, BuildStage } from "../api";
import { Ticker } from "../crawl/tickers";

const STEPS: { id: BuildStage; label: string }[] = [
  { id: "crawling", label: "Find and read pages" },
  { id: "reading", label: "Understand them" },
  { id: "writing", label: "Write your profile" },
  { id: "ready", label: "Ready" },
];

export const STAGE_WORDS: Record<BuildStage, string> = {
  new: "Getting ready",
  crawling: "Reading your pages",
  reading: "Understanding what you do",
  writing: "Writing your business profile",
  ready: "Your profile is ready",
  failed: "I had to stop",
};

/** "about 2 minutes left", rounded the way a person would say it. */
export function etaWords(secs: number | null | undefined) {
  if (secs == null) return "working out how long this takes";
  if (secs < 45) return "less than a minute left";
  if (secs < 90) return "about a minute left";
  return `about ${Math.round(secs / 60)} minutes left`;
}

export function BuildRing({ build }: { build: Build }) {
  const reduced = useReducedMotion();
  const size = 220;
  const stroke = 12;
  const r = (size - stroke) / 2;
  const c = 2 * Math.PI * r;
  const pct = Math.max(0, Math.min(100, build.pct));
  const done = build.stage === "ready";
  return (
    <div className={`wl-ring ${done ? "done" : ""} ${build.stage === "failed" ? "failed" : ""}`}>
      <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`} aria-hidden>
        <defs>
          <linearGradient id="wl-ring-g" x1="0" y1="0" x2="1" y2="1">
            <stop offset="0%" stopColor="var(--accent)" />
            <stop offset="100%" stopColor="var(--accent-2)" />
          </linearGradient>
        </defs>
        <circle cx={size / 2} cy={size / 2} r={r} fill="none" stroke="var(--line)" strokeWidth={stroke} />
        <motion.circle
          cx={size / 2}
          cy={size / 2}
          r={r}
          fill="none"
          stroke="url(#wl-ring-g)"
          strokeWidth={stroke}
          strokeLinecap="round"
          strokeDasharray={c}
          initial={false}
          animate={{ strokeDashoffset: c * (1 - pct / 100) }}
          transition={reduced ? { duration: 0 } : { type: "spring", stiffness: 40, damping: 18 }}
          transform={`rotate(-90 ${size / 2} ${size / 2})`}
        />
      </svg>
      <div className="wl-ring-c" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={pct} aria-label="How far Seemo is">
        {done ? (
          <Check size={56} strokeWidth={2.4} />
        ) : build.stage === "failed" ? (
          <TriangleAlert size={48} strokeWidth={2} />
        ) : (
          <b>
            <Ticker value={pct} />
            <small>%</small>
          </b>
        )}
      </div>
    </div>
  );
}

export function StageSteps({ stage }: { stage: BuildStage }) {
  const at = stage === "new" ? -1 : stage === "failed" ? -1 : STEPS.findIndex((s) => s.id === stage);
  return (
    <ol className="wl-steps">
      {STEPS.map((s, i) => {
        const state = i < at || stage === "ready" ? "done" : i === at ? "now" : "next";
        return (
          <li key={s.id} className={state}>
            <span className="dot">{state === "done" ? <Check size={11} strokeWidth={3} /> : null}</span>
            {s.label}
          </li>
        );
      })}
    </ol>
  );
}
