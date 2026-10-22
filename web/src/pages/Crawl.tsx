import { useEffect, useMemo, useRef, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { api, useAsync, useEvents, type KiqrEvent } from "../api";
import "../crawl.css";

type NodeState = "queued" | "live" | "read" | "thinking" | "parsed" | "blocked" | "failed";
type Node = { url: string; from: string | null; state: NodeState; meta: string; order: number; depth: number; angle: number };
type FactRow = { id: number; url: string; section: string; field: string; value: string; quote: string };

const W = 1000;
const H = 440;

function pathOf(u: string) {
  try {
    const x = new URL(u);
    return x.pathname === "/" ? "/" : x.pathname.replace(/\/$/, "");
  } catch {
    return u;
  }
}
function hash(s: string) {
  let h = 2166136261;
  for (let i = 0; i < s.length; i++) h = Math.imul(h ^ s.charCodeAt(i), 16777619);
  return (h >>> 0) / 4294967295;
}
function depthOf(u: string) {
  const p = pathOf(u);
  return p === "/" ? 0 : Math.min(3, p.split("/").filter(Boolean).length);
}

export function CrawlPage() {
  const { slug = "" } = useParams();
  const project = useAsync(() => api.project(slug), [slug]);
  const [nodes, setNodes] = useState<Map<string, Node>>(new Map());
  const [facts, setFacts] = useState<FactRow[]>([]);
  const [factCount, setFactCount] = useState(0);
  const [waiting, setWaiting] = useState(0);
  const [started, setStarted] = useState<{ max: number; concurrency: number; ai: number } | null>(null);
  const [done, setDone] = useState<{ pages: number; blocked: number; failed: number } | null>(null);
  const order = useRef(0);
  const factId = useRef(0);

  const upsert = (url: string, patch: Partial<Node>, from: string | null = null) =>
    setNodes((m) => {
      const n = new Map(m);
      const prev = n.get(url);
      if (prev) n.set(url, { ...prev, ...patch });
      else {
        const o = order.current++;
        n.set(url, {
          url,
          from: from && from !== "sitemap" ? from : null,
          state: "queued",
          meta: "",
          order: o,
          depth: depthOf(url),
          angle: hash(url) * Math.PI * 2,
          ...patch,
        });
      }
      return n;
    });

  useEvents(slug, (e: KiqrEvent) => {
    const url = e.url as string | undefined;
    switch (e.type) {
      case "crawl_started":
        setNodes(new Map());
        setFacts([]);
        setFactCount(0);
        setWaiting(0);
        setDone(null);
        order.current = 0;
        setStarted({ max: e.max_pages as number, concurrency: e.concurrency as number, ai: e.ai_concurrency as number });
        break;
      case "page_discovered":
        upsert(url!, { meta: "queued" }, (e.from as string | null) ?? null);
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
        setFactCount((c) => c + n);
        const preview = (e.preview as Omit<FactRow, "id" | "url">[]) ?? [];
        setFacts((f) => [...preview.map((p) => ({ ...p, id: factId.current++, url: url! })), ...f].slice(0, 60));
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

  const list = useMemo(() => [...nodes.values()].sort((a, b) => a.order - b.order), [nodes]);
  const read = list.filter((n) => n.state === "read" || n.state === "thinking" || n.state === "parsed").length;
  const blocked = list.filter((n) => n.state === "blocked" || n.state === "failed").length;
  const host = project.data?.name ?? slug;
  const running = started !== null && done === null;
  const seemoBusy = waiting > 0 || list.some((n) => n.state === "thinking");

  return (
    <div>
      <div className="crawl-head">
        <h1 className="d2">{done ? `Read ${host}` : running ? `Reading ${host}` : `Crawl ${host}`}</h1>
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
        {started && !done && (
          <span className="lbl">
            {started.concurrency} fetches, {started.ai} Seemo reads in parallel
          </span>
        )}
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

      <div className="stats">
        <Stat n={read} label={started ? `of ${started.max} pages read` : "pages read"} hot={running} />
        <Stat n={factCount} label="facts found" hot={seemoBusy} />
        <Stat n={waiting} label="waiting for Seemo" hot={waiting > 0} />
        <Stat n={blocked} label="blocked or failed" />
      </div>

      <div className={`map ${done ? "done" : ""}`}>
        {running && <div className="sweep" />}
        <SiteMap nodes={list} />
        <div className={`seemo-badge ${seemoBusy ? "busy" : ""}`}>
          <span className="av" />
          {seemoBusy ? `Seemo is reading ${waiting > 1 ? `${waiting} pages` : "a page"}` : done ? "Seemo is done" : "Seemo is waiting for pages"}
        </div>
        <div className="legend">
          <span>
            <i style={{ background: "var(--line-2)" }} />
            queued
          </span>
          <span>
            <i style={{ background: "var(--accent)" }} />
            fetched
          </span>
          <span>
            <i style={{ background: "var(--signal)" }} />
            with Seemo
          </span>
          <span>
            <i style={{ background: "var(--ink)" }} />
            understood
          </span>
          <span>
            <i style={{ background: "var(--warn)" }} />
            blocked
          </span>
        </div>
        {!started && !done && list.length === 0 && (
          <div className="big">
            <div>
              <h2 className="d2">{project.data?.last_crawl ? "Last crawl is in the profile" : "Nothing read yet"}</h2>
              <p className="muted">Start a crawl to watch the site map build.</p>
            </div>
          </div>
        )}
      </div>

      {done && done.blocked > 0 && (
        <p className="muted" style={{ marginBottom: 18 }}>
          {done.blocked} {done.blocked === 1 ? "page was" : "pages were"} blocked or need JavaScript — connect UnblockingAPI in <Link to="/connections">connections</Link> to read them too.
        </p>
      )}

      <div className="streams">
        <div className="stream">
          <h3 className="d3">
            Pages <span className="chip">{list.length}</span>
          </h3>
          {list.length === 0 && <p className="faint">Waiting for the first page.</p>}
          {[...list].reverse().map((n) => (
            <div key={n.url} className={`row ${n.state}`}>
              <span className="st" />
              <span className="u" title={n.url}>
                {pathOf(n.url)}
              </span>
              <span className="m">{n.meta}</span>
            </div>
          ))}
        </div>
        <div className="stream">
          <h3 className="d3">
            Seemo found <span className="chip">{factCount}</span>
          </h3>
          {facts.length === 0 && <p className="faint">Facts appear here as Seemo reads each page.</p>}
          {facts.map((f, i) => (
            <div key={f.id} className={`fact ${i === 0 ? "new" : ""}`}>
              <div className="k">
                {f.field.replace(/_/g, " ")} <span className="chip">{f.section}</span>
              </div>
              <div className="v">{f.value}</div>
              <div className="e">
                <code>{pathOf(f.url)}</code> {f.quote && `“${f.quote}”`}
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

function Stat({ n, label, hot }: { n: number; label: string; hot?: boolean }) {
  const [shown, setShown] = useState(n);
  useEffect(() => {
    if (shown === n) return;
    const step = Math.max(1, Math.ceil(Math.abs(n - shown) / 8));
    const t = setTimeout(() => setShown((s) => (s < n ? Math.min(n, s + step) : Math.max(n, s - step))), 30);
    return () => clearTimeout(t);
  }, [n, shown]);
  return (
    <div className={`stat ${hot ? "hot" : ""}`}>
      <div className="n">{shown}</div>
      <div className="l">{label}</div>
    </div>
  );
}

function SiteMap({ nodes }: { nodes: Node[] }) {
  const cx = W / 2;
  const cy = H / 2;
  const rings = [0, 95, 160, 205];
  const pos = (n: Node) => {
    if (n.depth === 0 && pathOf(n.url) === "/") return { x: cx, y: cy };
    const r = rings[n.depth] + (hash(n.url + "r") - 0.5) * 24;
    return { x: cx + Math.cos(n.angle) * r * 2.1, y: cy + Math.sin(n.angle) * r };
  };
  const byUrl = new Map(nodes.map((n) => [n.url, n]));
  return (
    <svg viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="xMidYMid meet" aria-hidden="true">
      {rings.slice(1).map((r) => (
        <ellipse key={r} className="ring" cx={cx} cy={cy} rx={r * 2.1} ry={r} />
      ))}
      {nodes.map((n) => {
        const from = n.from ? byUrl.get(n.from) : undefined;
        if (!from) return null;
        const a = pos(from);
        const b = pos(n);
        return <line key={`e-${n.url}`} className="edge" x1={a.x} y1={a.y} x2={b.x} y2={b.y} />;
      })}
      {nodes.map((n) => {
        const p = pos(n);
        const isRoot = pathOf(n.url) === "/";
        const label = pathOf(n.url);
        return (
          <g key={n.url} className={`node ${n.state} ${isRoot ? "root" : ""}`} transform={`translate(${p.x} ${p.y})`}>
            <title>{n.url}</title>
            {(n.state === "live" || n.state === "thinking") && <circle className="halo" r="7" />}
            <circle className="c" r={isRoot ? 9 : 6} />
            {(isRoot || n.depth <= 1 || n.state === "thinking" || n.state === "live") && (
              <text x="0" y={isRoot ? -14 : -11} textAnchor="middle">
                {label.length > 22 ? label.slice(0, 21) + "…" : label}
              </text>
            )}
          </g>
        );
      })}
    </svg>
  );
}
