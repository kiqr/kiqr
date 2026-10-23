import { useEffect, useRef, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { AnimatePresence, motion } from "motion/react";
import {
  BadgeDollarSign,
  Ban,
  Brain,
  Building2,
  Check,
  ChevronRight,
  CircleCheck,
  Download,
  ExternalLink,
  FileText,
  Globe,
  Handshake,
  Images,
  Languages,
  MapPin,
  MessageSquareQuote,
  Network,
  Package,
  Palette,
  Pencil,
  Plus,
  RefreshCw,
  Save,
  SearchX,
  ShieldCheck,
  Sparkles,
  Swords,
  Tag,
  Trash2,
  Users,
  X,
  type LucideIcon,
} from "lucide-react";
import { api, useAsync, useEvents, type Brand, type Brief, type Field, type Project, type Section } from "../api";
import "../profile.css";

const CLUSTER = ["#5eead4", "#818cf8", "#f472b6", "#fbbf24", "#34d399", "#60a5fa", "#fb923c", "#a78bfa", "#2dd4bf", "#f87171"];

export function ProjectPage() {
  const { slug = "" } = useParams();
  const navigate = useNavigate();
  const project = useAsync(() => api.project(slug), [slug]);
  const brief = useAsync(() => api.brief(slug), [slug]);
  const brand = useAsync(() => api.brand(slug), [slug]);
  const profile = useAsync(() => api.profile(slug), [slug]);
  const [building, setBuilding] = useState(false);
  useEvents(slug, (e) => {
    if (e.type === "brand_updated") brand.reload();
    if (e.type === "crawl_started" || e.type === "job_finished") project.reload();
    if (e.type === "job_finished") setBuilding(false);
    if (e.type === "profile_updated" || e.type === "crawl_finished" || e.type === "synthesis_finished") {
      if (e.type !== "profile_updated" || e.section === "brief") setBuilding(false);
      brief.reload();
      profile.reload();
      project.reload();
    }
  });

  if (project.error) return <p className="empty">{project.error}</p>;
  const p = project.data;
  const sections = (profile.data ?? []).filter((s) => s.fields.length > 0);
  const b = brief.data;
  const br = brand.data ?? null;
  const facts = sections.reduce((n, s) => n + s.fields.length, 0);

  const crawling = p?.jobs?.includes("crawl") ?? false;
  const recrawl = async () => {
    try {
      await api.crawl(slug);
    } catch (e) {
      // A crawl is already running: show it rather than start a second one.
      if (!/already running/.test((e as Error).message)) return window.alert((e as Error).message);
    }
    navigate(`/p/${slug}/crawl`);
  };
  const rebuild = async () => {
    setBuilding(true);
    try {
      await api.rebuildBrief(slug);
    } catch (e) {
      // Already being written (or a crawl will write it): keep showing progress.
      if (/already running|being rebuilt/.test((e as Error).message)) return;
      setBuilding(false);
      window.alert((e as Error).message);
    }
  };

  return (
    <div className="prof" style={brandVars(br)}>
      <Hero
        slug={slug}
        project={p ?? null}
        brand={br}
        name={b?.business_name || br?.site_name || p?.name || slug}
        meta={b ? [b.language && langName(b.language), b.geographies.length ? b.geographies.join(", ") : "Serves customers worldwide"] : []}
        onRescan={recrawl}
        crawling={crawling}
      />

      {b && (
        <div className="stats">
          <Stat icon={Package} n={b.offerings.length} label="Products & services" i={0} />
          <Stat icon={Users} n={b.audiences.length} label="Audiences" i={1} />
          <Stat icon={FileText} n={p?.last_crawl?.pages ?? 0} label="Pages read" i={2} />
          <Stat icon={ShieldCheck} n={facts} label="Facts with a source" i={3} />
          <Stat icon={Network} n={b.topic_clusters.length} label="Topic clusters" i={4} to={`/p/${slug}/map`} />
          <Stat icon={SearchX} n={b.content_gaps.length} label="Content gaps" i={5} to={`/p/${slug}/gaps`} />
        </div>
      )}

      {!brief.loading && !b && (
        <div className="pcard none">
          <Brain size={22} className="ac2" />
          <p className="t">No business profile yet</p>
          <p className="muted">
            {p?.last_crawl
              ? "The crawl is done. Seemo reads what it found and writes the profile: what the business sells, who it serves, how it sounds and what it hasn't written about yet."
              : "After a crawl, Seemo reads every page and builds a profile of what this business sells, who it serves and how it sounds. Everything KIQR writes is based on it."}
          </p>
          {p?.last_crawl ? (
            <button className="btn p" onClick={rebuild} disabled={building}>
              <Sparkles size={15} /> {building ? "Writing the profile…" : "Build the profile"}
            </button>
          ) : (
            <Link className="btn p" to={`/p/${slug}/crawl`}>
              Watch the crawl
            </Link>
          )}
        </div>
      )}

      {b && <BriefView key={`${b.version}-${b.updated_at}`} slug={slug} initial={b} brand={br} onSaved={brief.reload} />}
      {!b && br && <BrandKit brand={br} name={br.site_name || p?.name || slug} i={0} />}

      {sections.length > 0 && <Evidence slug={slug} sections={sections} onSaved={profile.reload} />}
    </div>
  );
}

/* ------------------------------------------------------------------- hero */

/** The business as it presents itself: its cover, its mark, its name and its own words. */
function Hero({ slug, project, brand, name, meta, onRescan, crawling }: { slug: string; project: Project | null; brand: Brand | null; name: string; meta: (string | false)[]; onRescan: () => void; crawling: boolean }) {
  const font = useBrandFont(brand?.fonts ?? []);
  const [coverOk, setCoverOk] = useState(true);
  const cover = coverOk ? brand?.cover : null;
  return (
    <header className={`hero ${cover ? "has-cover" : ""}`}>
      <div className="hero-bg" aria-hidden>
        {cover && <img src={cover} alt="" onError={() => setCoverOk(false)} />}
        <i className="g1" />
        <i className="g2" />
      </div>
      <div className="hero-top">
        {project && (
          <a href={project.url} target="_blank" rel="noreferrer" className="hero-url">
            <Globe size={13} /> {hostOf(project.url)} <ExternalLink size={11} />
          </a>
        )}
        <div className="prof-acts">
          {project && (
            <a className="btn glass" href={`/api/projects/${slug}/profile.md`} download={`${slug}-profile.md`}>
              <Download size={15} /> Export
            </a>
          )}
          {project &&
            (crawling ? (
              <Link className="btn glass live" to={`/p/${slug}/crawl`}>
                <span className="pulse" /> Crawl running
              </Link>
            ) : (
              <button className="btn glass" onClick={onRescan}>
                <RefreshCw size={15} /> Rescan website
              </button>
            ))}
        </div>
      </div>
      <div className="hero-main">
        <Mark brand={brand} name={name} />
        <div className="min0">
          <h1 className="hero-name" style={font ? { fontFamily: `"${font}", var(--sans)` } : undefined}>
            {name}
          </h1>
          {brand?.tagline && <p className="tagline">{brand.tagline}</p>}
          <div className="hero-meta">
            {meta.filter(Boolean).map((m, i) => (
              <span key={i} className="chip glass">
                {i === 0 ? <Languages size={12} /> : <MapPin size={12} />} {m}
              </span>
            ))}
            {brand?.social.map((s) => (
              <a key={s.network} className="chip glass" href={s.url} target="_blank" rel="noreferrer">
                {SOCIAL[s.network] ?? s.network} <ExternalLink size={10} />
              </a>
            ))}
          </div>
        </div>
      </div>
    </header>
  );
}

const SOCIAL: Record<string, string> = { linkedin: "LinkedIn", x: "X", facebook: "Facebook", instagram: "Instagram", youtube: "YouTube", github: "GitHub", tiktok: "TikTok" };

/** The site's icon, or its initial on its own colours when it has none. */
function Mark({ brand, name, size = 76 }: { brand: Brand | null; name: string; size?: number }) {
  const [ok, setOk] = useState(true);
  const src = brand?.icon;
  return (
    <span className="bmark" style={{ width: size, height: size }}>
      {src && ok ? <img src={src} alt="" onError={() => setOk(false)} /> : <b style={{ fontSize: size * 0.42 }}>{name.trim()[0]?.toUpperCase()}</b>}
    </span>
  );
}

function Stat({ icon: Icon, n, label, i, to }: { icon: LucideIcon; n: number; label: string; i: number; to?: string }) {
  if (!n) return null;
  const body = (
    <>
      <span className="ic">
        <Icon size={15} />
      </span>
      <span className="v">
        <Count to={n} />
      </span>
      <span className="l">{label}</span>
    </>
  );
  const style = { animationDelay: `${120 + i * 50}ms` };
  return to ? (
    <Link to={to} className="stat" style={style}>
      {body}
    </Link>
  ) : (
    <div className="stat" style={style}>
      {body}
    </div>
  );
}

/** A number that counts up to its value when it first shows. */
function Count({ to }: { to: number }) {
  const [v, setV] = useState(reduced() ? to : 0);
  useEffect(() => {
    if (reduced()) return setV(to);
    let raf = 0;
    const t0 = performance.now();
    const tick = (t: number) => {
      const k = Math.min(1, (t - t0) / 900);
      setV(Math.round(to * (1 - Math.pow(1 - k, 3))));
      if (k < 1) raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [to]);
  return <>{v}</>;
}

/* -------------------------------------------------------------- brand kit */

/** Logo, colours and type, as the site uses them. Click a colour to copy it. */
function BrandKit({ brand, name, i }: { brand: Brand; name: string; i: number }) {
  const [copied, setCopied] = useState<string | null>(null);
  const [logoOk, setLogoOk] = useState(true);
  const copy = (c: string) => {
    navigator.clipboard?.writeText(c).catch(() => {});
    setCopied(c);
    setTimeout(() => setCopied((x) => (x === c ? null : x)), 1400);
  };
  useBrandFont(brand.fonts);
  return (
    <Card title="Brand kit" icon={Palette} i={i}>
      {brand.logo && logoOk && (
        <div className="logos">
          <span className="light">
            <img src={brand.logo} alt={`${name} logo`} onError={() => setLogoOk(false)} />
          </span>
        </div>
      )}
      {brand.colors.length > 0 && (
        <>
          <p className="lab">Colours</p>
          <div className="swatches">
            {brand.colors.map((c, k) => (
              <button key={c} className="swc" style={{ "--c": c, animationDelay: `${200 + k * 70}ms` } as React.CSSProperties} onClick={() => copy(c)} title={`Copy ${c}`}>
                <i />
                <span>{copied === c ? <Check size={11} /> : c}</span>
              </button>
            ))}
          </div>
        </>
      )}
      {brand.fonts.length > 0 && (
        <>
          <p className="lab">Type</p>
          <div className="fonts">
            {brand.fonts.map((f) => (
              <div key={f} className="font">
                <span className="aa" style={{ fontFamily: `"${f}", var(--sans)` }}>
                  Aa
                </span>
                <span className="fn">{f}</span>
              </div>
            ))}
          </div>
        </>
      )}
      {!brand.logo && !brand.colors.length && !brand.fonts.length && <p className="none">The site doesn't declare a logo, colours or fonts Seemo could read.</p>}
    </Card>
  );
}

/** Pictures the site uses for itself: share images and the large images on its pages. */
function Imagery({ brand, i }: { brand: Brand; i: number }) {
  const [bad, setBad] = useState<string[]>([]);
  const imgs = brand.images.filter((m) => !bad.includes(m.url)).slice(0, 7);
  if (!imgs.length) return null;
  return (
    <Card title="From your website" icon={Images} i={i} more={<span className="faint">{imgs.length} images</span>}>
      <div className={`gallery n${imgs.length}`}>
        {imgs.map((m, k) => (
          <a key={m.url} href={m.page || m.url} target="_blank" rel="noreferrer" className="shot" style={{ animationDelay: `${k * 60}ms` }}>
            <img src={m.url} alt={m.alt} loading="lazy" onError={() => setBad((b) => [...b, m.url])} />
            <span className="cap2">{m.alt || pathOf(m.page)}</span>
          </a>
        ))}
      </div>
    </Card>
  );
}

/** Load the site's own display font from Google Fonts, when it uses one we don't ship. */
function useBrandFont(fonts: string[]) {
  // The first font is the one the site sets most of its text in.
  const font = fonts[0] ?? null;
  useEffect(() => {
    for (const f of fonts) {
      if (/^(inter|jetbrains mono)$/i.test(f) || !/^[a-z0-9 ]+$/i.test(f)) continue;
      const id = `bf-${f.replace(/\s+/g, "-").toLowerCase()}`;
      if (document.getElementById(id)) continue;
      const l = document.createElement("link");
      l.id = id;
      l.rel = "stylesheet";
      l.href = `https://fonts.googleapis.com/css2?family=${encodeURIComponent(f).replace(/%20/g, "+")}:wght@400;600;700&display=swap`;
      document.head.appendChild(l);
    }
  }, [fonts.join("|")]); // eslint-disable-line react-hooks/exhaustive-deps
  return font;
}

/** The site's colours as CSS variables for the identity accents (never for actions). */
function brandVars(brand: Brand | null): React.CSSProperties {
  const c = brand?.colors ?? [];
  if (!c.length) return {};
  return { "--brand": c[0], "--brand-2": c[1] ?? c[0] } as React.CSSProperties;
}

/* ------------------------------------------------------------------ brief */

function BriefView({ slug, initial, brand, onSaved }: { slug: string; initial: Brief; brand: Brand | null; onSaved: () => void }) {
  const [data, setData] = useState(initial);
  const [editing, setEditing] = useState(false);
  const [saving, setSaving] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const set = <K extends keyof Brief>(k: K, v: Brief[K]) => setData((d) => ({ ...d, [k]: v }));
  const tone = data.tone_of_voice;
  const setTone = (patch: Partial<Brief["tone_of_voice"]>) => set("tone_of_voice", { ...tone, ...patch });
  const palette = [...(brand?.colors ?? []), ...CLUSTER];
  let n = 0;

  const save = async () => {
    setSaving(true);
    setErr(null);
    try {
      await api.saveBrief(slug, data);
      setEditing(false);
      onSaved();
    } catch (e) {
      setErr((e as Error).message);
    } finally {
      setSaving(false);
    }
  };

  return (
    <>
      <div className={`toolbar ${editing ? "on" : ""}`}>
        <p className="ver">
          <Sparkles size={12} className="ac2" /> Version {data.version} · {data.source === "edited" ? "edited by you" : "written by Seemo from the crawl"} · {ago(data.updated_at)}
        </p>
        {editing ? (
          <div className="row">
            <button
              className="btn t"
              onClick={() => {
                setData(initial);
                setEditing(false);
              }}
            >
              <X size={14} /> Cancel
            </button>
            <button className="btn p" onClick={save} disabled={saving}>
              <Save size={14} /> {saving ? "Saving…" : "Save new version"}
            </button>
          </div>
        ) : (
          <button className="btn g" onClick={() => setEditing(true)}>
            <Pencil size={14} /> Edit
          </button>
        )}
      </div>
      {err && <p className="help bad">{err}</p>}

      <div className="stack">
        <div className={brand ? "two wide" : ""}>
          <Card title="What the business does" icon={Building2} i={n++}>
            {editing ? (
              <div className="ed">
                <div className="row">
                  <input className="input" value={data.business_name} onChange={(e) => set("business_name", e.target.value)} aria-label="Business name" />
                  <input className="input lang" value={data.language} onChange={(e) => set("language", e.target.value)} aria-label="Language" />
                </div>
                <textarea className="input tall" value={data.summary} onChange={(e) => set("summary", e.target.value)} />
              </div>
            ) : (
              <p className="body lead2">{data.summary}</p>
            )}
            {!editing && data.brand_terms.length > 0 && (
              <div className="chips terms">
                {data.brand_terms.slice(0, 6).map((t) => (
                  <span key={t} className="tk brand">
                    {t}
                  </span>
                ))}
              </div>
            )}
          </Card>
          {brand && <BrandKit brand={brand} name={data.business_name} i={n++} />}
        </div>

        <div className="two">
          <Card title="Products & services" icon={Package} i={n++} more={<span className="faint">{data.offerings.length}</span>}>
            <ul className="items">
              {data.offerings.map((o, i) => (
                <li key={i} className="it rich" style={{ animationDelay: `${i * 40}ms` }}>
                  {editing ? (
                    <div className="ed">
                      <div className="row">
                        <input className="input" value={o.name} onChange={(e) => set("offerings", data.offerings.map((x, j) => (j === i ? { ...x, name: e.target.value } : x)))} />
                        <select className="input" value={o.kind} onChange={(e) => set("offerings", data.offerings.map((x, j) => (j === i ? { ...x, kind: e.target.value as "product" | "service" } : x)))}>
                          <option value="service">service</option>
                          <option value="product">product</option>
                        </select>
                        <Remove onClick={() => set("offerings", data.offerings.filter((_, j) => j !== i))} />
                      </div>
                      <textarea className="input" value={o.description} onChange={(e) => set("offerings", data.offerings.map((x, j) => (j === i ? { ...x, description: e.target.value } : x)))} />
                    </div>
                  ) : (
                    <>
                      <span className={`oic ${o.kind}`}>{o.kind === "product" ? <Package size={15} /> : <Handshake size={15} />}</span>
                      <div className="min0 grow">
                        <p className="nm">
                          {o.name} <span className="kind">{o.kind}</span>
                          {o.url && (
                            <a className="src" href={o.url} target="_blank" rel="noreferrer" title={o.url}>
                              {pathOf(o.url)}
                            </a>
                          )}
                        </p>
                        <p className="ds">{o.description}</p>
                      </div>
                    </>
                  )}
                </li>
              ))}
            </ul>
            {editing && <Add label="Add offering" onClick={() => set("offerings", [...data.offerings, { name: "", kind: "service", description: "" }])} />}
          </Card>

          <Card title="Audiences" icon={Users} i={n++} more={<span className="faint">{data.audiences.length}</span>}>
            <ul className="items">
              {data.audiences.map((a, i) => (
                <li key={i} className="it rich" style={{ animationDelay: `${i * 40}ms` }}>
                  {editing ? (
                    <div className="ed">
                      <div className="row">
                        <input className="input" value={a.name} onChange={(e) => set("audiences", data.audiences.map((x, j) => (j === i ? { ...x, name: e.target.value } : x)))} />
                        <Remove onClick={() => set("audiences", data.audiences.filter((_, j) => j !== i))} />
                      </div>
                      <Chips items={a.needs} editing onChange={(needs) => set("audiences", data.audiences.map((x, j) => (j === i ? { ...x, needs } : x)))} />
                    </div>
                  ) : (
                    <>
                      <span className="avatar" style={{ "--c": palette[i % palette.length] } as React.CSSProperties}>
                        {initials(a.name)}
                      </span>
                      <div className="min0 grow">
                        <p className="nm">{a.name}</p>
                        <div className="needs">
                          {a.needs.map((x) => (
                            <span key={x}>{x}</span>
                          ))}
                        </div>
                      </div>
                    </>
                  )}
                </li>
              ))}
            </ul>
            {editing && <Add label="Add audience" onClick={() => set("audiences", [...data.audiences, { name: "", needs: [] }])} />}
          </Card>
        </div>

        <Card title="Tone of voice" icon={MessageSquareQuote} i={n++}>
          {!editing && tone.examples.length > 0 && <Voice examples={tone.examples} font={brand?.fonts[0]} />}
          {editing ? <textarea className="input" value={tone.description} onChange={(e) => setTone({ description: e.target.value })} /> : <p className="body">{tone.description}</p>}
          <div className="three">
            <Labeled label="Traits">
              <Chips items={tone.traits} editing={editing} onChange={(traits) => setTone({ traits })} icon={<CircleCheck size={11} />} />
            </Labeled>
            <Labeled label="Example sentences">
              <Chips items={tone.examples} editing={editing} block quotes onChange={(examples) => setTone({ examples })} />
            </Labeled>
            <Labeled label="Avoid">
              <Chips items={tone.avoid} editing={editing} onChange={(avoid) => setTone({ avoid })} icon={<Ban size={11} />} variant="avoid" />
            </Labeled>
          </div>
        </Card>

        {brand && !editing && <Imagery brand={brand} i={n++} />}

        <div className="two">
          <Card title="Brand terms" icon={Tag} i={n++}>
            <Chips items={data.brand_terms} editing={editing} onChange={(v) => set("brand_terms", v)} variant="brand" />
          </Card>
          <Card title="Regions served" icon={MapPin} i={n++}>
            <Chips items={data.geographies} editing={editing} onChange={(v) => set("geographies", v)} empty="Worldwide, no specific region" icon={<MapPin size={11} />} />
          </Card>
        </div>

        {(data.differentiators.length > 0 || data.competitors.length > 0 || editing) && (
          <div className="two">
            <Card title="What sets it apart" icon={Sparkles} i={n++}>
              {editing ? (
                <Chips items={data.differentiators} editing block onChange={(v) => set("differentiators", v)} />
              ) : (
                <ul className="apart">
                  {data.differentiators.map((d, i) => (
                    <li key={i} style={{ animationDelay: `${i * 50}ms` }}>
                      <CircleCheck size={15} />
                      <span>{d}</span>
                    </li>
                  ))}
                </ul>
              )}
            </Card>
            <Card title="Competitors" icon={Swords} i={n++}>
              <ul className="items">
                {data.competitors.map((c, i) => (
                  <li key={i} className="it rich">
                    {editing ? (
                      <div className="ed">
                        <div className="row">
                          <input className="input" value={c.name} onChange={(e) => set("competitors", data.competitors.map((x, j) => (j === i ? { ...x, name: e.target.value } : x)))} />
                          <Remove onClick={() => set("competitors", data.competitors.filter((_, j) => j !== i))} />
                        </div>
                        <input className="input" value={c.note} onChange={(e) => set("competitors", data.competitors.map((x, j) => (j === i ? { ...x, note: e.target.value } : x)))} />
                      </div>
                    ) : (
                      <>
                        <span className="avatar sq">{initials(c.name)}</span>
                        <div className="min0 grow">
                          <p className="nm">{c.name}</p>
                          <p className="ds">{c.note}</p>
                        </div>
                      </>
                    )}
                  </li>
                ))}
              </ul>
              {editing && <Add label="Add competitor" onClick={() => set("competitors", [...data.competitors, { name: "", note: "" }])} />}
            </Card>
          </div>
        )}

        {(data.pricing_summary || editing) && (
          <Card title="Pricing" icon={BadgeDollarSign} i={n++}>
            {editing ? <textarea className="input" value={data.pricing_summary} onChange={(e) => set("pricing_summary", e.target.value)} /> : <p className="body">{data.pricing_summary}</p>}
          </Card>
        )}

        <Card title="Existing topic clusters" icon={Network} i={n++} more={<Link to={`/p/${slug}/map`}>Content map <ChevronRight size={12} /></Link>}>
          <ClusterBar clusters={data.topic_clusters} />
          <div className="clusters">
            {data.topic_clusters.map((c, i) => (
              <div key={i} className="it cl" style={{ "--c": CLUSTER[i % CLUSTER.length], animationDelay: `${i * 40}ms` } as React.CSSProperties}>
                <p className="nm">
                  <b className="dot" />
                  {c.label} <span className="pc">· {c.page_count} pages</span>
                </p>
                <p className="ds">{c.description}</p>
              </div>
            ))}
          </div>
        </Card>

        <Card title="Content gaps we found" icon={SearchX} i={n++} more={<Link to={`/p/${slug}/gaps`}>All gaps <ChevronRight size={12} /></Link>}>
          <ul className="items">
            {data.content_gaps.map((g, i) => (
              <li key={i} className="it gap" style={{ animationDelay: `${i * 30}ms` }}>
                <span className={`intent-ic ${g.intent}`} title={g.intent}>
                  {String(i + 1).padStart(2, "0")}
                </span>
                <div className="min0 grow">
                  {editing ? (
                    <input className="input" value={g.topic} onChange={(e) => set("content_gaps", data.content_gaps.map((x, j) => (j === i ? { ...x, topic: e.target.value } : x)))} />
                  ) : (
                    <p className="nm">{g.topic}</p>
                  )}
                  <p className="ds">
                    <span className="intent">{g.intent}</span> · {g.rationale}
                  </p>
                </div>
                {editing && <Remove onClick={() => set("content_gaps", data.content_gaps.filter((_, j) => j !== i))} />}
              </li>
            ))}
          </ul>
        </Card>
      </div>
    </>
  );
}

/** One of the site's own sentences at a time, in its own display type. */
function Voice({ examples, font }: { examples: string[]; font?: string }) {
  const [k, setK] = useState(0);
  const hover = useRef(false);
  useEffect(() => {
    if (examples.length < 2 || reduced()) return;
    const t = setInterval(() => !hover.current && setK((x) => (x + 1) % examples.length), 4800);
    return () => clearInterval(t);
  }, [examples.length]);
  return (
    <div className="voice" onMouseEnter={() => (hover.current = true)} onMouseLeave={() => (hover.current = false)}>
      <span className="qm">“</span>
      <AnimatePresence mode="wait">
        <motion.p
          key={k}
          initial={{ opacity: 0, y: 10, filter: "blur(4px)" }}
          animate={{ opacity: 1, y: 0, filter: "blur(0px)" }}
          exit={{ opacity: 0, y: -8, filter: "blur(4px)" }}
          transition={{ duration: 0.45, ease: [0.2, 0.8, 0.2, 1] }}
          style={font ? { fontFamily: `"${font}", var(--sans)` } : undefined}
        >
          {examples[k % examples.length]}
        </motion.p>
      </AnimatePresence>
      {examples.length > 1 && (
        <div className="dots">
          {examples.map((_, j) => (
            <button key={j} className={j === k % examples.length ? "on" : ""} onClick={() => setK(j)} aria-label={`Example ${j + 1}`} />
          ))}
        </div>
      )}
    </div>
  );
}

function ClusterBar({ clusters }: { clusters: Brief["topic_clusters"] }) {
  const total = clusters.reduce((s, c) => s + c.page_count, 0) || 1;
  return (
    <div className="cm-bar sm" aria-hidden>
      {clusters.map((c, i) => (
        <i key={i} title={`${c.label}: ${c.page_count}`} style={{ width: `${(c.page_count / total) * 100}%`, background: CLUSTER[i % CLUSTER.length], animationDelay: `${300 + i * 60}ms` }} />
      ))}
    </div>
  );
}

function Card({ title, icon: Icon, i, more, children }: { title: string; icon: LucideIcon; i: number; more?: React.ReactNode; children: React.ReactNode }) {
  return (
    <section className="pcard" style={{ animationDelay: `${180 + i * 60}ms` }}>
      <h3 className="cap">
        <span className="ic">
          <Icon size={14} />
        </span>
        {title}
        {more && <span className="more">{more}</span>}
      </h3>
      {children}
    </section>
  );
}

function Labeled({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div>
      <p className="lab">{label}</p>
      {children}
    </div>
  );
}

function Remove({ onClick }: { onClick: () => void }) {
  return (
    <button className="rm" onClick={onClick} aria-label="Remove">
      <Trash2 size={14} />
    </button>
  );
}

function Add({ label, onClick }: { label: string; onClick: () => void }) {
  return (
    <button className="addl" onClick={onClick}>
      <Plus size={12} /> {label}
    </button>
  );
}

function Chips({
  items,
  editing,
  onChange,
  block = false,
  quotes = false,
  empty = "—",
  icon,
  variant,
}: {
  items: string[];
  editing: boolean;
  onChange: (v: string[]) => void;
  block?: boolean;
  quotes?: boolean;
  empty?: string;
  icon?: React.ReactNode;
  variant?: "brand" | "avoid";
}) {
  const [draft, setDraft] = useState("");
  return (
    <div>
      <div className={`chips ${block ? "block" : ""} ${quotes ? "quotes" : ""}`}>
        {items.map((it, i) => (
          <span key={i} className={`tk ${variant ?? ""}`} style={{ animationDelay: `${i * 35}ms` }}>
            {icon}
            <span>{it}</span>
            {editing && (
              <button onClick={() => onChange(items.filter((_, j) => j !== i))} aria-label={`Remove ${it}`}>
                <X size={11} />
              </button>
            )}
          </span>
        ))}
        {!items.length && !editing && <span className="none">{empty}</span>}
      </div>
      {editing && (
        <input
          className="input sm"
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && draft.trim()) {
              e.preventDefault();
              onChange([...items, draft.trim()]);
              setDraft("");
            }
          }}
          placeholder="Type and press Enter"
        />
      )}
    </div>
  );
}

const reduced = () => window.matchMedia("(prefers-reduced-motion: reduce)").matches;

function initials(s: string) {
  const w = s.replace(/[^\p{L}\p{N} ]/gu, " ").split(/\s+/).filter(Boolean);
  return ((w[0]?.[0] ?? "") + (w[1]?.[0] ?? "")).toUpperCase() || "·";
}

function hostOf(u: string) {
  try {
    return new URL(u).host.replace(/^www\./, "");
  } catch {
    return u;
  }
}

const LANGS: Record<string, string> = { en: "English", sv: "Swedish", de: "German", fr: "French", es: "Spanish", da: "Danish", no: "Norwegian", nb: "Norwegian", fi: "Finnish", nl: "Dutch", it: "Italian", pt: "Portuguese", pl: "Polish" };
function langName(code: string) {
  return LANGS[code.toLowerCase().slice(0, 2)] ?? code;
}

/* --------------------------------------------------------------- evidence */

/** Every fact the profile rests on, by section, with the page and quote it came from. */
function Evidence({ slug, sections, onSaved }: { slug: string; sections: Section[]; onSaved: () => void }) {
  const [open, setOpen] = useState<string | null>(null);
  const total = sections.reduce((n, s) => n + s.fields.length, 0);
  return (
    <section className="pcard evid">
      <h3 className="cap">
        <span className="ic">
          <ShieldCheck size={14} />
        </span>
        Every fact, with its source <span className="more faint">{total} fields</span>
      </h3>
      <p className="ds">What Seemo read on the site, section by section. Edit or lock anything; locked fields keep your value through every rescan.</p>
      <div className="acc">
        {sections.map((s) => (
          <div key={s.id} className={`accs ${open === s.id ? "on" : ""}`}>
            <button className="acch" onClick={() => setOpen((o) => (o === s.id ? null : s.id))} aria-expanded={open === s.id}>
              <ChevronRight size={14} className="chev" />
              <span className="nm">{s.title}</span>
              <span className="ds">{s.description}</span>
              <span className="n">{s.fields.length}</span>
            </button>
            {open === s.id && <SectionFields slug={slug} section={s} onSaved={onSaved} />}
          </div>
        ))}
      </div>
    </section>
  );
}

function SectionFields({ slug, section, onSaved }: { slug: string; section: Section; onSaved: () => void }) {
  const save = async (fields: Field[]) => {
    await api.saveSection(slug, { ...section, fields });
    onSaved();
  };
  return (
    <div className="facts">
      {section.fields.map((f, i) => (
        <FactRow
          key={f.name}
          field={f}
          onChange={(nf) => save(section.fields.map((x, j) => (j === i ? nf : x)))}
          onRemove={() => save(section.fields.filter((_, j) => j !== i))}
        />
      ))}
    </div>
  );
}

function FactRow({ field, onChange, onRemove }: { field: Field; onChange: (f: Field) => void; onRemove: () => void }) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(field.value);
  const low = !field.locked && field.confidence > 0 && field.confidence < 0.6;
  return (
    <div className="fact">
      <div className="fh">
        <span className="nm">{human(field.name)}</span>
        {field.locked && <span className="chip acc">locked by you</span>}
        {low && <span className="chip warn">needs review</span>}
        <span className="acts">
          <button className="btn t sm" onClick={() => (setDraft(field.value), setEditing(true))}>
            Edit
          </button>
          <button className="btn t sm" onClick={() => onChange({ ...field, locked: !field.locked })}>
            {field.locked ? "Unlock" : "Lock"}
          </button>
          <button className="btn t sm" onClick={() => window.confirm(`Remove "${human(field.name)}" from the profile?`) && onRemove()}>
            Remove
          </button>
        </span>
      </div>
      {editing ? (
        <div className="ed">
          <textarea className="input" value={draft} onChange={(e) => setDraft(e.target.value)} autoFocus />
          <div className="row">
            <button
              className="btn p sm"
              onClick={() => {
                onChange({ ...field, value: draft, locked: true, edited_at: new Date().toISOString() });
                setEditing(false);
              }}
            >
              Save and lock
            </button>
            <button className="btn t sm" onClick={() => setEditing(false)}>
              Cancel
            </button>
          </div>
        </div>
      ) : (
        <p className="fv">{field.value}</p>
      )}
      <div className="fe">
        {field.evidence.slice(0, 3).map((e, i) => (
          <span key={i}>
            <code>{pathOf(e.url)}</code> {e.quote && <q>{e.quote}</q>}
          </span>
        ))}
        {!field.locked && field.confidence > 0 && (
          <span className={`conf ${low ? "low" : ""}`}>
            <i style={{ "--w": `${Math.round(field.confidence * 100)}%` } as React.CSSProperties} />
            {field.confidence.toFixed(2)}
          </span>
        )}
      </div>
    </div>
  );
}

/* ---------------------------------------------------------------- helpers */

const WORDS: Record<string, string> = { cta: "calls to action", seo: "SEO", faq: "FAQ", mcp: "MCP", api: "API", sdks: "SDKs", sla: "SLA", ai: "AI" };
/** `value_props` → "Value props", `mcp_server` → "MCP server". */
export function human(name: string) {
  const s = name
    .split("_")
    .map((w) => WORDS[w.toLowerCase()] ?? w)
    .join(" ");
  return s[0].toUpperCase() + s.slice(1);
}

export function pathOf(u: string) {
  try {
    return new URL(u).pathname || "/";
  } catch {
    return u;
  }
}

export function ago(iso: string) {
  const s = (Date.now() - new Date(iso).getTime()) / 1000;
  if (!isFinite(s)) return "";
  if (s < 60) return "just now";
  if (s < 3600) return `${Math.floor(s / 60)}m ago`;
  if (s < 86400) return `${Math.floor(s / 3600)}h ago`;
  return `${Math.floor(s / 86400)}d ago`;
}
