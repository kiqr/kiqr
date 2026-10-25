// The library: everything made with Seemo, by type. Types are data, so this page works for
// kinds of assets nobody has defined yet: it previews what it can and lists files otherwise.
import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { Link, useNavigate, useParams } from "react-router-dom";
import { AnimatePresence, motion } from "motion/react";
import { ArrowLeft, Bot, Download, FileText, Library as LibraryIcon, Maximize2, Rocket, SearchX, Send, Trash2, User, X } from "lucide-react";
import { api, useAsync, useEvents, type Asset, type AssetStatus, type AssetType } from "../api";
import { PostCard } from "./PostCard";
import { AssetActions, StatusChip, STATUSES, STATUS_LABEL } from "./AssetActions";
import { Markdown } from "../markdown";
import { assetIcon } from "../assetIcons";
import { ago } from "./Project";
import { EmptyLibrary, Guide } from "./LibraryEmpty";
import "../profile.css";
import "../library.css";

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
  // Cards without an image take the brand's accent, so the grid still looks like the business.
  const kit = useAsync(() => api.brandKit(slug).catch(() => null), [slug]);
  const brand = useAsync(() => api.brand(slug), [slug]);
  const accent = kit.data?.palette?.accent;
  const [status, setStatus] = useState<AssetStatus | null>(null);
  // Posts open in a modal over the grid; the page behind stays where it was.
  const [open, setOpen] = useState<{ type: string; id: string } | null>(null);
  useEffect(() => setStatus(null), [type]);
  const t = types.find((x) => x.id === type);
  const all = assets.data ?? [];
  // Archived assets show only in the Archive, never under All.
  const live = all.filter((a) => a.status !== "archived");
  const list = status ? all.filter((a) => a.status === status) : live;
  // Until the first asset exists, the library page is the getting-started page.
  const fresh = !t && !!assets.data && all.length === 0;
  const Icon = t ? assetIcon(t.icon) : fresh ? Rocket : LibraryIcon;
  const opened = open ? all.find((a) => a.type === open.type && a.id === open.id) : undefined;

  return (
    <div className="prof lib">
      <div className="cm-head">
        <div className="lib-title">
          <span className="lib-ic">
            <Icon size={18} />
          </span>
          <div>
            <p className="cap">{t ? "Library" : "Content"}</p>
            <h1 className="d2">{t ? t.plural : fresh ? "Getting started" : "Library"}</h1>
            <p>
              {t
                ? t.description
                : fresh
                  ? "A few steps from your website to your first posts. Everything you and Seemo make lands here, in your library."
                  : "Everything you and Seemo have made for this project: articles, posts, images, videos and whatever else you dream up."}
            </p>
          </div>
        </div>
      </div>

      {!t && all.length > 0 && <Guide slug={slug} hideWhenComplete />}
      {!t && live.length > 0 && <TypeStrip slug={slug} types={types} assets={live} />}

      {all.length > 0 && (
        <div className="filters">
          <button className={`chip ${status === null ? "acc" : ""}`} onClick={() => setStatus(null)}>
            All {live.length}
          </button>
          {STATUSES.map((s) => {
            const n = all.filter((a) => a.status === s).length;
            return n ? (
              <button key={s} className={`chip ${status === s ? "acc" : ""}`} onClick={() => setStatus(s)}>
                {STATUS_LABEL[s]} {n}
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
            <AssetCard key={`${a.type}/${a.id}`} slug={slug} a={a} t={types.find((x) => x.id === a.type)} i={i} showType={!t} accent={accent} onOpen={types.find((x) => x.id === a.type)?.preview === "post" ? () => setOpen({ type: a.type, id: a.id }) : undefined} />
          ))}
        </div>
      )}
      <AnimatePresence>{opened && <PostModal slug={slug} a={opened} brand={brand.data ?? null} accent={accent} onClose={() => setOpen(null)} onChange={assets.reload} />}</AnimatePresence>
    </div>
  );
}

/** A post over the library: the feed card, its status, and where to send it. Esc or the backdrop closes it. */
function PostModal({ slug, a, brand, accent, onClose, onChange }: { slug: string; a: Asset; brand: import("../api").Brand | null; accent?: string; onClose: () => void; onChange: () => void }) {
  useEffect(() => {
    const key = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", key);
    const prev = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      window.removeEventListener("keydown", key);
      document.body.style.overflow = prev;
    };
  }, [onClose]);
  // A portal: the page content animates with a transform, which would otherwise pin a fixed backdrop to it.
  return createPortal(
    <motion.div className="lib-backdrop" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} transition={{ duration: 0.18 }} onClick={onClose} role="dialog" aria-modal aria-label={a.title}>
      <motion.div className="lib-modal" initial={{ opacity: 0, y: 14, scale: 0.985 }} animate={{ opacity: 1, y: 0, scale: 1 }} exit={{ opacity: 0, y: 8, scale: 0.99 }} transition={{ duration: 0.22, ease: [0.2, 0.8, 0.2, 1] }} onClick={(e) => e.stopPropagation()}>
        <div className="stage">
          <PostCard slug={slug} a={a} brand={brand} accent={accent} />
        </div>
        <aside className="side">
          <button className="x" onClick={onClose} aria-label="Close">
            <X size={16} />
          </button>
          <div>
            <p className="cap">Post</p>
            <h2>{a.title}</h2>
            {a.summary && <p className="muted sm">{a.summary}</p>}
          </div>
          <div>
            <p className="cap">Status</p>
            <StatusChip a={a} />
            <p className="muted sm by">
              <Maker by={a.created.by} long /> · {ago(a.updated_at)}
            </p>
          </div>
          <div>
            <p className="cap">{a.status === "published" ? "Published" : a.status === "draft" ? "Approve" : "Publish"}</p>
            <AssetActions slug={slug} a={a} onChange={onChange} />
          </div>
          <Link className="btn g full" to={`/p/${slug}/library/${a.type}/${a.id}`}>
            <Maximize2 size={14} /> Open the page
          </Link>
        </aside>
      </motion.div>
    </motion.div>,
    document.body,
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

