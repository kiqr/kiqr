// The live crawl screen. A port of ContentWriter's crawl-live.tsx onto KIQR's project event stream.
import { useEffect, useState, useSyncExternalStore, type ReactNode } from "react";
import { Link } from "react-router-dom";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { ArrowRight, Radio, RotateCcw } from "lucide-react";
import { api, useAsync, useEvents, type KiqrEvent } from "../api";
import { CrawlStore, pathOf } from "./store";
import { SiteGraph, type HoverInfo } from "./site-graph";
import { ProgressRing, Ticker, fmt } from "./tickers";
import { PhaseTimeline } from "./phases";
import { NowFetching, UrlFeed } from "./feed";
import { Discoveries } from "./discoveries";
import { LearningPanel } from "./learning";
import "../crawl.css";

/** If synthesis never announces itself after the crawl, stop waiting for it. */
const SETTLE_MS = 12000;

export function CrawlLive({ slug, fresh }: { slug: string; fresh: boolean }) {
  const project = useAsync(() => api.project(slug), [slug]);
  const health = useAsync(() => api.health(), []);
  // One store per mount; the page is keyed by slug.
  const [store] = useState(() => new CrawlStore("", 200));
  useSyncExternalStore(store.subscribe, store.getVersion, store.getVersion);
  useEvents(slug, (e: KiqrEvent) => store.apply(e));
  const reducedMotion = useReducedMotion() ?? false;
  const [hover, setHover] = useState<HoverInfo | null>(null);
  const [starting, setStarting] = useState(false);
  const [startError, setStartError] = useState<string | null>(null);

  const p = project.data;
  useEffect(() => {
    if (p) store.configure(p.url, p.crawl.max_pages ?? 200);
  }, [p, store]);

  const s = store.state;
  const running = s.live && s.phase !== "done";

  // Keep the elapsed clock moving while a crawl runs.
  useEffect(() => {
    if (!running || s.completed) return;
    const t = setInterval(() => store.touch(), 250);
    return () => clearInterval(t);
  }, [running, s.completed, store]);

  // Older daemons (or a crawl with nothing to synthesise) never send synthesis events.
  const waitingForSynthesis = s.phase === "learning" && !s.analysis.started;
  useEffect(() => {
    if (!waitingForSynthesis) return;
    const t = setTimeout(() => store.settle(), SETTLE_MS);
    return () => clearTimeout(t);
  }, [waitingForSynthesis, store]);

  const start = () => {
    setStartError(null);
    setStarting(true);
    store.reset();
    store.touch();
    api.crawl(slug).catch((err: Error) => {
      setStarting(false);
      setStartError(err.message);
    });
  };

  const host = safeHost(s.siteUrl || p?.url || "") || p?.name || slug;
  const showLive = s.live || fresh || starting;
  const elapsed = s.startedAt ? (s.completed?.durationMs ?? Math.max(0, Date.now() - s.startedAt)) / 1000 : 0;

  if (!showLive) {
    return (
      <div className="cr">
        <Header host={host} label="Crawl" live={false} />
        <div className="cr-empty bg-grid">
          <div className="cr-empty-in">
            <span className="cr-radar">
              <span />
            </span>
            <h2>{p?.last_crawl ? "No crawl running" : "Nothing crawled yet"}</h2>
            <p>
              {p?.last_crawl
                ? `The last crawl read ${fmt.format(p.last_crawl.pages)} pages on ${new Date(p.last_crawl.finished_at).toLocaleDateString()}. Its findings are in the business profile.`
                : `Start a crawl to map ${host} and watch the business profile build itself.`}
            </p>
            <div className="cr-actions">
              <button className="btn p" onClick={start} disabled={!p}>
                Start crawl
              </button>
              {p?.last_crawl && (
                <Link to={`/p/${slug}`} className="btn g">
                  Business profile
                </Link>
              )}
            </div>
            {startError && <p className="cr-err">{startError}</p>}
          </div>
        </div>
      </div>
    );
  }

  const tipLeft = hover ? (hover.x + 280 > hover.w ? Math.max(8, hover.x - 270) : hover.x + 14) : 0;

  return (
    <div className="cr">
      <Header host={host} label="Live crawl" live={running || (!s.live && (fresh || starting))}>
        <PhaseTimeline phase={s.phase} />
      </Header>

      <div className="cr-grid">
        <section className="cr-graph">
          <div className="bg-grid cr-graph-grid" />
          <SiteGraph key={slug} store={store} reducedMotion={reducedMotion} onHover={setHover} />
          <Legend />
          <div className="cr-now-box">
            <NowFetching state={s} />
          </div>
          <AnimatePresence>
            {hover && (
              <motion.div
                key={hover.node.id}
                initial={{ opacity: 0, scale: 0.95 }}
                animate={{ opacity: 1, scale: 1 }}
                exit={{ opacity: 0 }}
                transition={{ duration: 0.12 }}
                className="cr-tip"
                style={{ left: tipLeft, top: hover.y + 14 }}
              >
                <Tip hover={hover} />
              </motion.div>
            )}
          </AnimatePresence>
          <AnimatePresence>
            {s.nodes.size === 0 && (
              <motion.div exit={{ opacity: 0 }} className="cr-connecting">
                <span className="cr-ping">
                  <span />
                </span>
                Connecting to {host}…
              </motion.div>
            )}
          </AnimatePresence>
        </section>

        <aside className="cr-side">
          <div className="cr-card">
            <div className="cr-stats">
              <ProgressRing value={s.stats.crawled} max={s.maxPages} phase={s.phase} />
              <dl>
                <Stat label="Found" value={s.stats.found} />
                <Stat label="Queue" value={s.stats.queued} />
                <Stat label="Avg resp." value={s.stats.avgMs} suffix="ms" />
                <Stat label="Elapsed" value={elapsed} format={(v) => `${v.toFixed(1)}s`} />
                <Stat label="Errors" value={s.stats.errors} tone={s.stats.errors ? "bad" : undefined} />
                <Stat label="Blocked" value={s.stats.blocked} tone={s.stats.blocked ? "warn" : undefined} />
              </dl>
            </div>
            <div className="cr-words">
              <p>Words indexed</p>
              <Ticker value={s.stats.words} className="text-gradient" />
            </div>
            {s.stats.found > s.maxPages && (
              <motion.div initial={reducedMotion ? false : { opacity: 0, height: 0 }} animate={{ opacity: 1, height: "auto" }} className="cr-quota">
                <div className="cr-quota-bar">
                  <i style={{ width: `${Math.min((s.maxPages / s.stats.found) * 100, 100)}%` }} />
                </div>
                <p>
                  Found <b>{fmt.format(s.stats.found)}</b> URLs — this project reads up to {fmt.format(s.maxPages)}.{" "}
                  <Link to={`/p/${slug}/settings`}>Raise the limit</Link>
                </p>
              </motion.div>
            )}
          </div>
          <Discoveries state={s} reducedMotion={reducedMotion} />
          <div className="cr-card cr-activity">
            <p className="cr-activity-h">
              Activity <span>{fmt.format(s.stats.crawled)} pages</span>
            </p>
            <UrlFeed state={s} reducedMotion={reducedMotion} />
          </div>
        </aside>
      </div>

      <LearningPanel state={s} reducedMotion={reducedMotion} />

      <AnimatePresence>
        {s.completed && (
          <motion.div initial={reducedMotion ? false : { opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} className="cr-done">
            <div>
              <p className="cr-done-t">
                Crawled {fmt.format(s.completed.pages)} pages in {(s.completed.durationMs / 1000).toFixed(1)}s
              </p>
              <p className="cr-done-s">
                {s.phase === "done" ? "The business profile is up to date." : "Now writing your business profile…"}
              </p>
              {s.completed.blocked > 0 && health.data && !health.data.unblockingapi && (
                <p className="cr-done-s warn">
                  {s.completed.blocked} {s.completed.blocked === 1 ? "page was" : "pages were"} blocked or need JavaScript. Connect UnblockingAPI in{" "}
                  <Link to="/connections">Connections</Link> to read them too.
                </p>
              )}
            </div>
            <div className="cr-actions">
              {s.phase === "done" && (
                <button className="btn g" onClick={start}>
                  <RotateCcw size={14} /> Crawl again
                </button>
              )}
              <Link to={`/p/${slug}`} className={`btn ${s.phase === "done" ? "p" : "g"}`}>
                Business profile <ArrowRight size={16} />
              </Link>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

function Header({ host, label, live, children }: { host: string; label: string; live: boolean; children?: ReactNode }) {
  return (
    <div className="cr-head">
      <div>
        <p className="cr-label">{label}</p>
        <h1 className="cr-host">
          {host}
          {live && (
            <span className="cr-live">
              <Radio size={10} /> live
            </span>
          )}
        </h1>
      </div>
      {children}
    </div>
  );
}

function Stat({ label, value, suffix, tone, format }: { label: string; value: number; suffix?: string; tone?: "bad" | "warn"; format?: (v: number) => string }) {
  return (
    <div>
      <dt>{label}</dt>
      <dd className={tone ?? ""}>
        <Ticker value={value} format={format} />
        {suffix && <span className="sfx">{suffix}</span>}
      </dd>
    </div>
  );
}

function Legend() {
  const items = [
    ["#56627c", "queued"],
    ["#5eead4", "fetching"],
    ["#a78bfa", "UnblockingAPI"],
    ["#34d399", "200"],
    ["#fbbf24", "blocked"],
    ["#f87171", "error"],
    ["#818cf8", "AI reading"],
  ];
  return (
    <div className="cr-legend">
      {items.map(([c, l]) => (
        <span key={l}>
          <i style={l === "AI reading" ? { background: "transparent", boxShadow: `inset 0 0 0 1.5px ${c}` } : { background: c }} /> {l}
        </span>
      ))}
    </div>
  );
}

function Tip({ hover }: { hover: HoverInfo }) {
  const n = hover.node;
  return (
    <>
      <p className="cr-tip-t">{n.title || pathOf(n.id)}</p>
      <p className="cr-tip-u">{n.id}</p>
      <div className="cr-tip-m">
        <span>status {n.httpStatus ?? n.status}</span>
        {n.words !== undefined && <span>{fmt.format(n.words)} words</span>}
        {n.ms !== undefined && <span>{n.ms}ms</span>}
        <span>depth {n.depth}</span>
        <span>{n.inbound} linked</span>
        {n.via === "unblockingapi" && <span>via UnblockingAPI</span>}
      </div>
      {n.facts !== undefined && <p className="cr-tip-f">{n.facts === 0 ? "No facts found" : `${n.facts} ${n.facts === 1 ? "fact" : "facts"} for the profile`}</p>}
      {n.reading && <p className="cr-tip-f">Being read by AI…</p>}
      {n.reason && <p className="cr-tip-w">{n.reason}</p>}
    </>
  );
}

function safeHost(url: string) {
  if (!url) return "";
  try {
    return new URL(url).hostname.replace(/^www\./, "");
  } catch {
    return url;
  }
}
