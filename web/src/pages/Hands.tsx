// The live view ("Take the controls"): what a login's hosted program (a
// browser) shows, a few pictures a second over the project's event stream,
// and a person driving it for a moment to finish a sign-in the runner could
// not (a captcha, a device check). A click on the picture clicks there; keys
// typed while it has focus go to the page (text in short batches, Enter,
// Backspace, Tab, Escape and the arrows one by one); the address bar opens a
// page. "I'm signed in" ends it and has KIQR check; "Done" just ends it.
// Pictures are painted and dropped, never kept. Opened from a login in the Vault.
import { useEffect, useRef, useState, type KeyboardEvent as ReactKeyboardEvent, type PointerEvent as ReactPointerEvent } from "react";
import { createPortal } from "react-dom";
import { motion } from "motion/react";
import { Check, Globe, Keyboard, Loader2, MousePointerClick, X } from "lucide-react";
import { api, useEvents, type HandsCan, type HandsInput } from "../api";
import "../library.css";
import "../accounts.css";

/** Keys sent one by one, by the name the program takes. */
const SPECIAL = new Set(["Enter", "Backspace", "Tab", "Escape", "Delete", "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "Home", "End", "PageUp", "PageDown"]);
const TYPE_DELAY = 150;

/** A web address from what the account view shows about it (where it starts, its identity), when it is one. */
export function startUrlOf(...values: (string | undefined | null)[]): string | undefined {
  for (const v of values) {
    const t = (v ?? "").trim();
    if (/^https?:\/\/\S+$/i.test(t)) return t;
  }
  return undefined;
}

type Session = { hands: string; token: string; can: HandsCan };

export function Hands({ slug, account, name, startUrl, onClose }: { slug: string; account: string; name?: string; startUrl?: string; onClose: (signedIn: boolean) => void }) {
  const [session, setSession] = useState<Session | null>(null);
  const [frame, setFrame] = useState<string | null>(null);
  const [url, setUrl] = useState(startUrl ?? "");
  const [editingUrl, setEditingUrl] = useState(false);
  const [ended, setEnded] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [focused, setFocused] = useState(false);
  const [busy, setBusy] = useState(false);
  const live = useRef<Session | null>(null);
  const over = useRef(false);
  const img = useRef<HTMLImageElement>(null);
  const keys = useRef<HTMLTextAreaElement>(null);
  // Inputs go out one after the other, in the order they were made.
  const queue = useRef<Promise<unknown>>(Promise.resolve());
  const text = useRef("");
  const timer = useRef<number | undefined>(undefined);

  useEffect(() => {
    let gone = false;
    api.startHands(slug, account, startUrl).then(
      (s) => {
        if (gone) {
          void api.endHands(slug, s.hands, s.token, false).catch(() => undefined);
          return;
        }
        live.current = s;
        setSession(s);
      },
      (e) => !gone && setError((e as Error).message),
    );
    return () => {
      gone = true;
      window.clearTimeout(timer.current);
      // Closed some other way (the page left): free the login.
      const s = live.current;
      if (s && !over.current) void api.endHands(slug, s.hands, s.token, false).catch(() => undefined);
    };
  }, [slug, account]);

  useEvents(
    slug,
    (e) => {
      const s = live.current;
      // Events name the credential and the session (`hands`): only this one's.
      if (!s || e.hands !== s.hands || (e.credential != null && e.credential !== account)) return;
      if (e.type === "hands_frame" && !over.current) {
        // Only the latest picture is held, to paint it.
        setFrame(`data:${String(e.mime || "image/png")};base64,${String(e.data ?? "")}`);
        if (typeof e.url === "string" && e.url && !editingUrl) setUrl(e.url);
      }
      if (e.type === "hands_ended") {
        over.current = true;
        setFrame(null);
        setEnded(String(e.message ?? ""));
      }
    },
    { demo: false },
  );

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && !focused && void finish(false);
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  const send = (input: HandsInput) => {
    const s = live.current;
    if (!s || over.current) return;
    queue.current = queue.current.then(() =>
      api.handsInput(slug, s.hands, s.token, input).then((r) => {
        // The browser refused it (a click on its own bar, a key it does not know): said once, gone on the next input.
        setError(r.ok ? null : r.message || "the browser did not take that");
      }).catch((e) => {
        const m = (e as Error).message;
        if (/over|not your session/i.test(m)) {
          over.current = true;
          setEnded(m);
        } else setError(m);
      }),
    );
  };
  const flush = () => {
    window.clearTimeout(timer.current);
    if (text.current) send({ kind: "type", text: text.current });
    text.current = "";
  };
  const typed = (t: string) => {
    if (!t) return;
    text.current += t;
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(flush, TYPE_DELAY);
  };

  const click = (e: ReactPointerEvent<HTMLImageElement>) => {
    const el = img.current;
    if (!el || !session?.can.click) return;
    const r = el.getBoundingClientRect();
    const x = Math.min(1, Math.max(0, (e.clientX - r.left) / r.width));
    const y = Math.min(1, Math.max(0, (e.clientY - r.top) / r.height));
    flush();
    send({ kind: "click", x, y, width: el.naturalWidth, height: el.naturalHeight });
    // With a mouse the keys follow the click; on a touch screen the keyboard opens only when asked.
    if (e.pointerType !== "touch") keys.current?.focus({ preventScroll: true });
  };
  const onKeyDown = (e: ReactKeyboardEvent<HTMLTextAreaElement>) => {
    if (e.nativeEvent.isComposing) return;
    if (SPECIAL.has(e.key)) {
      e.preventDefault();
      flush();
      if (session?.can.key) send({ kind: "key", key: e.key });
      return;
    }
    if ((e.ctrlKey || e.metaKey) && e.key.length === 1 && e.key.toLowerCase() !== "v") {
      e.preventDefault();
      flush();
      if (session?.can.key) send({ kind: "key", key: `${e.ctrlKey ? "Control" : "Meta"}+${e.key}` });
    }
  };
  // Text arrives through the field's input (a physical keyboard, a phone's, a paste): batched, then cleared.
  const onInput = (v: string) => {
    if (session?.can.type) typed(v);
    if (keys.current) keys.current.value = "";
  };

  const finish = async (signedIn: boolean) => {
    flush();
    const s = live.current;
    if (!s || over.current) return onClose(false);
    setBusy(true);
    try {
      await queue.current;
      await api.endHands(slug, s.hands, s.token, signedIn);
    } catch {
      /* already over */
    }
    over.current = true;
    onClose(signedIn);
  };
  const go = () => {
    const u = url.trim();
    if (!u || !session?.can.navigate) return;
    flush();
    send({ kind: "navigate", url: /^[a-z]+:\/\//i.test(u) ? u : `https://${u}` });
    setEditingUrl(false);
  };

  const someoneElse = ended != null && /someone else/i.test(ended);
  return createPortal(
    <motion.div className="lib-backdrop hd-back" initial={{ opacity: 0 }} animate={{ opacity: 1 }} transition={{ duration: 0.18 }}>
      <motion.div className="hd" role="dialog" aria-label="Live view" initial={{ opacity: 0, y: 10 }} animate={{ opacity: 1, y: 0 }} transition={{ duration: 0.2 }}>
        <header className="hd-head">
          <div className="min0">
            <p className="cap">Live view</p>
            <h2>{name ? `Signing in to ${name}` : "Take the controls"}</h2>
          </div>
          <button className="btn t sm" onClick={() => void finish(false)} aria-label="Close">
            <X size={14} />
          </button>
        </header>

        {session?.can.navigate && !ended && (
          <form className="hd-url" onSubmit={(e) => (e.preventDefault(), go())}>
            <Globe size={13} />
            <input className="input mono" value={url} placeholder="An address on the site" onFocus={() => setEditingUrl(true)} onBlur={() => setEditingUrl(false)} onChange={(e) => setUrl(e.target.value)} aria-label="Address" />
            <button className="btn g sm" type="submit" disabled={!url.trim()}>
              Go
            </button>
          </form>
        )}

        <div className={`hd-stage ${focused ? "on" : ""}`}>
          {ended != null ? (
            <div className="hd-wait">
              <p className="t">{someoneElse ? "Someone else took over" : "The session ended"}</p>
              <p className="muted sm">{someoneElse ? "The controls were taken in another window. Close this one." : ended ? `${ended.charAt(0).toUpperCase()}${ended.slice(1)}.` : "Nothing more to do here."}</p>
            </div>
          ) : error && !session ? (
            <div className="hd-wait">
              <p className="t">The live view could not start</p>
              <p className="err sm">{error}</p>
            </div>
          ) : frame ? (
            <>
              <img ref={img} src={frame} alt="What the browser shows now" draggable={false} onPointerUp={click} />
              <textarea
                ref={keys}
                className="hd-keys"
                aria-label="Keys for the page"
                autoCapitalize="off"
                autoCorrect="off"
                spellCheck={false}
                onFocus={() => setFocused(true)}
                onBlur={() => (flush(), setFocused(false))}
                onKeyDown={onKeyDown}
                onChange={(e) => onInput(e.target.value)}
              />
            </>
          ) : (
            <div className="hd-wait">
              <Loader2 size={18} className="spin" />
              <p className="muted sm">{session ? "Waiting for the first picture…" : "Opening the browser…"}</p>
            </div>
          )}
        </div>

        {frame && !ended && (
          <p className="hd-tip muted sm">
            {focused ? (
              <>
                <Keyboard size={12} /> Your keys go to the page. Click outside the picture to stop.
              </>
            ) : (
              <>
                <MousePointerClick size={12} /> Click the picture to click there; then type.
                <button className="lnk-btn" onClick={() => keys.current?.focus({ preventScroll: true })}>
                  <Keyboard size={12} /> Keyboard
                </button>
              </>
            )}
          </p>
        )}
        {error && session && <p className="err sm">{error}</p>}

        <footer className="hd-acts">
          {ended == null ? (
            <>
              <button className="btn p" disabled={busy || !session} onClick={() => void finish(true)}>
                {busy ? <Loader2 size={14} className="spin" /> : <Check size={14} />} I'm signed in
              </button>
              <button className="btn t" disabled={busy} onClick={() => void finish(false)}>
                Done
              </button>
            </>
          ) : (
            <button className="btn g" onClick={() => onClose(false)}>
              Close
            </button>
          )}
          <span className="grow" />
          <span className="dim sm">What you type goes straight to the page and is never kept.</span>
        </footer>
      </motion.div>
    </motion.div>,
    document.body,
  );
}
