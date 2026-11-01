// What Seemo is learning, one thought at a time: each sentence it takes from
// a page comes in word by word, stays long enough to read and gives way to the
// next. Names and single words are left out; they read as a list, not a thought.
// When it runs ahead, older thoughts are skipped; when it is quiet, earlier
// ones come back now and then, so the line never stands still for long.
import { useEffect, useRef, useState } from "react";
import { AnimatePresence, motion } from "motion/react";

export type Thought = { key: string; label: string; text: string };

/** At least this many words and characters, or it is a name, not a sentence. */
const MIN_WORDS = 5;
const MIN_CHARS = 30;
/** More waiting than this and the line jumps ahead to the newest few. */
const BACKLOG = 6;

/** A fact as a sentence to show, or null when it is too short to be one. */
export function asThought(key: string, label: string, value: string): Thought | null {
  const v = value.trim().replace(/\s+/g, " ");
  const words = v.split(" ");
  if (v.length < MIN_CHARS || words.length < MIN_WORDS) return null;
  // A page title or a list of names ("Amazon product template · Acme"): mostly capitals, or pieces joined by separators.
  if (/\s[·|•]\s/.test(v)) return null;
  const plain = words.filter((w) => /^[a-z]/.test(w)).length;
  if (plain < 3 || plain / words.length < 0.4) return null;
  const text = v[0].toUpperCase() + v.slice(1) + (/[.!?…)"”]$/.test(v) ? "" : ".");
  return { key, label, text };
}

/** How long a thought stays: long enough to read it, a little longer for long ones. */
const dwell = (t: Thought) => Math.min(9000, Math.max(4200, 2400 + t.text.length * 55));

export function Thinking({ thoughts, reduced, empty }: { thoughts: Thought[]; reduced: boolean; empty: string }) {
  // Every thought seen, in the order it came, and where the line is in it.
  const pool = useRef<Thought[]>([]);
  const seen = useRef(new Set<string>());
  const next = useRef(0);
  const [now, setNow] = useState<Thought | null>(null);
  const [tick, setTick] = useState(0);

  for (const t of thoughts) {
    if (seen.current.has(t.key)) continue;
    seen.current.add(t.key);
    pool.current.push(t);
  }
  const waiting = pool.current.length - next.current;

  // The first one shows as soon as it is there.
  useEffect(() => {
    if (!now && waiting > 0) setTick((n) => n + 1);
  }, [now, waiting]);

  useEffect(() => {
    if (tick === 0) return;
    const p = pool.current;
    let pick: Thought | null = null;
    if (next.current < p.length) {
      if (p.length - next.current > BACKLOG) next.current = p.length - 3;
      pick = p[next.current++];
    } else if (p.length >= 3) {
      // Nothing new: an earlier one, not the one just shown.
      const others = p.filter((t) => t.key !== now?.key);
      pick = others[Math.floor(Math.random() * others.length)];
    }
    if (pick) setNow(pick);
    const t = window.setTimeout(() => setTick((n) => n + 1), pick ? dwell(pick) : 1200);
    return () => window.clearTimeout(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tick]);

  const words = now ? now.text.split(" ") : [];
  return (
    <div className="wl-think" aria-live="polite">
      <AnimatePresence mode="wait" initial={false}>
        {now ? (
          <motion.div
            key={now.key}
            className="wl-think-in"
            initial={reduced ? { opacity: 0 } : { opacity: 0, y: 8, filter: "blur(6px)" }}
            animate={{ opacity: 1, y: 0, filter: "blur(0px)" }}
            exit={reduced ? { opacity: 0 } : { opacity: 0, y: -8, filter: "blur(6px)" }}
            transition={{ duration: reduced ? 0.2 : 0.45, ease: [0.2, 0.8, 0.2, 1] }}
          >
            <p className="cap">
              <span className="wl-think-dot" /> {now.label}
            </p>
            <p className="wl-think-line">
              {reduced
                ? now.text
                : words.map((w, i) => (
                    <motion.span key={i} initial={{ opacity: 0, filter: "blur(4px)" }} animate={{ opacity: 1, filter: "blur(0px)" }} transition={{ delay: 0.15 + i * 0.045, duration: 0.3 }}>
                      {w}{" "}
                    </motion.span>
                  ))}
            </p>
          </motion.div>
        ) : (
          <motion.p key="empty" className="dim sm wl-think-empty" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }}>
            {empty}
          </motion.p>
        )}
      </AnimatePresence>
    </div>
  );
}
