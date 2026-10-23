import { AnimatePresence, motion } from "motion/react";
import { Brain, Sparkles } from "lucide-react";
import { pathOf, sectionTitle, type CrawlState } from "./store";

const field = (f: string) => f.replace(/_/g, " ");

/** "Learning your business": per-page reads streaming in, then profile sections typing in. */
export function LearningPanel({ state, reducedMotion }: { state: CrawlState; reducedMotion: boolean }) {
  const a = state.analysis;
  if (a.read === 0 && state.phase !== "learning" && state.phase !== "done") return null;
  const writing = state.phase === "learning" && a.started;
  const pct = a.total ? Math.round((a.sections.length / a.total) * 100) : 0;
  const d = (s: number) => (reducedMotion ? 0 : s);
  return (
    <motion.section initial={reducedMotion ? false : { opacity: 0, y: 24 }} animate={{ opacity: 1, y: 0 }} className="cr-learn">
      <header>
        <h2>
          <Brain size={16} className="ic" />
          {state.phase === "done" ? "What we learned about your business" : writing ? "Writing your business profile…" : "Learning your business…"}
        </h2>
        <span className="cr-learn-meta">
          {writing && a.total > 0
            ? `${a.sections.length}/${a.total} sections · ${pct}%`
            : `${a.read} ${a.read === 1 ? "page" : "pages"} read · ${state.stats.facts} facts${a.waiting > 0 && state.phase !== "done" ? ` · ${a.waiting} waiting` : ""}`}
        </span>
      </header>

      {a.brief && a.brief.length > 0 && (
        <div className="cr-brief">
          <p className="cr-sec-h">
            <Sparkles size={12} className="ic" /> {sectionTitle("brief")}
          </p>
          <div className="cr-brief-chips">
            {a.brief.map((it, i) => (
              <motion.span
                key={it + i}
                initial={reducedMotion ? false : { opacity: 0, scale: 0.6 }}
                animate={{ opacity: 1, scale: 1 }}
                transition={{ delay: d(i * 0.08), type: "spring", stiffness: 300, damping: 20 }}
              >
                {it}
              </motion.span>
            ))}
          </div>
        </div>
      )}

      {a.sections.length > 0 ? (
        <div className="cr-secs">
          {a.sections.map((s, i) => (
            <motion.div
              key={s.section}
              initial={reducedMotion ? false : { opacity: 0, y: 16 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ delay: d(Math.min(i, 3) * 0.12) }}
              className="cr-sec"
            >
              <p className="cr-sec-h">
                <Sparkles size={12} className="ic acc" /> {s.label}
              </p>
              <ul>
                {s.items.slice(0, 6).map((it, j) => (
                  <motion.li key={j} initial={reducedMotion ? false : { opacity: 0, x: -8 }} animate={{ opacity: 1, x: 0 }} transition={{ delay: d(j * 0.06) }}>
                    {it}
                  </motion.li>
                ))}
              </ul>
            </motion.div>
          ))}
          {writing && <div className="cr-sec shimmer" />}
        </div>
      ) : (
        <ul className="cr-recent">
          <AnimatePresence initial={false}>
            {a.recent.slice(0, 5).map((p) => (
              <motion.li
                key={p.url}
                layout={!reducedMotion}
                initial={reducedMotion ? false : { opacity: 0, y: -10 }}
                animate={{ opacity: 1, y: 0 }}
                exit={{ opacity: 0 }}
              >
                <div className="cr-recent-h">
                  {p.facts[0] && <span className="cr-tag">{sectionTitle(p.facts[0].section)}</span>}
                  <span className="cr-recent-path">{pathOf(p.url)}</span>
                  <span className="cr-recent-n">
                    {p.count} {p.count === 1 ? "fact" : "facts"}
                  </span>
                </div>
                {p.facts.slice(0, 2).map((f, j) => (
                  <p key={j} className="cr-recent-f">
                    <span>{field(f.field)}</span> {f.value}
                  </p>
                ))}
              </motion.li>
            ))}
          </AnimatePresence>
          {a.recent.length === 0 && <li className="cr-recent-ph shimmer" />}
        </ul>
      )}
    </motion.section>
  );
}
