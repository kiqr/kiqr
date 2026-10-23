// The library: everything made with Seemo, by type. Types are data, so this page works for
// kinds of assets nobody has defined yet: it previews what it can and lists files otherwise.
import { useEffect, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { motion } from "motion/react";
import { ArrowLeft, Bot, Download, ExternalLink, FileText, Library as LibraryIcon, SearchX, Send, Trash2, User } from "lucide-react";
import { api, useAsync, useEvents, type Asset, type AssetStatus, type AssetType } from "../api";
import { assetIcon } from "../assetIcons";
import { ago } from "./Project";
import { EmptyLibrary, Guide } from "./LibraryEmpty";
import "../profile.css";
import "../library.css";

const STATUSES: AssetStatus[] = ["draft", "ready", "scheduled", "published", "archived"];
const STATUS_TONE: Record<AssetStatus, string> = { draft: "", ready: "acc", scheduled: "ai", published: "ok", archived: "" };
const IMAGE = /\.(png|jpe?g|webp|gif|svg|avif)$/i;
const VIDEO = /\.(mp4|webm|mov)$/i;
const AUDIO = /\.(mp3|wav|ogg|m4a)$/i;
const TEXT = /\.(md|txt|html|ya?ml|json)$/i;

function useLibrary(slug: string, type?: string) {
  const types = useAsync(() => api.assetTypes(), []);
  const assets = useAsync(() => api.assets(slug, type), [slug, type]);
  useEvents(slug, (e) => (e.type === "asset_saved" || e.type === "asset_deleted") && assets.reload(), { demo: false });
  return { types: types.data ?? [], assets };
}

/** `/p/:slug/library` and `/p/:slug/library/:type`. */
export function LibraryPage() {
  const { slug = "", type } = useParams();
  const { types, assets } = useLibrary(slug, type);
  const [status, setStatus] = useState<AssetStatus | null>(null);
  useEffect(() => setStatus(null), [type]);
  const t = types.find((x) => x.id === type);
  const all = assets.data ?? [];
  const list = all.filter((a) => !status || a.status === status);
  const Icon = t ? assetIcon(t.icon) : LibraryIcon;

  return (
    <div className="prof lib">
      <div className="cm-head">
        <div className="lib-title">
          <span className="lib-ic">
            <Icon size={18} />
          </span>
          <div>
            <p className="cap">{t ? "Library" : "Content"}</p>
            <h1 className="d2">{t ? t.plural : "Library"}</h1>
            <p>{t ? t.description : "Everything you and Seemo have made for this project: articles, posts, images, videos and whatever else you dream up."}</p>
          </div>
        </div>
      </div>

      {!t && all.length > 0 && <Guide slug={slug} hideWhenComplete />}
      {!t && all.length > 0 && <TypeStrip slug={slug} types={types} assets={all} />}

      {all.length > 0 && (
        <div className="filters">
          <button className={`chip ${status === null ? "acc" : ""}`} onClick={() => setStatus(null)}>
            All {all.length}
          </button>
          {STATUSES.map((s) => {
            const n = all.filter((a) => a.status === s).length;
            return n ? (
              <button key={s} className={`chip ${status === s ? "acc" : ""}`} onClick={() => setStatus(s)}>
                {s} {n}
              </button>
            ) : null;
          })}
        </div>
      )}

      {assets.loading && !assets.data ? null : all.length === 0 ? (
        t ? <EmptyType slug={slug} t={t} /> : <EmptyLibrary slug={slug} />
      ) : (
        <div className="lib-grid">
          {list.map((a, i) => (
            <AssetCard key={`${a.type}/${a.id}`} slug={slug} a={a} t={types.find((x) => x.id === a.type)} i={i} showType={!t} />
          ))}
        </div>
      )}
    </div>
  );
}

function TypeStrip({ slug, types, assets }: { slug: string; types: AssetType[]; assets: Asset[] }) {
  const counts = types.map((t) => ({ t, n: assets.filter((a) => a.type === t.id).length })).filter((x) => x.n > 0);
  return (
    <div className="lib-types">
      {counts.map(({ t, n }, i) => {
        const Icon = assetIcon(t.icon);
        return (
          <Link key={t.id} to={`/p/${slug}/library/${t.id}`} className="lib-type" style={{ animationDelay: `${i * 40}ms` }}>
            <Icon size={16} />
            <span>{t.plural}</span>
            <b>{n}</b>
          </Link>
        );
      })}
    </div>
  );
}

function AssetCard({ slug, a, t, i, showType }: { slug: string; a: Asset; t?: AssetType; i: number; showType: boolean }) {
  const Icon = assetIcon(t?.icon);
  const visual = a.files.find((f) => IMAGE.test(f.path));
  const src = visual ? api.assetFile(slug, a, visual.path) : "";
  return (
    <motion.div initial={{ opacity: 0, y: 10 }} animate={{ opacity: 1, y: 0 }} transition={{ delay: Math.min(i, 12) * 0.035, duration: 0.3 }}>
      <Link to={`/p/${slug}/library/${a.type}/${a.id}`} className="lib-card">
        <div className={`lib-thumb ${src ? "img" : ""}`}>
          {src ? <img src={src} alt="" loading="lazy" /> : <Icon size={26} />}
          {showType && (
            <span className="lib-kind">
              <Icon size={12} /> {t?.name ?? a.type}
            </span>
          )}
        </div>
        <div className="lib-body">
          <p className="nm">{a.title}</p>
          {a.summary && <p className="ds">{a.summary}</p>}
          <div className="lib-meta">
            <span className={`chip ${STATUS_TONE[a.status]}`}>{a.status}</span>
            {a.publications.map((p, k) => (
              <span key={k} className="chip ok" title={p.account}>
                <Send size={11} /> {p.connector}
              </span>
            ))}
            <span className="when">
              <Maker by={a.created.by} /> {ago(a.updated_at)}
            </span>
          </div>
        </div>
      </Link>
    </motion.div>
  );
}

/** Who made it: you, Seemo in a chat, or a scheduled task. */
function Maker({ by, long }: { by: string; long?: boolean }) {
  const seemo = by === "seemo" || by.startsWith("task:") || by.startsWith("chat:");
  const label = by.startsWith("task:") ? `Task ${by.slice(5)}` : by.startsWith("chat:") ? "Seemo, in a chat" : by === "seemo" ? "Seemo" : "You";
  return (
    <span className={`maker ${seemo ? "ai" : ""}`} title={label}>
      {seemo ? <Bot size={12} /> : <User size={12} />}
      {long && label}
    </span>
  );
}

function EmptyType({ slug, t }: { slug: string; t: AssetType }) {
  const Icon = assetIcon(t.icon);
  return (
    <div className="pcard none">
      <Icon size={22} className="ac2" />
      <p className="t">No {t.plural.toLowerCase()} yet</p>
      <p className="muted">{t.id === "article" ? "Pick a content gap and Seemo adds a draft article here." : "Ask Seemo for one in a chat, or schedule a task that makes them."}</p>
      {t.id === "article" && (
        <Link className="btn p" to={`/p/${slug}/gaps`}>
          <SearchX size={15} /> Content gaps
        </Link>
      )}
    </div>
  );
}

/** `/p/:slug/library/:type/:id`: one asset, previewed by its type, with its files and where it went out. */
export function AssetPage() {
  const { slug = "", type = "", id = "" } = useParams();
  const navigate = useNavigate();
  const asset = useAsync(() => api.asset(slug, type, id), [slug, type, id]);
  const types = useAsync(() => api.assetTypes(), []);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [busy, setBusy] = useState(false);
  useEvents(slug, (e) => e.type === "asset_saved" && e.asset_type === type && e.id === id && asset.reload(), { demo: false });

  const a = asset.data;
  if (asset.loading && !a) return null;
  if (!a)
    return (
      <div className="prof">
        <div className="pcard none">
          <p className="t">This asset is gone</p>
          <Link className="btn g" to={`/p/${slug}/library`}>
            Back to the library
          </Link>
        </div>
      </div>
    );
  const t = types.data?.find((x) => x.id === a.type);
  const Icon = assetIcon(t?.icon);

  const setStatus = async (status: AssetStatus) => {
    setBusy(true);
    try {
      await api.saveAsset(slug, { ...a, status });
      asset.reload();
    } finally {
      setBusy(false);
    }
  };
  const remove = async () => {
    await api.deleteAsset(slug, a.type, a.id);
    navigate(`/p/${slug}/library/${a.type}`);
  };

  return (
    <div className="prof lib">
      <Link className="lib-back" to={`/p/${slug}/library/${a.type}`}>
        <ArrowLeft size={14} /> {t?.plural ?? "Library"}
      </Link>
      <div className="cm-head">
        <div className="lib-title">
          <span className="lib-ic">
            <Icon size={18} />
          </span>
          <div>
            <p className="cap">{t?.name ?? a.type}</p>
            <h1 className="d2">{a.title}</h1>
            {a.summary && <p>{a.summary}</p>}
          </div>
        </div>
        <div className="prof-acts">
          {confirmDelete ? (
            <>
              <span className="muted sm">Remove it and its files?</span>
              <button className="btn t" onClick={() => setConfirmDelete(false)}>
                Keep
              </button>
              <button className="btn g danger" onClick={remove}>
                <Trash2 size={14} /> Remove
              </button>
            </>
          ) : (
            <button className="btn t" onClick={() => setConfirmDelete(true)} aria-label="Remove">
              <Trash2 size={15} />
            </button>
          )}
        </div>
      </div>

      <div className="lib-detail">
        <div className="pcard lib-preview">
          <Preview slug={slug} a={a} t={t} />
        </div>
        <aside className="lib-side">
          <div className="pcard">
            <p className="cap">Status</p>
            <div className="lib-status">
              {STATUSES.map((s) => (
                <button key={s} disabled={busy} className={`chip ${a.status === s ? STATUS_TONE[s] || "acc" : ""}`} onClick={() => a.status !== s && setStatus(s)}>
                  {s}
                </button>
              ))}
            </div>
            <dl className="lib-dl">
              <dt>Made by</dt>
              <dd>
                <Maker by={a.created.by} long />
              </dd>
              <dt>Created</dt>
              <dd>{new Date(a.created.at).toLocaleString()}</dd>
              <dt>Changed</dt>
              <dd>{ago(a.updated_at)}</dd>
              {a.tags.length > 0 && (
                <>
                  <dt>Tags</dt>
                  <dd className="tags">
                    {a.tags.map((x) => (
                      <span key={x} className="chip">
                        {x}
                      </span>
                    ))}
                  </dd>
                </>
              )}
              {a.source && Object.keys(a.source).length > 0 && (
                <>
                  <dt>Made from</dt>
                  <dd>
                    {Object.entries(a.source).map(([k, v]) => (
                      <span key={k} className="src">
                        {k}: {String(v)}
                      </span>
                    ))}
                  </dd>
                </>
              )}
            </dl>
          </div>
          <div className="pcard">
            <p className="cap">Published</p>
            {a.publications.length === 0 ? (
              <p className="muted sm">Not yet. Once connectors are set up, Seemo publishes from here or from a scheduled task.</p>
            ) : (
              <ul className="lib-pubs">
                {a.publications.map((p, i) => (
                  <li key={i}>
                    <Send size={13} />
                    <span>
                      <b>{p.connector}</b> {p.account}
                      <small>{new Date(p.at).toLocaleString()}</small>
                    </span>
                    {p.url && (
                      <a href={p.url} target="_blank" rel="noreferrer" aria-label="Open">
                        <ExternalLink size={13} />
                      </a>
                    )}
                  </li>
                ))}
              </ul>
            )}
          </div>
          <div className="pcard">
            <p className="cap">Files</p>
            {a.files.length === 0 ? (
              <p className="muted sm">No files yet.</p>
            ) : (
              <ul className="lib-files">
                {a.files.map((f) => (
                  <li key={f.path}>
                    <FileText size={13} />
                    <span className="mono">{f.path}</span>
                    <small>
                      {f.role} · {size(f.bytes)}
                    </small>
                    <a href={api.assetFile(slug, a, f.path)} target="_blank" rel="noreferrer" aria-label={`Open ${f.path}`}>
                      <Download size={13} />
                    </a>
                  </li>
                ))}
              </ul>
            )}
          </div>
        </aside>
      </div>
    </div>
  );
}

/** Shows what the type says to show; any type falls back to its files. */
function Preview({ slug, a, t }: { slug: string; a: Asset; t?: AssetType }) {
  const kind = t?.preview ?? "files";
  const image = a.files.find((f) => IMAGE.test(f.path));
  const video = a.files.find((f) => VIDEO.test(f.path));
  const audio = a.files.find((f) => AUDIO.test(f.path));
  const text = a.files.find((f) => TEXT.test(f.path) && ["body", "caption", "script", "lyrics", "campaign"].includes(f.role)) ?? a.files.find((f) => TEXT.test(f.path));
  const [body, setBody] = useState<string | null>(null);
  useEffect(() => {
    setBody(null);
    if (text) api.assetText(slug, a, text.path).then(setBody, () => setBody(""));
  }, [slug, a.type, a.id, text?.path, text?.sha256]); // eslint-disable-line react-hooks/exhaustive-deps

  const visual =
    video && kind === "video" ? (
      <video src={api.assetFile(slug, a, video.path)} controls className="lib-media" />
    ) : image ? (
      <img src={api.assetFile(slug, a, image.path)} alt="" className={`lib-media ${kind === "article" ? "cover" : ""}`} />
    ) : null;

  return (
    <>
      {visual}
      {audio && <audio src={api.assetFile(slug, a, audio.path)} controls className="lib-audio" />}
      {text && body ? (
        /\.md$/i.test(text.path) && kind !== "post" ? (
          <Markdown text={body} />
        ) : (
          <div className={`lib-text ${kind === "post" ? "post" : ""} ${/\.(ya?ml|json)$/i.test(text.path) ? "mono" : ""}`}>{body}</div>
        )
      ) : null}
      {!visual && !audio && !text && <p className="muted sm">Nothing to preview yet. Its files are listed beside it.</p>}
    </>
  );
}

/** Enough Markdown for drafts: headings, paragraphs, lists, quotes, bold, italics and code. */
function Markdown({ text }: { text: string }) {
  const blocks = text.replace(/\r/g, "").split(/\n{2,}/);
  return (
    <div className="lib-md">
      {blocks.map((b, i) => {
        const h = /^(#{1,4})\s+(.*)$/.exec(b.trim());
        if (h) {
          const H = (["h1", "h2", "h3", "h4"] as const)[h[1].length - 1];
          return <H key={i}>{inline(h[2])}</H>;
        }
        const lines = b.split("\n");
        if (lines.every((l) => /^\s*([-*]|\d+\.)\s+/.test(l))) {
          const L = /^\s*\d+\./.test(lines[0]) ? "ol" : "ul";
          return <L key={i}>{lines.map((l, k) => <li key={k}>{inline(l.replace(/^\s*([-*]|\d+\.)\s+/, ""))}</li>)}</L>;
        }
        if (lines.every((l) => l.startsWith(">"))) return <blockquote key={i}>{inline(lines.map((l) => l.replace(/^>\s?/, "")).join(" "))}</blockquote>;
        if (b.startsWith("```")) return <pre key={i}>{b.replace(/^```\w*\n?|```$/g, "")}</pre>;
        return <p key={i}>{inline(lines.join(" "))}</p>;
      })}
    </div>
  );
}

function inline(s: string): React.ReactNode[] {
  return s.split(/(\*\*[^*]+\*\*|`[^`]+`|\*[^*]+\*)/g).map((part, i) =>
    part.startsWith("**") ? <b key={i}>{part.slice(2, -2)}</b> : part.startsWith("`") ? <code key={i}>{part.slice(1, -1)}</code> : part.startsWith("*") && part.length > 2 ? <em key={i}>{part.slice(1, -1)}</em> : part,
  );
}

function size(n: number) {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(n < 10240 ? 1 : 0)} KB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}
