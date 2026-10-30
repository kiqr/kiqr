// The welcome flow: a new project's first build, start to finish, with the
// rest of the app locked until the business profile is ready.
//   Act 1: Seemo reads the site. One overall progress, what it learns as it
//          goes, the brand as soon as it is found, and four quick questions.
//   Act 2: the reveal: the business as Seemo sees it, confirmed or fixed.
//   Act 3: the first week: the plan and three posts in the brand.
import { useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import { Link, useParams } from "react-router-dom";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { ArrowRight, Bell, BellRing, Hourglass, Play, RotateCcw, TriangleAlert } from "lucide-react";
import { api, useAsync, useEvents, type Brand, type Brief, type Build, type KiqrEvent } from "../api";
import { DEMO } from "../demo";
import { CrawlStore, sectionTitle } from "../crawl/store";
import { SiteGraph } from "../crawl/site-graph";
import { BuildRing, STAGE_WORDS, StageSteps, etaWords } from "./BuildRing";
import { Intake } from "./Intake";
import { Reveal } from "./Reveal";
import { FirstWeek } from "./FirstWeek";
import "../crawl.css";
import "../welcome.css";

/** Without word for this long, say it is still working rather than leave a frozen screen. */
const QUIET_MS = 45000;

type Act = "build" | "done" | "reveal" | "week";

export function Welcome() {
  const { slug = "" } = useParams();
  const reduced = useReducedMotion() ?? false;
  const project = useAsync(() => api.project(slug), [slug]);
  const [build, setBuild] = useState<Build | null>(null);
  const [act, setAct] = useState<Act>("build");
  const [brand, setBrand] = useState<Brand | null>(null);
  const [brief, setBrief] = useState<Brief | null>(null);
  const [now, setNow] = useState(Date.now());
  const [notify, setNotify] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const sawRunning = useRef(false);

  // The same stream the crawl screen uses, replayed first: the graph and what Seemo learned.
  const [store] = useState(() => new CrawlStore("", 200));
  useSyncExternalStore(store.subscribe, store.getVersion, store.getVersion);
  const replayed = useRef<KiqrEvent[]>([]);
  const [stream, setStream] = useState(0);
  useEvents(
    slug,
    (e: KiqrEvent) => {
      if (e.replay) return void replayed.current.push(e);
      if (e.type === "replay_done") {
        store.replay(replayed.current, e.running === true);
        replayed.current = [];
        return;
      }
      store.apply(e);
      if (e.type === "build_progress") setBuild((b) => (b ? { ...b, pct: Number(e.pct), stage: e.stage as Build["stage"], eta_secs: (e.eta_secs as number | null) ?? null, last_at: e.at, running: e.stage !== "ready" } : b));
      if (e.type === "build_stage") api.build(slug).then(setBuild, () => {});
      if (e.type === "brand_updated") api.brand(slug).then(setBrand);
      if (e.type === "crawl_started") api.build(slug).then(setBuild, () => {});
    },
    { replay: true, key: stream },
  );

  const p = project.data;
  useEffect(() => {
    if (!p) return;
    store.configure(p.url, p.crawl.max_pages ?? 200);
    if (p.build) setBuild(p.build);
    api.brand(slug).then(setBrand);
  }, [p, store, slug]);

  // The demo has no daemon: its build is read off the scripted crawl.
  const shown: Build | null = DEMO ? demoBuild(store) : build;
  const stage = shown?.stage ?? "new";
  if (shown?.running) sawRunning.current = true;

  // Ready: a short moment for the finished ring when it happened live, then the reveal.
  useEffect(() => {
    if (stage !== "ready" || act !== "build") return;
    if (!sawRunning.current || reduced) return setAct("reveal");
    setAct("done");
    if (notify && document.hidden && "Notification" in window) new Notification("Seemo has read your site", { body: "Your business profile is ready." });
  }, [stage, act, reduced, notify]);
  // Its own effect: had the timer lived in the one above, moving to "done" would have cancelled it.
  useEffect(() => {
    if (act !== "done") return;
    const t = setTimeout(() => setAct("reveal"), 1800);
    return () => clearTimeout(t);
  }, [act]);
  // The brief, once the build is ready; asked again until it comes (it is saved just before "ready").
  useEffect(() => {
    if (stage !== "ready" || brief) return;
    let stop = false;
    const load = () =>
      api.brief(slug).then(
        (b) => (b ? setBrief(b) : !stop && setTimeout(load, 1500)),
        () => !stop && setTimeout(load, 1500),
      );
    load();
    return () => {
      stop = true;
    };
  }, [stage, brief, slug]);

  // A clock for "still working".
  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), 5000);
    return () => clearInterval(t);
  }, []);
  const quiet = shown?.running && shown.last_at ? now - new Date(shown.last_at).getTime() > QUIET_MS : false;

  const s = store.state;
  const learned = useMemo(() => {
    const seen = new Set<string>();
    const out: { key: string; label: string; value: string }[] = [];
    for (const r of s.analysis.recent) {
      for (const f of r.facts) {
        const v = f.value.trim();
        if (!v || seen.has(v.toLowerCase())) continue;
        seen.add(v.toLowerCase());
        out.push({ key: `${f.section}:${v}`, label: sectionTitle(f.section), value: v });
      }
    }
    return out.slice(0, 6);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [s.analysis.recent]);
  const audienceHint = s.analysis.recent.flatMap((r) => r.facts).find((f) => f.section === "audience")?.value;

  const host = (p?.url ?? "").replace(/^https?:\/\//, "").replace(/\/$/, "") || slug;

  const start = async (what: "crawl" | "profile") => {
    setBusy(true);
    setError(null);
    store.reset();
    try {
      await (what === "crawl" ? api.crawl(slug) : api.rebuildBrief(slug));
      setStream((n) => n + 1);
      setBuild(await api.build(slug));
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };

  const askNotify = async () => {
    if (!("Notification" in window)) return;
    const r = await Notification.requestPermission();
    setNotify(r === "granted");
  };

  if (project.error) {
    return (
      <div className="wl wl-center">
        <p className="muted">There is no project called {slug}.</p>
        <Link className="btn g" to="/new">
          Add a website
        </Link>
      </div>
    );
  }
  if (!shown || !p) return <div className="wl" />;

  if (act === "reveal" && brief) {
    return (
      <div className="wl">
        <Reveal slug={slug} brief={brief} brand={brand} blocked={s.stats.blocked || p.last_crawl?.blocked || 0} onSaved={setBrief} onNext={() => setAct("week")} />
      </div>
    );
  }
  if (act === "week") {
    return (
      <div className="wl">
        <FirstWeek slug={slug} brand={brand} />
      </div>
    );
  }

  return (
    <div className="wl">
      <header className="wl-head">
        <span className="av" />
        <div>
          <p className="cap">Seemo · {host}</p>
          <h1 className="d2">{stage === "new" ? "Ready when you are" : stage === "failed" ? "I had to stop" : act === "done" ? "Done. I know your business now." : `I'm reading ${host}`}</h1>
        </div>
      </header>

      <div className="wl-grid">
        <section className="wl-graph">
          <div className="bg-grid cr-graph-grid" />
          <SiteGraph key={`${slug}-${stream}`} store={store} reducedMotion={reduced} onHover={() => {}} />
          <div className="wl-graph-foot mono">
            <span>{s.stats.crawled} pages read</span>
            <span>{s.stats.facts} facts</span>
            {s.stats.blocked > 0 && <span className="warn">{s.stats.blocked} blocked</span>}
          </div>
        </section>

        <section className="wl-center-col">
          <BuildRing build={shown} />
          <div className="wl-stage">
            <h2>{STAGE_WORDS[stage]}</h2>
            {shown.running && <p className="muted">{etaWords(shown.eta_secs)}</p>}
            <AnimatePresence>
              {quiet && (
                <motion.p initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} className="wl-quiet">
                  <Hourglass size={13} /> Still working. The model takes its time on long pages.
                </motion.p>
              )}
            </AnimatePresence>
          </div>
          <StageSteps stage={stage} />

          {stage === "ready" && (
            <div className="wl-act">
              <button className="btn a wl-big" disabled={!brief} onClick={() => setAct("reveal")}>
                {brief ? "See what I found" : "Opening your profile…"} <ArrowRight size={16} />
              </button>
              <Link className="btn t sm" to={`/p/${slug}/todo`} state={{ welcome: true }}>
                Skip to KIQR
              </Link>
            </div>
          )}
          {stage === "new" && (
            <div className="wl-act">
              <p className="muted">I haven't read {host} yet.</p>
              <button className="btn a" disabled={busy} onClick={() => void start("crawl")}>
                <Play size={14} /> Start reading
              </button>
            </div>
          )}
          {stage === "failed" && (
            <div className="wl-problem">
              <p>
                <TriangleAlert size={15} /> {shown.problem ?? "The build ended before the business profile was written."}
              </p>
              <div className="row">
                {((p.last_crawl?.pages ?? 0) > 0 || s.stats.crawled > 0) && (
                  <button className="btn p sm" disabled={busy} onClick={() => void start("profile")}>
                    Write the profile
                  </button>
                )}
                <button className="btn g sm" disabled={busy} onClick={() => void start("crawl")}>
                  <RotateCcw size={13} /> Read the site again
                </button>
                <Link className="btn t sm" to="/settings/models">
                  Check the models
                </Link>
              </div>
            </div>
          )}
          {error && <p className="err sm">{error}</p>}

          {shown.running && (
            <div className="wl-leave">
              {notify ? (
                <span className="dim sm">
                  <BellRing size={13} /> I'll let you know when it's ready.
                </span>
              ) : (
                <button className="btn t sm" onClick={() => void askNotify()}>
                  <Bell size={13} /> You can leave this tab. Tell me when it's ready
                </button>
              )}
            </div>
          )}

          {brand && (brand.colors.length > 0 || brand.logo) && (
            <motion.div initial={reduced ? false : { opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }} className="wl-brandline">
              <span className="cap">Your brand</span>
              {brand.logo && <img src={brand.logo} alt="" onError={(e) => (e.currentTarget.style.display = "none")} />}
              {brand.colors.slice(0, 6).map((c, i) => (
                <motion.i key={c} initial={reduced ? false : { scale: 0 }} animate={{ scale: 1 }} transition={{ delay: 0.1 + i * 0.06 }} style={{ background: c }} title={c} />
              ))}
            </motion.div>
          )}

          <div className="wl-learned">
            <p className="cap">What I've learned so far</p>
            {learned.length === 0 && <p className="dim sm">The first facts show up here as I read.</p>}
            <AnimatePresence initial={false} mode="popLayout">
              {learned.map((f) => (
                <motion.div key={f.key} layout initial={reduced ? false : { opacity: 0, y: -10 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0 }} className="wl-fact">
                  <span className="lb">{f.label}</span>
                  <span>{f.value}</span>
                </motion.div>
              ))}
            </AnimatePresence>
          </div>
        </section>

        <aside className="wl-side">{stage !== "new" && <Intake slug={slug} host={host} brand={brand} suggestion={audienceHint} done={stage === "ready"} />}</aside>
      </div>
    </div>
  );
}

/** The demo's build, from its scripted crawl: the same weights the daemon uses. */
function demoBuild(store: CrawlStore): Build {
  const s = store.state;
  const fetch = s.maxPages ? Math.min(s.stats.crawled / Math.min(s.maxPages, Math.max(s.stats.found, 1)), 1) : 0;
  const read = s.analysis.total ? Math.min(s.analysis.read / s.analysis.total, 1) : 0;
  const sections = s.analysis.sections.length;
  const stage: Build["stage"] = s.phase === "done" ? "ready" : s.analysis.started || s.completed ? "writing" : fetch >= 1 ? "reading" : s.live ? "crawling" : "crawling";
  const pct = stage === "ready" ? 100 : Math.min(99, Math.round(30 * fetch + 35 * (stage === "writing" ? 1 : read) + (stage === "writing" ? Math.min(35, sections * 4) : 0)));
  return { stage, pct, eta_secs: stage === "ready" ? null : Math.max(20, Math.round((100 - pct) * 1.6)), max_pages: s.maxPages, found: s.stats.found, fetched: s.stats.crawled, queued: s.analysis.total, read: s.analysis.read, facts: s.stats.facts, sections_done: sections, sections_total: 12, running: stage !== "ready", started_at: null, last_at: new Date().toISOString(), problem: null };
}
