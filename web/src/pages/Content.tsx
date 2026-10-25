import { useEffect, useState } from "react";
import { Link, Navigate, useLocation, useNavigate, useParams } from "react-router-dom";
import { Check, Network, Plus, RefreshCw, SearchX, type LucideIcon } from "lucide-react";
import { api, useAsync, useEvents, type Asset, type ContentGap } from "../api";
import { pathOf } from "./Project";
import "../profile.css";
import "../library.css";

const COLORS = ["#5eead4", "#818cf8", "#f472b6", "#fbbf24", "#34d399", "#60a5fa", "#fb923c", "#a78bfa", "#2dd4bf", "#f87171"];
const INTENTS: ContentGap["intent"][] = ["informational", "commercial", "transactional", "navigational"];

function useBrief() {
  const { slug = "" } = useParams();
  const brief = useAsync(() => api.brief(slug), [slug]);
  useEvents(slug, (e) => e.type === "profile_updated" && e.section === "brief" && brief.reload());
  return { slug, brief };
}

/** Your business → Content: what the site has published (the site map, in topic clusters) and what it has not written about yet (the gaps). */
export function ContentPage() {
  const { slug, brief } = useBrief();
  const navigate = useNavigate();
  const { hash } = useLocation();
  const project = useAsync(() => api.project(slug), [slug]);
  const b = brief.data;
  useEffect(() => {
    if (hash === "#gaps" && b) document.getElementById("gaps")?.scrollIntoView({ behavior: "smooth", block: "start" });
  }, [hash, b]);
  if (brief.loading) return null;
  if (!b) return <NoBrief slug={slug} icon={Network} what="the content map" />;
  const total = b.topic_clusters.reduce((n, c) => n + c.page_count, 0) || 1;
  const crawling = project.data?.jobs?.includes("crawl") ?? false;
  const rescan = async () => {
    try {
      await api.crawl(slug);
    } catch (e) {
      if (!/already running/.test((e as Error).message)) return;
    }
    navigate(`/p/${slug}/crawl`);
  };
  return (
    <div className="prof">
      <div className="cm-head">
        <div>
          <p className="cap">Your business</p>
          <h1 className="d2">Content</h1>
          <p>
            What {b.business_name} has published, in {b.topic_clusters.length} topic clusters over {total} pages, and the {b.content_gaps.length} gaps Seemo would fill next.
          </p>
        </div>
        <div className="cm-views">
          <a className="chip" href="#gaps">
            <SearchX size={13} /> Gaps {b.content_gaps.length}
          </a>
          {crawling ? (
            <Link className="btn g live" to={`/p/${slug}/crawl`}>
              <span className="pulse" /> Crawl running
            </Link>
          ) : (
            <button className="btn g" onClick={() => void rescan()}>
              <RefreshCw size={14} /> Rescan website
            </button>
          )}
        </div>
      </div>
      <section className="cm-sec" id="map">
        <h2>
          <Network size={16} /> Site map <span className="n">{total} pages</span>
        </h2>
        <MapSection b={b} total={total} />
      </section>
      <section className="cm-sec" id="gaps">
        <h2>
          <SearchX size={16} /> Content gaps <span className="n">{b.content_gaps.length}</span>
        </h2>
        <p className="cm-sec-ds">Each gap is tied to something the business sells or someone it serves, and is likely to bring in customers from search. Seemo writes articles and posts from these.</p>
        <GapsSection slug={slug} b={b} />
      </section>
    </div>
  );
}

function MapSection({ b, total }: { b: NonNullable<ReturnType<typeof useBrief>["brief"]["data"]>; total: number }) {
  return (
    <>
      <div className="cm-bar" aria-hidden>
        {b.topic_clusters.map((c, i) => (
          <i key={i} title={`${c.label}: ${c.page_count}`} style={{ width: `${(c.page_count / total) * 100}%`, background: COLORS[i % COLORS.length], animationDelay: `${i * 60}ms` }} />
        ))}
      </div>
      <div className="cm-grid">
        {b.topic_clusters.map((c, i) => {
          const urls = c.urls ?? [];
          return (
            <section key={i} className="pcard cm-card" style={{ "--c": COLORS[i % COLORS.length], animationDelay: `${i * 50}ms` } as React.CSSProperties}>
              <p className="nm">
                {c.label}
                <span className="pc">{c.page_count} pages</span>
              </p>
              <p className="ds">{c.description}</p>
              {urls.length > 0 && (
                <ul>
                  {urls.slice(0, 8).map((u) => (
                    <li key={u}>
                      <a href={u} target="_blank" rel="noreferrer" title={u}>
                        {pathOf(u)}
                      </a>
                    </li>
                  ))}
                </ul>
              )}
              {urls.length > 8 && <p className="rest">+{urls.length - 8} more</p>}
            </section>
          );
        })}
      </div>
    </>
  );
}

