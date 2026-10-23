// "Raise the limit": change how many pages the crawl reads, applied to the running crawl at once.
import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { AnimatePresence, motion } from "motion/react";
import { Gauge, X } from "lucide-react";
import { api } from "../api";
import { fmt } from "./tickers";

const MAX = 5000;

export function LimitDialog({
  open,
  slug,
  host,
  found,
  current,
  running,
  onClose,
  onApplied,
  onRecrawl,
}: {
  open: boolean;
  slug: string;
  host: string;
  found: number;
  current: number;
  running: boolean;
  onClose: () => void;
  /** The new limit, and whether the running crawl picked it up. */
  onApplied: (maxPages: number, live: boolean) => void;
  onRecrawl: () => void;
}) {
  const [value, setValue] = useState(String(suggest(found, current)));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState<number | null>(null);
  const input = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!open) return;
    setValue(String(suggest(found, current)));
    setError(null);
    setSaved(null);
    setBusy(false);
    setTimeout(() => input.current?.select(), 30);
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // Only when it opens: the counts keep moving while it is up.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  const n = Math.round(Number(value));
  const valid = Number.isFinite(n) && n >= 1 && n <= MAX;
  const all = Math.min(found, MAX);
  const choices = [...new Set([all, Math.min(current * 2, MAX), 500, 1000])].filter((c) => c > current).sort((a, b) => a - b).slice(0, 4);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!valid) return setError(`Choose a number from 1 to ${fmt.format(MAX)}.`);
    setBusy(true);
    setError(null);
    try {
      const r = await api.setCrawlLimit(slug, n);
      onApplied(r.max_pages, r.live);
      if (r.live) onClose();
      else setSaved(r.max_pages);
    } catch (err) {
      setError((err as Error).message);
    } finally {
      setBusy(false);
    }
  };

  // On <body>: the page's entry animation uses a transform, which would pin a fixed overlay to it.
  return createPortal(
    <AnimatePresence>
      {open && (
        <motion.div className="lim-back" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} transition={{ duration: 0.15 }} onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
          <motion.div
            className="lim"
            role="dialog"
            aria-modal="true"
            aria-labelledby="lim-t"
            initial={{ opacity: 0, y: 12, scale: 0.97 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: 8, scale: 0.98 }}
            transition={{ duration: 0.22, ease: [0.2, 0.8, 0.2, 1] }}
          >
            <button className="lim-x" onClick={onClose} aria-label="Close">
              <X size={16} />
            </button>
            <span className="lim-ic">
              <Gauge size={18} />
            </span>
            <h2 id="lim-t">Raise the page limit</h2>
            <p className="lim-lead">
              Seemo has found <b>{fmt.format(found)}</b> pages on {host} so far. {running ? "This crawl" : "The last crawl"} reads up to <b>{fmt.format(current)}</b>.
            </p>

            {saved === null ? (
              <form onSubmit={submit}>
                <label className="lim-field">
                  <span>Pages to read</span>
                  <input ref={input} className="input" type="number" min={1} max={MAX} step={10} value={value} onChange={(e) => setValue(e.target.value)} />
                </label>
                {choices.length > 0 && (
                  <div className="lim-choices">
                    {choices.map((c) => (
                      <button type="button" key={c} className={`chip ${n === c ? "acc" : ""}`} onClick={() => setValue(String(c))}>
                        {c === all ? `All found · ${fmt.format(c)}` : fmt.format(c)}
                      </button>
                    ))}
                  </div>
                )}
                <p className="lim-note">
                  {running ? "The running crawl picks it up at once and carries on with the pages it has queued. " : ""}The limit is saved for future crawls of this project. More pages take longer and use more model time.
                </p>
                {error && <p className="lim-err">{error}</p>}
                <div className="lim-acts">
                  <button type="button" className="btn t" onClick={onClose}>
                    Cancel
                  </button>
                  <button className="btn p" disabled={busy || !valid}>
                    {busy ? "Saving…" : valid ? `Read up to ${fmt.format(n)} pages` : "Read more pages"}
                  </button>
                </div>
              </form>
            ) : (
              <>
                <p className="lim-note">This crawl had already finished fetching pages, so the new limit of {fmt.format(saved)} applies from the next crawl.</p>
                <div className="lim-acts">
                  <button className="btn t" onClick={onClose}>
                    Close
                  </button>
                  <button
                    className="btn p"
                    onClick={() => {
                      onClose();
                      onRecrawl();
                    }}
                  >
                    Crawl again
                  </button>
                </div>
              </>
            )}
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>,
    document.body,
  );
}

/** Enough to cover everything found so far. */
function suggest(found: number, current: number) {
  return Math.min(Math.max(found, current + 10), MAX);
}
