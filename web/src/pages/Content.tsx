import { useState } from "react";
import { Link, useParams } from "react-router-dom";
import { Network, SearchX, type LucideIcon } from "lucide-react";
import { api, useAsync, useEvents, type ContentGap } from "../api";
import { pathOf } from "./Project";
import "../profile.css";

const COLORS = ["#5eead4", "#818cf8", "#f472b6", "#fbbf24", "#34d399", "#60a5fa", "#fb923c", "#a78bfa", "#2dd4bf", "#f87171"];
const INTENTS: ContentGap["intent"][] = ["informational", "commercial", "transactional", "navigational"];

function useBrief() {
  const { slug = "" } = useParams();
  const brief = useAsync(() => api.brief(slug), [slug]);
  useEvents(slug, (e) => e.type === "profile_updated" && e.section === "brief" && brief.reload());
  return { slug, brief };
}

/** The site's existing content, grouped into topic clusters. */
export function ContentMap() {
  const { slug, brief } = useBrief();
  const b = brief.data;
  if (brief.loading) return null;
  if (!b) return <NoBrief slug={slug} icon={Network} what="the content map" />;
  const total = b.topic_clusters.reduce((n, c) => n + c.page_count, 0) || 1;
  return (
    <div className="prof">
      <div className="cm-head">
        <div>
          <p className="cap">Content map</p>
          <h1 className="d2">What {b.business_name} has published</h1>
          <p>
            {total} pages in {b.topic_clusters.length} topic clusters. New articles fill the gaps between them.
          </p>
        </div>
        <Link className="btn g" to={`/p/${slug}/gaps`}>
          <SearchX size={15} /> {b.content_gaps.length} content gaps
        </Link>
      </div>
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
    </div>
  );
}

/** Topics the site doesn't cover yet; articles and posts are written from these. */
export function ContentGaps() {
  const { slug, brief } = useBrief();
  const [intent, setIntent] = useState<ContentGap["intent"] | null>(null);
  const b = brief.data;
  if (brief.loading) return null;
  if (!b) return <NoBrief slug={slug} icon={SearchX} what="content gaps" />;
  const gaps = b.content_gaps.filter((g) => !intent || g.intent === intent);
  return (
    <div className="prof">
      <div className="cm-head">
        <div>
          <p className="cap">Content gaps</p>
          <h1 className="d2">What {b.business_name} hasn't written about yet</h1>
          <p>Each gap is tied to something the business sells or someone it serves, and is likely to bring in customers from search. Seemo writes articles and posts from these.</p>
        </div>
      </div>
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
            <span className="chip soon" title="Article writing arrives in the next release">
              Write article · soon
            </span>
          </li>
        ))}
      </ul>
    </div>
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

/** A content screen that arrives in a later release, said plainly. */
export function ComingNext({ icon: Icon, title, text }: { icon: LucideIcon; title: string; text: string }) {
  const { slug = "" } = useParams();
  return (
    <div className="prof">
      <div className="cm-head">
        <div>
          <p className="cap">Content</p>
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