function AssetCard({ slug, a, t, i, showType, accent, onOpen }: { slug: string; a: Asset; t?: AssetType; i: number; showType: boolean; accent?: string; onOpen?: () => void }) {
  const Icon = assetIcon(t?.icon);
  const visual = a.files.find((f) => IMAGE.test(f.path));
  const src = visual ? api.assetFile(slug, a, visual.path) : "";
  const tint = !src && accent ? ({ "--brand": accent } as React.CSSProperties) : undefined;
  return (
    <motion.div initial={{ opacity: 0, y: 10 }} animate={{ opacity: 1, y: 0 }} transition={{ delay: Math.min(i, 12) * 0.035, duration: 0.3 }}>
      <Link
        to={`/p/${slug}/library/${a.type}/${a.id}`}
        className="lib-card"
        onClick={(e) => {
          // A plain click opens the modal; modifier clicks still open the page in a new tab.
          if (onOpen && !e.metaKey && !e.ctrlKey && !e.shiftKey && e.button === 0) {
            e.preventDefault();
            onOpen();
          }
        }}
      >
        <div className={`lib-thumb ${src ? "img" : ""}`} style={tint}>
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
            <StatusChip a={a} />
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

/** Who made it: you, Seemo in a chat, or a scheduled task. Chats and tasks link to where it happened. */
function Maker({ by, long }: { by: string; long?: boolean }) {
  const { slug = "" } = useParams();
  const seemo = by === "seemo" || by.startsWith("task:") || by.startsWith("chat:");
  const label = by.startsWith("task:") ? `Seemo, in a task` : by.startsWith("chat:") ? "Seemo, in a chat" : by === "seemo" ? "Seemo" : "You";
  const to = by.startsWith("task:") ? `/p/${slug}/tasks#${by.slice(5)}` : by.startsWith("chat:") ? `/p/${slug}/chats/${by.slice(5)}` : null;
  const inner = (
    <>
      {seemo ? <Bot size={12} /> : <User size={12} />}
      {long && label}
    </>
  );
  return to && long ? (
    <Link className={`maker ${seemo ? "ai" : ""}`} title={label} to={to}>
      {inner}
    </Link>
  ) : (
    <span className={`maker ${seemo ? "ai" : ""}`} title={label}>
      {inner}
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
        <Link className="btn p" to={`/p/${slug}/content#gaps`}>
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
  const brand = useAsync(() => api.brand(slug), [slug]);
  const kit = useAsync(() => api.brandKit(slug).catch(() => null), [slug]);
  const [confirmDelete, setConfirmDelete] = useState(false);
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
        {t?.preview === "post" ? (
          <div className="post-stage">
            <PostCard slug={slug} a={a} brand={brand.data ?? null} accent={kit.data?.palette?.accent} />
          </div>
        ) : (
          <div className="pcard lib-preview">
            <Preview slug={slug} a={a} t={t} />
          </div>
        )}
        <aside className="lib-side">
          <div className="pcard">
            <p className="cap">Status</p>
            <StatusChip a={a} />
            <dl className="lib-dl">
              <dt>Made by</dt>
              <dd>
                <Maker by={a.created.by} long />
              </dd>
              <dt>Created</dt>
              <dd>{new Date(a.created.at).toLocaleString()}</dd>
              <dt>Changed</dt>
              <dd>{ago(a.updated_at)}</dd>
              {typeof a.source?.campaign === "string" && (
                <>
                  <dt>Campaign</dt>
                  <dd>
                    <Link to={`/p/${slug}/plan`}>{String(a.source.campaign)}</Link>
                  </dd>
                </>
              )}
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
            <p className="cap">{a.status === "published" ? "Published" : a.status === "draft" ? "Approve" : "Publish"}</p>
            {a.status === "draft" && <p className="muted sm">Approve it for a channel; then publish it now or pick a time. Seemo can ask to publish it too, and you say yes on its card.</p>}
            <AssetActions slug={slug} a={a} onChange={asset.reload} />
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

function size(n: number) {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(n < 10240 ? 1 : 0)} KB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}