/** Topics the site doesn't cover yet; articles and posts are written from these. */
function GapsSection({ slug, b }: { slug: string; b: NonNullable<ReturnType<typeof useBrief>["brief"]["data"]> }) {
  const [intent, setIntent] = useState<ContentGap["intent"] | null>(null);
  const articles = useAsync(() => api.assets(slug, "article").catch(() => [] as Asset[]), [slug]);
  const [busy, setBusy] = useState<string | null>(null);
  const planned = (topic: string) => (articles.data ?? []).find((a) => a.source?.gap === topic);
  const plan = async (g: ContentGap) => {
    setBusy(g.topic);
    try {
      await api.createAsset(slug, { type: "article", title: g.topic, summary: g.rationale, tags: [g.intent], source: { gap: g.topic, intent: g.intent } });
      articles.reload();
    } finally {
      setBusy(null);
    }
  };
  const gaps = b.content_gaps.filter((g) => !intent || g.intent === intent);
  return (
    <>
      <div className="filters">
        <button className={`chip ${intent === null ? "acc" : ""}`} onClick={() => setIntent(null)}>
          All {b.content_gaps.length}
        </button>
        {INTENTS.map((k) => {
          const n = b.content_gaps.filter((g) => g.intent === k).length;
          return n ? (
            <button key={k} className={`chip ${intent === k ? "acc" : ""}`} onClick={() => setIntent(k)}>
              {k} {n}
            </button>
          ) : null;
        })}
      </div>
      <ul className="items gaps">
        {gaps.map((g, i) => (
          <li key={g.topic} className="it" style={{ animationDelay: `${i * 35}ms` }}>
            <span className="num">{String(i + 1).padStart(2, "0")}</span>
            <div className="min0">
              <p className="nm">{g.topic}</p>
              <p className="ds">
                <span className="intent">{g.intent}</span> · {g.rationale}
              </p>
            </div>
            {planned(g.topic) ? (
              <Link className="chip acc plan done" to={`/p/${slug}/library/article/${planned(g.topic)!.id}`}>
                <Check size={12} /> In the library
              </Link>
            ) : (
              <button className="chip plan" disabled={busy === g.topic} onClick={() => plan(g)} title="Add a draft article for this gap to the library">
                <Plus size={12} /> Plan article
              </button>
            )}
          </li>
        ))}
      </ul>
    </>
  );
}

function NoBrief({ slug, icon: Icon, what }: { slug: string; icon: LucideIcon; what: string }) {
  return (
    <div className="pcard none">
      <Icon size={22} className="ac2" />
      <p className="t">No business profile yet</p>
      <p className="muted">Seemo builds {what} from the business profile, which is written after a crawl.</p>
      <Link className="btn p" to={`/p/${slug}`}>
        Business profile
      </Link>
    </div>
  );
}

/** A relative redirect inside the project, for links to screens that moved. */
export function Redirect({ to }: { to: string }) {
  const { slug = "" } = useParams();
  return <Navigate to={`/p/${slug}/${to}`} replace />;
}

/** A screen that arrives in a later release, said plainly. */
export function ComingNext({ icon: Icon, title, text, cap = "Content" }: { icon: LucideIcon; title: string; text: string; cap?: string }) {
  const { slug = "" } = useParams();
  return (
    <div className="prof">
      <div className="cm-head">
        <div>
          <p className="cap">{cap}</p>
          <h1 className="d2">{title}</h1>
        </div>
      </div>
      <div className="pcard none">
        <Icon size={22} className="ac2" />
        <p className="t">Coming in the next release</p>
        <p className="muted">{text}</p>
        <Link className="btn g" to={`/p/${slug}/gaps`}>
          See the content gaps
        </Link>
      </div>
    </div>
  );
}
