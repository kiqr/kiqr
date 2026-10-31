// The big red button: erase the whole KIQR home and start again as a new
// installation. Four locks: a first "are you sure" that lists what goes, a
// captcha of our own (a sum drawn by the daemon, checked there), the phrase
// typed out, and a button held for two seconds. The daemon adds its own: a
// one-time token, and it only takes the request from this page (never from
// Seemo, the CLI or an ability).
import { useEffect, useRef, useState } from "react";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { Bomb, Loader2, RefreshCw, ShieldAlert, X } from "lucide-react";
import { api, type Project } from "../api";

const HOLD_MS = 2000;

type Step = "closed" | "sure" | "type" | "erasing";

export function FactoryReset({ home, boot }: { home?: string; boot?: string }) {
  const [step, setStep] = useState<Step>("closed");
  const [projects, setProjects] = useState<Project[]>([]);
  const [token, setToken] = useState<{ token: string; phrase: string; captcha: string } | null>(null);
  const [typed, setTyped] = useState("");
  const [sum, setSum] = useState("");
  const [error, setError] = useState<string | null>(null);

  const open = () => {
    setError(null);
    setTyped("");
    setSum("");
    setToken(null);
    api.projects().then(setProjects, () => setProjects([]));
    setStep("sure");
  };
  const close = () => step !== "erasing" && setStep("closed");
  /** A new token and a new captcha; `keep` leaves an error showing (after a wrong answer). */
  const toType = async (keep?: boolean) => {
    if (!keep) setError(null);
    setSum("");
    try {
      setToken(await api.resetStart());
      setStep("type");
    } catch (e) {
      setError((e as Error).message);
      setStep("sure");
    }
  };
  const erase = async () => {
    if (!token) return;
    setError(null);
    try {
      const r = await api.reset(token.token, typed, sum);
      setStep("erasing");
      waitForFresh(r.port, boot);
    } catch (e) {
      // The token is spent either way: a new one, with a new sum.
      setError((e as Error).message);
      void toType(true);
    }
  };

  useEffect(() => {
    if (step === "closed" || step === "erasing") return;
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setStep("closed");
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [step]);

  const phrase = token?.phrase ?? "erase everything";
  const matches = typed.trim().toLowerCase() === phrase && /^\d+$/.test(sum.trim());

  return (
    <>
      <section className="pcard set-card fr-card">
        <div className="set-card-head">
          <span className="ic">
            <Bomb size={16} />
          </span>
          <div>
            <h2>Factory reset</h2>
            <p>Erase everything KIQR keeps on this machine and start over as if you had just installed it. The program itself stays.</p>
          </div>
        </div>
        <div className="fr-row">
          <p className="muted sm">For when you want a clean slate. There is no undo, not even in history.</p>
          <button className="btn fr-btn" onClick={open}>
            <Bomb size={14} /> Factory reset…
          </button>
        </div>
      </section>

      <AnimatePresence>
        {step !== "closed" && (
          <motion.div className="fr-back" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} onClick={close}>
            <motion.div className="fr-modal" role="alertdialog" aria-modal aria-labelledby="fr-title" initial={{ opacity: 0, y: 14, scale: 0.98 }} animate={{ opacity: 1, y: 0, scale: 1 }} exit={{ opacity: 0, y: 8 }} onClick={(e) => e.stopPropagation()}>
              {step === "erasing" ? (
                <div className="fr-erasing">
                  <Loader2 size={22} className="spin" />
                  <h2 id="fr-title">Starting fresh</h2>
                  <p className="muted">Erasing {home ?? "the home folder"} and starting KIQR again as a new installation. This page reloads when it is back.</p>
                </div>
              ) : (
                <>
                  <header>
                    <span className="fr-ic">
                      <ShieldAlert size={18} />
                    </span>
                    <h2 id="fr-title">{step === "sure" ? "This is the big red button" : "Last chance"}</h2>
                    <button className="btn t" aria-label="Close" onClick={close}>
                      <X size={16} />
                    </button>
                  </header>
                  {step === "sure" ? (
                    <>
                      <p>Everything in {home ? <code>{home}</code> : "KIQR's home folder"} is erased for good:</p>
                      <ul className="fr-list">
                        <li>
                          {projects.length ? (
                            <>
                              <b>{projects.length === 1 ? "1 project" : `${projects.length} projects`}</b>: {projects.map((p) => p.name || p.slug).join(", ")}
                            </>
                          ) : (
                            <b>Every project</b>
                          )}
                          , with their profiles, plans, chats, tasks and everything Seemo made
                        </li>
                        <li>Every account and key, and your model keys</li>
                        <li>Installed abilities and what you taught Seemo</li>
                        <li>Your settings, and the whole history, so nothing can be brought back</li>
                      </ul>
                      <p className="muted sm">Then KIQR starts again as if you had just installed it. Published posts stay where they were published.</p>
                      {error && <p className="fr-err">{error}</p>}
                      <div className="fr-acts">
                        <button className="btn p" autoFocus onClick={close}>
                          Keep everything
                        </button>
                        <button className="btn fr-btn" onClick={() => void toType()}>
                          I understand, continue
                        </button>
                      </div>
                    </>
                  ) : (
                    <>
                      <p>Prove you're a person, not Seemo: solve the sum.</p>
                      <div className="fr-captcha">
                        {token && <img src={token.captcha} alt="A sum to solve, drawn as a picture" width={320} height={96} />}
                        <div className="fr-captcha-in">
                          <input className="input fr-input" inputMode="numeric" value={sum} onChange={(e) => setSum(e.target.value.replace(/[^0-9]/g, ""))} placeholder="Answer" aria-label="The answer to the sum" autoFocus autoComplete="off" />
                          <button className="btn t" type="button" title="Another sum" aria-label="Another sum" onClick={() => void toType()}>
                            <RefreshCw size={14} />
                          </button>
                        </div>
                      </div>
                      <p>
                        Then type <code className="fr-phrase">{phrase}</code> and hold the button.
                      </p>
                      <input className="input fr-input" value={typed} onChange={(e) => setTyped(e.target.value)} placeholder={phrase} aria-label={`Type ${phrase}`} spellCheck={false} autoComplete="off" />
                      {error && <p className="fr-err">{error}</p>}
                      <div className="fr-acts">
                        <button className="btn g" onClick={close}>
                          Cancel
                        </button>
                        <HoldButton disabled={!matches} onDone={() => void erase()} />
                      </div>
                    </>
                  )}
                </>
              )}
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>
    </>
  );
}

/** Pressed and held for two seconds; letting go early cancels. Keyboard: hold Space or Enter. */
function HoldButton({ disabled, onDone }: { disabled: boolean; onDone: () => void }) {
  const reduced = useReducedMotion();
  const [holding, setHolding] = useState(false);
  const timer = useRef<number | null>(null);
  const start = () => {
    if (disabled || timer.current) return;
    setHolding(true);
    timer.current = window.setTimeout(() => {
      timer.current = null;
      setHolding(false);
      onDone();
    }, HOLD_MS);
  };
  const stop = () => {
    if (timer.current) window.clearTimeout(timer.current);
    timer.current = null;
    setHolding(false);
  };
  useEffect(() => stop, []);
  return (
    <button
      className={`btn fr-hold ${holding ? "on" : ""}`}
      disabled={disabled}
      onPointerDown={start}
      onPointerUp={stop}
      onPointerLeave={stop}
      onPointerCancel={stop}
      onKeyDown={(e) => (e.key === " " || e.key === "Enter") && !e.repeat && (e.preventDefault(), start())}
      onKeyUp={(e) => (e.key === " " || e.key === "Enter") && stop()}
      onContextMenu={(e) => e.preventDefault()}
    >
      <motion.i initial={false} animate={{ width: holding ? "100%" : "0%" }} transition={{ duration: holding ? (reduced ? 0 : HOLD_MS / 1000) : 0.2, ease: "linear" }} />
      <span>
        <Bomb size={14} /> {holding ? "Keep holding…" : "Hold to erase everything"}
      </span>
    </button>
  );
}

/** Wait until the daemon answers again after its restart (a new boot id), then open it like a new installation. */
function waitForFresh(port: number, boot?: string) {
  const origin = `${location.protocol}//${location.hostname}:${port}`;
  const same = origin === location.origin;
  const tick = async () => {
    for (;;) {
      await new Promise((r) => setTimeout(r, 1000));
      const up = same
        ? await fetch("/api/health", { cache: "no-store" }).then((r) => r.json()).then((x: { boot?: string }) => !!x.boot && x.boot !== boot, () => false)
        : await fetch(`${origin}/api/health`, { mode: "no-cors", cache: "no-store" }).then(() => true, () => false);
      if (!up) continue;
      // This browser's own leftovers (skipped steps, the last chat) belong to the projects that are gone.
      try {
        Object.keys(localStorage).filter((k) => k.startsWith("kiqr")).forEach((k) => localStorage.removeItem(k));
      } catch {
        /* private window */
      }
      return void (window.location.href = `${origin}/`);
    }
  };
  void tick();
}
