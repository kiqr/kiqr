import { useEffect, useMemo, useRef, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { api, useAsync, useEvents, type KiqrEvent } from "../api";
import "../crawl.css";

type PageState = "queued" | "live" | "read" | "thinking" | "parsed" | "blocked" | "failed";
type Page = { url: string; state: PageState; meta: string; order: number };
type Fact = { id: number; url: string; section: string; field: string; value: string; quote: string; at: number };

const SECTIONS: [string, string][] = [
  ["company", "Company"],
  ["products", "Products & services"],
  ["pricing", "Pricing"],
  ["audience", "Audience"],
  ["positioning", "Positioning"],
  ["voice", "Brand voice"],
  ["competitors", "Competitors"],
  ["seo", "SEO"],
  ["channels", "Channels"],
  ["content", "Content"],
  ["integrations", "Integrations"],
  ["trust", "Trust"],
];
const HINTS: Record<string, string> = {
  company: "who they are",
  products: "what they sell",
  pricing: "plans and prices",
  audience: "who it's for",
  positioning: "why them",
  voice: "how they write",
  competitors: "who they're against",
  seo: "what they rank for",
  channels: "where they talk",
  content: "what they've published",
  integrations: "what it works with",
  trust: "proof and logos",
};

function pathOf(u: string) {
  try {
    const x = new URL(u);
    return x.pathname === "/" ? "/" : x.pathname.replace(/\/$/, "");
  } catch {
    return u;
  }
}

export function CrawlPage() {
  const { slug = "" } = useParams();
  const project = useAsync(() => api.project(slug), [slug]);
  const [pages, setPages] = useState<Map<string, Page>>(new Map());
  const [facts, setFacts] = useState<Fact[]>([]);
  const [waiting, setWaiting] = useState(0);
  const [started, setStarted] = useState<{ max: number; concurrency: number; ai: number } | null>(null);
  const [done, setDone] = useState<{ pages: number; blocked: number; failed: number } | null>(null);
  const [showPages, setShowPages] = useState(false);
  const [lit, setLit] = useState<Record<string, number>>({});
  const order = useRef(0);
  const factId = useRef(0);

  const upsert = (url: string, patch: Partial<Page>) =>
    setPages((m) => {
      const n = new Map(m);
      const prev = n.get(url);
      n.set(url, prev ? { ...prev, ...patch } : { url, state: "queued", meta: "", order: order.current++, ...patch });
      return n;
    });

  useEvents(slug, (e: KiqrEvent) => {
    const url = e.url as string | undefined;
    switch (e.type) {
      case "crawl_started":
        setPages(new Map());
        setFacts([]);
        setWaiting(0);
        setDone(null);
        setLit({});
        order.current = 0;
        setStarted({ max: e.max_pages as number, concurrency: e.concurrency as number, ai: e.ai_concurrency as number });
        break;
      case "page_fetching":
        upsert(url!, { state: "live", meta: e.via === "unblockingapi" ? "via UnblockingAPI" : "reading" });
        break;
      case "page_fetched":
        upsert(url!, { state: "read", meta: `${e.ms} ms` });
        break;
      case "page_queued":
        setWaiting(e.waiting as number);
        upsert(url!, { state: "thinking", meta: "with Seemo" });
        break;
      case "page_parsed": {
        const n = e.facts as number;
        setWaiting(e.waiting as number);
        upsert(url!, { state: "parsed", meta: `${n} fact${n === 1 ? "" : "s"}` });
        const preview = (e.preview as Omit<Fact, "id" | "url" | "at">[]) ?? [];
        const now = Date.now();
        const rows = preview.map((p, i) => ({ ...p, id: factId.current++, url: url!, at: now + i * 350 }));
        setFacts((f) => [...rows, ...f]);
        const touched: Record<string, number> = {};
        rows.forEach((r) => (touched[r.section] = now));
        setLit((l) => ({ ...l, ...touched }));
        break;
      }
      case "page_blocked":
        upsert(url!, { state: "blocked", meta: e.reason as string });
        break;
      case "page_failed":
        upsert(url!, { state: "failed", meta: "failed" });
        break;
      case "crawl_finished":
        setDone({ pages: e.pages as number, blocked: e.blocked as number, failed: e.failed as number });
        break;
    }
  });

  // Sections stay lit for a moment after a fact lands.
  const [, tick] = useState(0);
  useEffect(() => {
    const t = setInterval(() => tick((x) => x + 1), 400);
    return () => clearInterval(t);
  }, []);

  const list = useMemo(() => [...pages.values()].sort((a, b) => a.order - b.order), [pages]);
  const read = list.filter((p) => p.state === "read" || p.state === "thinking" || p.state === "parsed").length;
  const blocked = list.filter((p) => p.state === "blocked" || p.state === "failed").length;
  const host = project.data?.name ?? slug;
  const running = started !== null && done === null;
  const seemoBusy = waiting > 0 || list.some((p) => p.state === "thinking");
  const bySection = useMemo(() => {
    const m: Record<string, Fact[]> = {};
    for (const f of facts) (m[f.section] ??= []).push(f);
    return m;
  }, [facts]);
  const pct = started ? Math.min(100, Math.round((read / started.max) * 100)) : 0;
  const latestFact = facts[0];

  const say = done
    ? `Done. I read ${done.pages} pages and found ${facts.length} facts${done.blocked ? `; ${done.blocked} ${done.blocked === 1 ? "page" : "pages"} wouldn't load` : ""}.`
    : seemoBusy && latestFact
      ? `Reading ${pathOf(latestFact.url)} — ${HINTS[latestFact.section] ?? latestFact.section}.`
      : running
        ? "Fetching pages. I start reading as soon as the first one lands."
        : "Ready when you are.";

  return (
    <div>
      <div className="crawl-head">
        <h1 className="d2">{done ? `What I learned about ${host}` : running ? `Learning about ${host}` : `Crawl ${host}`}</h1>
        {running && (
          <span className="chip acc live">
            <b />
            live
          </span>
        )}
        {done && (
          <span className="chip ok">
            <b />
            done
          </span>
        )}
        <span className="sp" />
        {!running && project.data && (
          <button className="btn g" onClick={() => api.crawl(slug)}>
            {done || project.data.last_crawl ? "Crawl again" : "Start crawl"}
          </button>
        )}
        {done && (
          <Link to={`/p/${slug}`} className="btn p">
            Open profile
          </Link>
        )}
      </div>

      <div className={`strip ${seemoBusy ? "busy" : ""}`}>
        <span className="av" />
        <div className="say">
          <b>Seemo</b> · {say}
          {started && !done && (
            <small>
              {started.concurrency} pages fetched in parallel, {started.ai} read by Seemo at a time
              {waiting > 0 && ` · ${waiting} waiting`}
            </small>
          )}
        </div>
        <div className="nums">
          <div className={running ? "hot" : ""}>
            <div className="n">
              <Count n={read} />
            </div>
            <div className="l">{started ? `of ${started.max} pages` : "pages"}</div>
          </div>
          <div className={seemoBusy ? "hot" : ""}>
            <div className="n">
              <Count n={facts.length} />
            </div>
            <div className="l">facts</div>
          </div>
          <div>
            <div className="n">{blocked}</div>
            <div className="l">blocked</div>
          </div>
        </div>
        <div className={`bar ${running ? "live" : ""}`}>
          <i style={{ width: `${done ? 100 : pct}%` }} />
        </div>
      </div>

      {!started && !done && list.length === 0 ? (
        <div className="big">
          <h2 className="d2">{project.data?.last_crawl ? "The last crawl is in the profile" : "Nothing read yet"}</h2>
          <p className="muted">Start a crawl to watch the profile build itself.</p>
        </div>
      ) : (
        <div className="grid">
          {SECTIONS.map(([id, title]) => {
            const rows = bySection[id] ?? [];
            const isLit = lit[id] !== undefined && Date.now() - lit[id] < 1800;
            return (
              <div key={id} className={`sec ${rows.length === 0 ? "empty" : ""} ${isLit ? "lit" : ""} ${done ? "done" : ""}`}>
                <h3>
                  {title}
                  {rows.length > 0 && <span className={`chip ${isLit ? "acc" : ""}`}>{rows.length}</span>}
                </h3>
                {rows.length === 0 ? (
                  <div className="hint">
                    {running ? <i /> : null}
                    {running ? `listening for ${HINTS[id]}` : `nothing yet about ${HINTS[id]}`}
                  </div>
                ) : (
                  <>
                    <ul>
                      {rows.slice(0, 3).map((f) => (
                        <li key={f.id} className={Date.now() - f.at < 2500 ? "new" : ""}>
                          <div className="f">
                            {f.field.replace(/_/g, " ")}
                            <code>{pathOf(f.url)}</code>
                          </div>
                          <div className="v">
                            <Typed text={f.value} startAt={f.at} />
                          </div>
                        </li>
                      ))}
                    </ul>
                    {rows.length > 3 && <div className="more">+{rows.length - 3} more in the profile</div>}
                  </>
                )}
              </div>
            );
          })}
        </div>
      )}

      {done && done.blocked > 0 && (
        <p className="muted" style={{ marginBottom: 18 }}>
          {done.blocked} {done.blocked === 1 ? "page was" : "pages were"} blocked or need JavaScript — connect UnblockingAPI in <Link to="/connections">connections</Link> to read them too.
        </p>
      )}

      {list.length > 0 && (
        <div className="ticker">
          <span className="lbl">Pages</span>
          <div className="lane">
            {[...list].reverse().slice(0, 12).map((p) => (
              <span key={p.url} className={`pg ${p.state}`} title={`${p.url} — ${p.meta}`}>
                <i />
                {pathOf(p.url)}
              </span>
            ))}
          </div>
          <button className="btn t toggle" onClick={() => setShowPages((s) => !s)}>
            {showPages ? "Hide" : `All ${list.length}`}
          </button>
        </div>
      )}
      {showPages && (
        <div className="pages">
          {[...list].reverse().map((p) => (
            <div key={p.url} className={`row ${p.state}`}>
              <span className="st" />
              <span className="u" title={p.url}>
                {pathOf(p.url)}
              </span>
              <span className="m">{p.meta}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function Count({ n }: { n: number }) {
  const [shown, setShown] = useState(n);
  useEffect(() => {
    if (shown === n) return;
    const step = Math.max(1, Math.ceil(Math.abs(n - shown) / 6));
    const t = setTimeout(() => setShown((s) => (s < n ? Math.min(n, s + step) : Math.max(n, s - step))), 40);
    return () => clearTimeout(t);
  }, [n, shown]);
  return <>{shown}</>;
}

/** Types the text in, starting at `startAt`, ~40 characters per second. */
function Typed({ text, startAt }: { text: string; startAt: number }) {
  const [len, setLen] = useState(() => (Date.now() - startAt > 3000 ? text.length : 0));
  useEffect(() => {
    if (len >= text.length) return;
    const delay = Math.max(0, startAt - Date.now());
    const t = setTimeout(() => setLen((l) => Math.min(text.length, l + 2)), len === 0 ? delay : 25);
    return () => clearTimeout(t);
  }, [len, text, startAt]);
  const typing = len < text.length;
  return (
    <>
      {text.slice(0, len)}
      {typing && <span className="cur" />}
    </>
  );
}
