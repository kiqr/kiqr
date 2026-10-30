// While Seemo reads the site it introduces itself and asks four quick things,
// one at a time, in the chat's own look. Every question can be skipped; every
// answer is saved at once and the brief and the first plan use them. The
// conversation is rebuilt from the saved answers, so a reload reads the same.
import { useEffect, useRef, useState } from "react";
import { motion, useReducedMotion } from "motion/react";
import { ArrowUp, Check } from "lucide-react";
import { api, type Brand, type Intake as IntakeT, type Network } from "../api";
import "../chat.css";

const EMPTY: IntakeT = { goal: "", goal_note: "", customers: "", time: "", networks: [], skipped: [] };

const GOALS: { id: IntakeT["goal"]; label: string; reply: string }[] = [
  { id: "customers", label: "More new customers", reply: "New customers it is. I'll look at who searches for what you sell, and how to reach them." },
  { id: "repeat", label: "Sell more to the customers I have", reply: "Then I'll focus on bringing people back, and on what they could buy next." },
  { id: "known", label: "Get known", reply: "Then the first job is getting your name in front of the right people." },
  { id: "launch", label: "Launch something new", reply: "Good. I'll plan towards the launch." },
];

const TIMES: { id: IntakeT["time"]; label: string; reply: string }[] = [
  { id: "lt1", label: "Less than an hour", reply: "Then I'll keep your part under an hour and do the rest myself." },
  { id: "1to3", label: "One to three hours", reply: "That's enough for a steady rhythm." },
  { id: "3plus", label: "More than that", reply: "Then we can get a lot done together." },
];

type Q = "goal" | "customers" | "time" | "networks";
const ORDER: Q[] = ["goal", "customers", "time", "networks"];
const ASK: Record<Q, string> = {
  goal: "What matters most to you right now?",
  customers: "Who are your best customers? The ones you'd like ten more of. A sentence is plenty.",
  time: "How much time can you give marketing each week? I'll plan for what you have, not for a team you don't.",
  networks: "Where are you already? I'll start where your customers already know you.",
};

type Line = { from: "seemo" | "you" | "skip"; text: string };

const answered = (i: IntakeT, q: Q) =>
  i.skipped.includes(q) || (q === "goal" ? !!i.goal || !!i.goal_note.trim() : q === "customers" ? !!i.customers.trim() : q === "time" ? !!i.time : i.networks.length > 0);

/** The conversation so far, from what was answered. */
function transcript(i: IntakeT, host: string, names: Map<string, string>): Line[] {
  const out: Line[] = [
    { from: "seemo", text: "Hi, I'm Seemo, your new marketing colleague." },
    { from: "seemo", text: `I'm reading ${host} right now. While I do, can I ask you four quick things? Skip any you like.` },
  ];
  for (const q of ORDER) {
    out.push({ from: "seemo", text: ASK[q] });
    if (!answered(i, q)) return out;
    if (i.skipped.includes(q)) {
      out.push({ from: "skip", text: "Skipped" }, { from: "seemo", text: "No problem." });
      continue;
    }
    if (q === "goal") {
      const g = GOALS.find((x) => x.id === i.goal);
      out.push({ from: "you", text: g?.label ?? i.goal_note }, { from: "seemo", text: g?.reply ?? "Understood. That's what I'll work towards." });
    } else if (q === "customers") {
      out.push({ from: "you", text: i.customers }, { from: "seemo", text: "Good to know. Everything I write will be for them." });
    } else if (q === "time") {
      const t = TIMES.find((x) => x.id === i.time);
      out.push({ from: "you", text: t?.label ?? i.time }, { from: "seemo", text: t?.reply ?? "Noted." });
    } else {
      const list = i.networks.map((k) => names.get(k) ?? k);
      out.push({ from: "you", text: list.join(", ") }, { from: "seemo", text: `${list.length > 1 ? "Those" : list[0]} it is. I'll start there.` });
    }
  }
  out.push({ from: "seemo", text: "Thanks. That's what I'll plan around. I'll show you what I found as soon as I've read everything." });
  return out;
}

