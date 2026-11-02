// While Seemo reads the site it introduces itself and asks four quick things,
// one at a time, in its own chat window (the widget in the corner). Every question can be skipped; every
// answer is saved at once and the brief and the first plan use them. The
// conversation is rebuilt from the saved answers, so a reload reads the same.
import { useContext, useEffect, useRef, useState } from "react";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { ArrowUp, Check, Cloud, Loader2, LogIn, Plus, Settings2, X } from "lucide-react";
import { api, type AbilitiesView, type Brand, type BuildStage, type Intake as IntakeT, type Network, type Setup, type SetupWhy } from "../api";
import { AbilityModal } from "../pages/AbilityModal";
import { useSignIns } from "../pages/SignIns";
import { IntroFollowUp } from "../SeemoWidget";
import "../chat.css";
import "../profile.css";
import "../settings.css";

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
  networks: "Where are you already? Social networks, a blog, a newsletter. I'll start where your customers already know you.",
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
  out.push({ from: "seemo", text: "Thanks. That's what I'll plan around, and your first steps in KIQR will start from it. I'll show you what I found as soon as I've read everything." });
  return out;
}

/**
 * "LinkedIn, X and Instagram" from what the owner typed: commas, "and", slashes.
 * People write sentences ("We have our website, X and Reddit."), so the words
 * around the names go, and so does the business's own site: that isn't a network.
 */
const FILLER = /^(?:(?:we|i)(?:'re| are|'m| am)?\s+(?:on|have|use|got|post on)|we|i|on|our|my|also|plus|then|and|a|an|the)\s+/i;
const splitNames = (s: string, host = "") => {
  const site = host.replace(/^www\./, "").toLowerCase();
  return s
    .split(/,|;|\/|\band\b|&|\+|\n/i)
    .map((x) => {
      let t = x.trim().replace(/[.!?:]+$/, "");
      for (let i = 0; i < 4 && FILLER.test(t); i++) t = t.replace(FILLER, "");
      return t.trim();
    })
    .filter((t) => t && !/\b(?:website|web site|site|homepage)\b/i.test(t) && !(site && t.toLowerCase().includes(site)))
    .slice(0, 12);
};

