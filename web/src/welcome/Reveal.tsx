// "Here's your business, as I see it": the brief in five short parts, each
// one the owner can confirm or fix on the spot, then on to the first plan.
import { useState } from "react";
import { motion, useReducedMotion } from "motion/react";
import { ArrowRight, Check, Lightbulb, Megaphone, Package, PenLine, Users } from "lucide-react";
import { api, type Brand, type Brief } from "../api";

type Part = { id: string; icon: typeof Users; title: string; lines: string[]; edit: string; apply: (b: Brief, text: string) => Brief };

const list = (text: string) =>
  text
    .split(/\n|,/)
    .map((s) => s.trim())
    .filter(Boolean);

function parts(b: Brief): Part[] {
  const all: Part[] = [
    {
      id: "summary",
      icon: Megaphone,
      title: "What you do",
      lines: [b.summary],
      edit: b.summary,
      apply: (x, t) => ({ ...x, summary: t.trim() }),
    },
    {
      id: "offerings",
      icon: Package,
      title: "What you sell",
      lines: b.offerings.slice(0, 5).map((o) => o.name),
      edit: b.offerings.map((o) => o.name).join("\n"),
      apply: (x, t) => {
        const names = list(t);
        return { ...x, offerings: names.map((n) => x.offerings.find((o) => o.name === n) ?? { name: n, kind: "product", description: "" }) };
      },
    },
    {
      id: "audiences",
      icon: Users,
      title: "Who it's for",
      lines: b.audiences.slice(0, 4).map((a) => a.name),
      edit: b.audiences.map((a) => a.name).join("\n"),
      apply: (x, t) => {
        const names = list(t);
        return { ...x, audiences: names.map((n) => x.audiences.find((a) => a.name === n) ?? { name: n, needs: [] }) };
      },
    },
    {
      id: "voice",
      icon: PenLine,
      title: "How you sound",
      lines: [b.tone_of_voice.traits.slice(0, 4).join(" · ")].filter(Boolean),
      edit: b.tone_of_voice.traits.join(", "),
      apply: (x, t) => ({ ...x, tone_of_voice: { ...x.tone_of_voice, traits: list(t) } }),
    },
    {
      id: "gaps",
      icon: Lightbulb,
      title: "Where I see openings",
      lines: b.content_gaps.slice(0, 3).map((g) => g.topic),
      edit: "",
      apply: (x) => x,
    },
  ];
  return all.filter((p) => p.lines.some((l) => l.trim()));
}

export function Reveal({ slug, brief, brand, blocked, onNext, onSaved }: { slug: string; brief: Brief; brand: Brand | null; blocked: number; onNext: () => void; onSaved: (b: Brief) => void }) {
  const reduced = useReducedMotion();
  const [ok, setOk] = useState<Set<string>>(new Set());
  const [editing, setEditing] = useState<string | null>(null);
  const [text, setText] = useState("");
  const [error, setError] = useState<string | null>(null);
  const all = parts(brief);
  const name = brief.business_name || brand?.site_name || "your business";

  const save = async (p: Part) => {
    setError(null);
    try {
      const next = await api.saveBrief(slug, p.apply(brief, text));
      onSaved(next);
      setOk(new Set(ok).add(p.id));
      setEditing(null);
    } catch (e) {
      setError((e as Error).message);
    }
  };

  return (
    <div className="wl-reveal">
      <motion.div initial={reduced ? false : { opacity: 0, y: 12 }} animate={{ opacity: 1, y: 0 }} className="wl-reveal-h">
        <p className="cap">I read the whole site</p>
        <h1 className="d1">
          Here's <em>{name}</em>, as I see it.
        </h1>
        <p className="lead">Tell me what's right and fix what isn't. Everything I make starts from this.</p>
      </motion.div>

      {brand && (brand.colors.length > 0 || brand.logo) && (
        <motion.div initial={reduced ? false : { opacity: 0 }} animate={{ opacity: 1 }} transition={{ delay: 0.2 }} className="wl-brandline">
          {brand.logo && <img src={brand.logo} alt="" onError={(e) => (e.currentTarget.style.display = "none")} />}
          {brand.colors.slice(0, 6).map((c) => (
            <i key={c} style={{ background: c }} title={c} />
          ))}
          {brand.fonts[0] && <span className="mono sm">{brand.fonts[0]}</span>}
        </motion.div>
      )}

      <div className="wl-parts">
        {all.map((p, i) => (
          <motion.section
            key={p.id}
            initial={reduced ? false : { opacity: 0, y: 16 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ delay: 0.25 + i * 0.09, duration: 0.35 }}
            className={`wl-part ${ok.has(p.id) ? "ok" : ""}`}
          >
            <header>
              <span className="ic">
                <p.icon size={15} />
              </span>
              <h3>{p.title}</h3>
              {ok.has(p.id) && (
                <span className="chip ok sm">
                  <Check size={11} strokeWidth={3} /> Spot on
                </span>
              )}
            </header>
            {editing === p.id ? (
              <div className="wl-fix">
                <textarea className="input" rows={p.id === "summary" ? 4 : 5} value={text} onChange={(e) => setText(e.target.value)} autoFocus aria-label={p.title} />
                <p className="dim sm">{p.id === "summary" ? "" : "One per line."}</p>
                <div className="row">
                  <button className="btn p sm" disabled={!text.trim()} onClick={() => void save(p)}>
                    Save
                  </button>
                  <button className="btn t sm" onClick={() => setEditing(null)}>
                    Cancel
                  </button>
                </div>
              </div>
            ) : (
              <>
                {p.lines.length === 1 && p.id === "summary" ? <p className="wl-summary">{p.lines[0]}</p> : (
                  <ul>
                    {p.lines.map((l) => (
                      <li key={l}>{l}</li>
                    ))}
                  </ul>
                )}
                {!ok.has(p.id) && p.edit !== "" && (
                  <div className="row">
                    <button className="btn g sm" onClick={() => setOk(new Set(ok).add(p.id))}>
                      <Check size={12} /> Spot on
                    </button>
                    <button className="btn t sm" onClick={() => (setEditing(p.id), setText(p.edit))}>
                      Fix this
                    </button>
                  </div>
                )}
              </>
            )}
          </motion.section>
        ))}
      </div>
      {error && <p className="err sm">{error}</p>}
      {blocked > 0 && (
        <p className="muted sm wl-blocked">
          {blocked} page{blocked === 1 ? "" : "s"} turned crawlers away, so I read around {blocked === 1 ? "it" : "them"}. UnblockingAPI can read pages like these; you can add it under Setup → Abilities.
        </p>
      )}
      <motion.div initial={reduced ? false : { opacity: 0 }} animate={{ opacity: 1 }} transition={{ delay: 0.3 + all.length * 0.09 }} className="wl-cta">
        <button className="btn a wl-big" onClick={onNext}>
          Make my marketing plan <ArrowRight size={16} />
        </button>
      </motion.div>
    </div>
  );
}
