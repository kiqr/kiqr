import { motion } from "motion/react";
import { Check } from "lucide-react";
import type { Phase } from "./store";

const STEPS: { id: Phase; label: string }[] = [
  { id: "discovering", label: "Discover" },
  { id: "crawling", label: "Crawl" },
  { id: "learning", label: "Learn" },
  { id: "done", label: "Profile" },
];
const ORDER: Phase[] = ["connecting", "discovering", "crawling", "learning", "done"];

export function PhaseTimeline({ phase }: { phase: Phase }) {
  const idx = ORDER.indexOf(phase);
  return (
    <ol className="cr-phases">
      {STEPS.map((s, i) => {
        const stepIdx = ORDER.indexOf(s.id);
        const done = idx > stepIdx || (phase === "done" && s.id === "done");
        const current = idx === stepIdx && phase !== "done";
        return (
          <li key={s.id}>
            {i > 0 && (
              <span className="cr-phase-line">
                <motion.span initial={false} animate={{ width: idx >= stepIdx ? "100%" : "0%" }} transition={{ duration: 0.6 }} />
              </span>
            )}
            <span className={`cr-phase ${done ? "done" : current ? "cur" : ""}`}>
              {done ? <Check size={12} /> : current ? <span className="cr-dot" /> : null}
              {s.label}
            </span>
          </li>
        );
      })}
    </ol>
  );
}
