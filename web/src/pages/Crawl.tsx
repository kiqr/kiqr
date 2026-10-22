import { useState } from "react";
import { Link, useParams } from "react-router-dom";
import { api, useAsync, useEvents, type KiqrEvent } from "../api";

type Row = { url: string; state: "queued" | "live" | "done" | "blocked" | "failed"; meta: string };

function pathOf(u: string) {
  try {
    const x = new URL(u);
    return x.pathname + x.search || "/";
  } catch {
    return u;
  }
}

export function CrawlPage() {
  const { slug = "" } = useParams();
  const project = useAsync(() => api.project(slug), [slug]);
  const [rows, setRows] = useState<Map<string, Row>>(new Map());
  const [facts, setFacts] = useState<{ url: string; n: number }[]>([]);
  const [done, setDone] = useState<{ pages: number; blocked: number } | null>(null);
  const [maxPages, setMaxPages] = useState<number | null>(null);

  useEvents(slug, (e: KiqrEvent) => {
    const url = e.url as string | undefined;
    const upd = (state: Row["state"], meta: string) =>
      setRows((m) => {
        const n = new Map(m);
        n.set(url!, { url: url!, state, meta });
        return n;
      });
    switch (e.type) {
      case "crawl_started":
        setRows(new Map());
        setFacts([]);
        setDone(null);
        setMaxPages(e.max_pages as number);
        break;
      case "page_fetching":
        upd("live", e.via === "unblockingapi" ? "via UnblockingAPI" : "reading");
        break;
      case "page_fetched":
        upd("done", `${e.ms} ms`);
        break;
      case "page_parsed":
        upd("done", `${e.facts} facts`);
        setFacts((f) => [{ url: url!, n: e.facts as number }, ...f]);
        break;
      case "page_blocked":
        upd("blocked", e.reason as string);
        break;
      case "page_failed":
        upd("failed", "failed");
        break;
      case "crawl_finished":
        setDone({ pages: e.pages as number, blocked: e.blocked as number });
        break;
    }
  });

  const list = [...rows.values()];
  const read = list.filter((r) => r.state === "done").length;
  const host = project.data?.name ?? slug;

  return (
    <div>
      <div className="feed-head">
        <h1 className="display">{done ? `Read ${done.pages} pages` : `Reading ${host}`}</h1>
        {!done && maxPages && <span className="feed-count body">{read} of {maxPages} pages</span>}
      </div>
      {done && done.blocked > 0 && (
        <p className="body" style={{ marginTop: "var(--space-2)" }}>
          {done.blocked} {done.blocked === 1 ? "page was" : "pages were"} blocked or need JavaScript — connect UnblockingAPI in{" "}
          <Link to="/settings">settings</Link> to read them.
        </p>
      )}
      {done && (
        <p className="body" style={{ marginTop: "var(--space-4)" }}>
          <Link to={`/p/${slug}`}>Open the profile</Link>
        </p>
      )}
      <div className="crawl">
        <section>
          {list.length === 0 && <p className="empty">Waiting for the first page.</p>}
          {list.map((r) => (
            <div key={r.url} className={`row ${r.state}`}>
              <span className={`dot ${r.state}`} />
              <span className="url" title={r.url}>{pathOf(r.url)}</span>
              <span className="meta">{r.meta}</span>
            </div>
          ))}
        </section>
        <section>
          {facts.length === 0 ? (
            <p className="empty">Facts appear here as pages are parsed.</p>
          ) : (
            facts.map((f, i) => (
              <div key={i} className="row done">
                <span className="dot" />
                <span className="url">{pathOf(f.url)}</span>
                <span className="meta">{f.n} facts</span>
              </div>
            ))
          )}
        </section>
      </div>
    </div>
  );
}