export function Intake({ slug, host, brand, suggestion, done }: { slug: string; host: string; brand: Brand | null; /** Who the site seems to be for, from what Seemo read so far. */ suggestion?: string; /** Seemo has finished reading. */ done?: boolean }) {
  const reduced = useReducedMotion();
  const [intake, setIntake] = useState<IntakeT | null>(null);
  const [networks, setNetworks] = useState<Network[]>([]);
  const [text, setText] = useState("");
  const [picked, setPicked] = useState<string[] | null>(null);
  // How many lines are out: Seemo's arrive one by one, after a moment of "typing".
  const [shown, setShown] = useState(0);
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => {
    api.intake(slug).then((i) => setIntake({ ...EMPTY, ...i }), () => setIntake(EMPTY));
    api.networks(slug).then((r) => setNetworks(r.networks.filter((n) => n.network)), () => setNetworks([]));
  }, [slug]);

  const names = new Map(networks.map((n) => [n.kind, n.name]));
  const lines = intake ? transcript(intake, host, names) : [];
  const q = intake ? ORDER.find((x) => !answered(intake, x)) : undefined;

  // A conversation that already happened (a reload) is there at once; a new one is typed out.
  const started = useRef(false);
  useEffect(() => {
    if (!intake || started.current) return;
    started.current = true;
    const fresh = ORDER.every((x) => !answered(intake, x));
    setShown(fresh && !reduced ? 0 : lines.length);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [intake]);
  // Keyed on plain values: the page re-renders on every crawl event, and a new array each time would restart the pause.
  const total = lines.length;
  const nextFrom = lines[shown]?.from;
  const nextLen = lines[shown]?.text.length ?? 0;
  useEffect(() => {
    if (shown >= total) return;
    if (reduced || nextFrom !== "seemo") return void setShown(shown + 1);
    const t = setTimeout(() => setShown(shown + 1), Math.min(1400, 450 + nextLen * 12));
    return () => clearTimeout(t);
  }, [shown, total, nextFrom, nextLen, reduced]);
  const typing = shown < lines.length && lines[shown].from === "seemo";
  const ready = shown >= lines.length;

  useEffect(() => {
    box.current?.scrollTo({ top: box.current.scrollHeight, behavior: reduced ? "auto" : "smooth" });
  }, [shown, typing, reduced]);

  if (!intake) return <div className="wl-intake" />;

  const save = (next: IntakeT) => {
    setIntake(next);
    setText("");
    setPicked(null);
    api.saveIntake(slug, next).catch(() => {});
  };
  const skip = (x: Q) => save({ ...intake, skipped: [...intake.skipped, x] });

  // The networks the business is already on (from its site) come first and start ticked.
  const found = new Set((brand?.social ?? []).map((s) => s.network.toLowerCase()));
  const nets = [...networks].sort((a, b) => Number(found.has(b.kind)) - Number(found.has(a.kind))).slice(0, 10);
  const chosen = picked ?? [...found].filter((k) => nets.some((n) => n.kind === k));

  const submitText = (e: React.FormEvent) => {
    e.preventDefault();
    const t = text.trim();
    if (!t || !q) return;
    if (q === "goal") save({ ...intake, goal_note: t });
    if (q === "customers") save({ ...intake, customers: t });
  };

  return (
    <div className="wl-intake wl-chat">
      <header className="wl-chat-h">
        <span className="av" />
        <div>
          <b>Seemo</b>
          <span className="dim sm">
            {done ? <>read {host}</> : <><i className="wl-live" /> reading {host}</>}
          </span>
        </div>
      </header>

      <div className="conv compact wl-conv" ref={box}>
        {lines.slice(0, shown).map((l, i) =>
          l.from === "seemo" ? (
            <div key={i} className="cm ai">
              {lines[i - 1]?.from === "seemo" ? <span className="av wl-av-gap" /> : <span className="av" />}
              <div className="body">
                <p>{l.text}</p>
              </div>
            </div>
          ) : l.from === "skip" ? (
            <div key={i} className="cm note">
              <span>Skipped</span>
            </div>
          ) : (
            <div key={i} className="cm you">
              <div className="ms y">{l.text}</div>
            </div>
          ),
        )}
        {typing && (
          <div className="cm ai live">
            <span className="av" />
            <div className="body">
              <p className="thinking">
                <i />
                <i />
                <i />
              </p>
            </div>
          </div>
        )}
      </div>

      {ready && q && (
        <motion.div className="wl-reply" initial={reduced ? false : { opacity: 0, y: 6 }} animate={{ opacity: 1, y: 0 }} key={q}>
          {q === "goal" && (
            <div className="wl-chips">
              {GOALS.map((g) => (
                <button key={g.id} className="wl-chip" onClick={() => save({ ...intake, goal: g.id })}>
                  {g.label}
                </button>
              ))}
            </div>
          )}
          {q === "time" && (
            <div className="wl-chips">
              {TIMES.map((t) => (
                <button key={t.id} className="wl-chip" onClick={() => save({ ...intake, time: t.id })}>
                  {t.label}
                </button>
              ))}
            </div>
          )}
          {q === "networks" && (
            <>
              <div className="wl-chips">
                {nets.map((n) => {
                  const on = chosen.includes(n.kind);
                  return (
                    <button key={n.kind} className={`wl-chip ${on ? "on" : ""}`} aria-pressed={on} onClick={() => setPicked(on ? chosen.filter((k) => k !== n.kind) : [...chosen, n.kind])}>
                      {on && <Check size={12} strokeWidth={3} />} {n.name}
                    </button>
                  );
                })}
              </div>
              <button className="btn p sm wl-next" onClick={() => (chosen.length ? save({ ...intake, networks: chosen }) : skip("networks"))}>
                {chosen.length ? "That's it" : "None yet"}
              </button>
            </>
          )}
          {(q === "goal" || q === "customers") && (
            <form className="wl-own" onSubmit={submitText}>
              {q === "goal" ? (
                <input className="input" placeholder="Or say it in your own words" value={text} onChange={(e) => setText(e.target.value)} aria-label="Your answer" />
              ) : (
                <textarea
                  className="input"
                  rows={2}
                  autoFocus
                  placeholder={suggestion ? `Like: ${suggestion}` : "Like: small cafés in the city that order every week"}
                  value={text}
                  onChange={(e) => setText(e.target.value)}
                  onKeyDown={(e) => e.key === "Enter" && !e.shiftKey && submitText(e)}
                  aria-label="Your answer"
                />
              )}
              <button className="btn p sm" disabled={!text.trim()} aria-label="Send">
                <ArrowUp size={14} />
              </button>
            </form>
          )}
          <div className="wl-q-foot">
            <span className="dim sm">
              {ORDER.indexOf(q) + 1} of {ORDER.length}
            </span>
            <button className="btn t sm" onClick={() => skip(q)}>
              Skip
            </button>
          </div>
        </motion.div>
      )}
    </div>
  );
}