export function Intake({ slug, host, brand, suggestion, stage, onDone }: { slug: string; host: string; brand: Brand | null; /** Who the site seems to be for, from what Seemo read so far. */ suggestion?: string; stage?: BuildStage; /** Every question answered or skipped: the person can write to Seemo now. */ onDone?: () => void }) {
  const reduced = useReducedMotion();
  const [intake, setIntake] = useState<IntakeT | null>(null);
  const [networks, setNetworks] = useState<Network[]>([]);
  const [text, setText] = useState("");
  const [picked, setPicked] = useState<string[] | null>(null);
  const [own, setOwn] = useState("");
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
  const done = !!intake && ready && !q;
  useEffect(() => {
    if (done) onDone?.();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [done]);

  useEffect(() => {
    box.current?.scrollTo({ top: box.current.scrollHeight, behavior: reduced ? "auto" : "smooth" });
  }, [shown, typing, reduced]);

  // What the person wrote to Seemo after the questions, and its answers: the same conversation, carried on below.
  const followUp = useContext(IntroFollowUp);
  useEffect(() => {
    const el = box.current;
    if (!el || !followUp) return;
    // Follow the bottom as the reply streams in, unless they scrolled up to read.
    const stick = () => el.scrollHeight - el.scrollTop - el.clientHeight < 160 && el.scrollTo({ top: el.scrollHeight });
    stick();
    const mo = new MutationObserver(stick);
    mo.observe(el, { childList: true, subtree: true, characterData: true });
    return () => mo.disconnect();
  }, [followUp]);

  if (!intake) return <div className="wl-embedded" />;

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
    <div className="wl-embedded">

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
        {ready && !q && <SetUp slug={slug} host={host} stage={stage} onGrow={() => box.current?.scrollTo({ top: box.current.scrollHeight, behavior: reduced ? "auto" : "smooth" })} onDismiss={(id) => save({ ...intake, dismissed: [...(intake.dismissed ?? []), id] })} />}
        {followUp}
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
              <input className="input" placeholder={nets.length ? "Others? Like Instagram, TikTok, a newsletter" : "Like Instagram, LinkedIn, a newsletter"} value={own} onChange={(e) => setOwn(e.target.value)} onKeyDown={(e) => e.key === "Enter" && (chosen.length || splitNames(own, host).length) && save({ ...intake, networks: [...chosen, ...splitNames(own, host)] })} aria-label="Where else you are" />
              <button className="btn p sm wl-next" onClick={() => (chosen.length || splitNames(own, host).length ? save({ ...intake, networks: [...chosen, ...splitNames(own, host)] }) : skip("networks"))}>
                {chosen.length || splitNames(own, host).length ? "That's it" : "None yet"}
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

const GOAL_FOR: Record<string, string> = {
  customers: "For more new customers",
  repeat: "For selling to the customers you have",
  known: "For getting known",
  launch: "For your launch",
};

function why(w: SetupWhy, host: string) {
  if (w.kind === "site") return `I saw it on ${host}`;
  if (w.kind === "every_site") return "Suggested for all websites";
  return w.kind === "network" ? `You're on ${w.name}` : (GOAL_FOR[w.goal] ?? "For your goal");
}

const listed = (xs: string[]) => (xs.length < 2 ? (xs[0] ?? "") : `${xs.slice(0, -1).join(", ")} and ${xs[xs.length - 1]}`);

/**
 * Seemo's last word in the first conversation: what it would set up, from the
 * answers and what it saw on the site. Adding one installs it and opens it, on
 * its Access, to give it a credential right there; one added but not signed in yet opens
 * the same way. Asked again as the crawl goes on, since the site tells it more
 * the further it reads.
 */
function SetUp({ slug, host, stage, onGrow, onDismiss }: { slug: string; host: string; stage?: BuildStage; /** The chat scrolls to show it. */ onGrow: () => void; /** Not wanted: saved with the answers, and never suggested again. */ onDismiss: (id: string) => void }) {
  const [setup, setSetup] = useState<Setup | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  // The ability open in its modal, with what the modal needs: the project's ability list.
  const [opened, setOpened] = useState<{ id: string; tab: "overview" | "connect" } | null>(null);
  const [view, setView] = useState<AbilitiesView | null>(null);
  const load = () => api.setup(slug).then(setSetup, () => {});
  const reload = () => {
    void load();
    return api.abilities(slug).then(setView, () => {});
  };
  useEffect(() => {
    void load();
    if (stage === "ready" || stage === "failed") return;
    const t = window.setInterval(() => void load(), 12000);
    return () => window.clearInterval(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [slug, stage]);
  // kiqr.cloud first, and optional: it makes the sign-ins below one click.
  const signIns = useSignIns();
  const cloud = signIns.data?.signins.find((x) => x.id === "kiqr-cloud");
  const signInCloud = async () => {
    setBusy("kiqr-cloud");
    setError(null);
    try {
      const r = await api.startSignIn("kiqr-cloud", window.location.origin);
      if (r.mode === "redirect") window.open(r.url, "_blank", "noopener");
      signIns.reload();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(null);
    }
  };
  const size = (setup?.suggestions.length ?? 0) + (setup?.waiting.length ?? 0) + (cloud ? 1 : 0);
  useEffect(() => {
    if (size) onGrow();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [size]);
  if (!setup || size === 0) return null;
  const sugs = setup.suggestions;

  /** Install it when it isn't yet, then open it on its Access (the modal falls back to its overview when it has none). */
  const setUp = async (id: string, installed: boolean) => {
    setBusy(id);
    setError(null);
    try {
      if (!installed) await api.installAbility(id);
      await reload();
      setOpened({ id, tab: "connect" });
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(null);
    }
  };

  return (
    <motion.div className="cm ai" initial={{ opacity: 0, y: 6 }} animate={{ opacity: 1, y: 0 }}>
      <span className="av" />
      <div className="body">
        {(sugs.length > 0 || cloud) && (
          <>
            <p>{sugs.length ? `From what you told me and what I see on ${host}, I'd set these up:` : "One thing you can set up now:"}</p>
            <ul className="wl-sugs">
              {cloud && (
                <li>
                  <span className="wl-sug-ic">
                    <Cloud size={15} />
                  </span>
                  <div className="min0">
                    <b>
                      Sign in to kiqr.cloud <span className="chip sm wl-opt">Optional</span>
                    </b>
                    <small>{cloud.signed_in ? `Signed in as ${cloud.account ?? "you"}` : cloud.waiting ? `Enter ${cloud.waiting.user_code.replace(/^(.{4})(.{4})$/, "$1-$2")} at ${cloud.waiting.verification_uri.replace(/^https?:\/\//, "")}` : "Makes signing in to the ones below one click"}</small>
                  </div>
                  {cloud.signed_in ? (
                    <span className="chip ok sm">
                      <Check size={11} strokeWidth={3} /> Ready
                    </span>
                  ) : cloud.waiting ? (
                    <a className="btn g sm" href={cloud.waiting.verification_uri_complete ?? cloud.waiting.verification_uri} target="_blank" rel="noreferrer">
                      Open
                    </a>
                  ) : (
                    <button className="btn g sm" disabled={!!busy} onClick={() => void signInCloud()}>
                      {busy === "kiqr-cloud" ? <Loader2 size={12} className="spin" /> : <LogIn size={12} />} Sign in
                    </button>
                  )}
                </li>
              )}
              {sugs.map((s) => (
                <li key={s.id}>
                  {s.avatar ? <img src={s.avatar} alt="" width={28} height={28} /> : <span className="wl-sug-ic" />}
                  <div className="min0">
                    <b>{s.name}</b>
                    <small>{s.why.map((w) => why(w, host)).join(" · ")}</small>
                  </div>
                  {s.ready ? (
                    <span className="chip ok sm">
                      <Check size={11} strokeWidth={3} /> Ready
                    </span>
                  ) : (
                    <button className={`btn sm ${s.installed ? "g" : "p"}`} disabled={!!busy} onClick={() => void setUp(s.id, s.installed)}>
                      {busy === s.id ? <Loader2 size={12} className="spin" /> : s.installed ? <Settings2 size={12} /> : <Plus size={12} />} {s.installed ? "Set up" : "Add"}
                    </button>
                  )}
                  {!s.ready && (
                    <button
                      className="btn t sm wl-sug-x"
                      title={`Not for me: don't suggest ${s.name}`}
                      aria-label={`Not for me: don't suggest ${s.name}`}
                      disabled={!!busy}
                      onClick={() => {
                        onDismiss(s.id);
                        setSetup((x) => (x ? { ...x, suggestions: x.suggestions.filter((y) => y.id !== s.id) } : x));
                      }}
                    >
                      <X size={13} />
                    </button>
                  )}
                </li>
              ))}
            </ul>
            {sugs.some((s) => !s.ready) && <p className="dim sm">Each one opens to sign in. You can also do it later, from your first steps.</p>}
          </>
        )}
        {setup.waiting.length > 0 && <p>{listed(setup.waiting)}: I can't post there yet. When I can, I'll tell you.</p>}
        {error && <p className="dim sm">{error}</p>}
      </div>
      <AnimatePresence>
        {opened && view && (() => {
          const a = [...view.abilities, ...view.market].find((x) => x.id === opened.id);
          return a ? <AbilityModal key={a.id} slug={slug} a={a} contracts={view.contracts} initialTab={opened.tab} onClose={() => (setOpened(null), void load())} onChange={() => void reload()} onPrefer={(contract, id) => void prefer(contract, id).then(reload)} /> : null;
        })()}
      </AnimatePresence>
    </motion.div>
  );
}

/** The modal's "use this one for …" choice, saved the way the Abilities page saves it. */
async function prefer(contract: string, id: string) {
  const c = await api.config();
  await api.saveConfig({ ...c, abilities: { ...(c.abilities ?? {}), [contract]: id } });
}
