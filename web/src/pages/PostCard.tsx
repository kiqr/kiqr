// A post as it will look in the feed: the brand as the author, the text with
// its links and hashtags lit, the image below, and a quiet action row. Used
// on the asset page and in the library's post modal.
import { useEffect, useState } from "react";
import { Bookmark, Heart, MessageCircle, Repeat2 } from "lucide-react";
import { api, type Asset, type Brand } from "../api";
import { ago } from "./Project";

const IMAGE = /\.(png|jpe?g|webp|gif|svg|avif)$/i;
const VIDEO = /\.(mp4|webm|mov)$/i;
/** How each network is named, and its length limit when it has one. */
const NETWORKS: Record<string, { name: string; limit?: number }> = {
  x: { name: "X", limit: 280 },
  twitter: { name: "X", limit: 280 },
  linkedin: { name: "LinkedIn", limit: 3000 },
  instagram: { name: "Instagram", limit: 2200 },
  facebook: { name: "Facebook" },
  threads: { name: "Threads", limit: 500 },
  tiktok: { name: "TikTok", limit: 2200 },
  reddit: { name: "Reddit" },
  bluesky: { name: "Bluesky", limit: 300 },
  mastodon: { name: "Mastodon", limit: 500 },
  pinterest: { name: "Pinterest" },
  youtube: { name: "YouTube" },
};

export function networkOf(a: Pick<Asset, "tags" | "summary">) {
  const tag = a.tags.map((t) => t.toLowerCase()).find((t) => NETWORKS[t]);
  if (tag) return { id: tag, ...NETWORKS[tag] };
  const m = /^for (\w+)/i.exec(a.summary ?? "");
  const id = m?.[1]?.toLowerCase() ?? "";
  return NETWORKS[id] ? { id, ...NETWORKS[id] } : null;
}

/** Links, hashtags and handles in the brand's colour, the rest as typed. */
function Lit({ text }: { text: string }) {
  const parts = text.split(/(https?:\/\/[^\s)]+|(?<![\w/])[#@][\p{L}\p{N}_]+|\b[\w-]+\.(?:se|com|io|dev|org|net|ai|co|app)\b(?:\/\S*)?)/u);
  return (
    <>
      {parts.map((p, i) =>
        i % 2 === 1 ? (
          <span key={i} className="tag">
            {p}
          </span>
        ) : (
          p
        ),
      )}
    </>
  );
}

export function PostCard({ slug, a, brand, accent, body }: { slug: string; a: Asset; brand: Brand | null; accent?: string; body?: string | null }) {
  const text = a.files.find((f) => /\.(md|txt)$/i.test(f.path) && f.role === "body") ?? a.files.find((f) => /\.(md|txt)$/i.test(f.path));
  const [own, setOwn] = useState<string | null>(null);
  useEffect(() => {
    if (body !== undefined) return;
    setOwn(null);
    if (text) api.assetText(slug, a, text.path).then(setOwn, () => setOwn(""));
  }, [slug, a.type, a.id, text?.path, text?.sha256, body]); // eslint-disable-line react-hooks/exhaustive-deps
  const content = (body !== undefined ? body : own) ?? "";
  const image = a.files.find((f) => IMAGE.test(f.path));
  const video = a.files.find((f) => VIDEO.test(f.path));
  const net = networkOf(a);
  const name = brand?.site_name?.trim() || slug;
  const avatar = brand?.icon || brand?.logo || null;
  const handle = `@${name.toLowerCase().replace(/[^a-z0-9]+/g, "")}`;
  const n = [...content.trim()].length;
  const over = net?.limit ? n > net.limit : false;
  return (
    <article className={`post ${net?.id ?? ""}`} style={accent ? ({ "--brand": accent } as React.CSSProperties) : undefined}>
      <header className="post-head">
        <span className="post-av">{avatar ? <img src={avatar} alt="" /> : name.slice(0, 1).toUpperCase()}</span>
        <div className="post-who">
          <p className="nm">{name}</p>
          <p className="ds">
            {handle} · {a.status === "published" ? ago(a.updated_at) : "not published yet"}
          </p>
        </div>
        {net && <span className="post-net">{net.name}</span>}
      </header>
      <div className="post-body">{content ? <Lit text={content.trim()} /> : <span className="dim">…</span>}</div>
      {video ? <video src={api.assetFile(slug, a, video.path)} controls className="post-media" /> : image ? <img src={api.assetFile(slug, a, image.path)} alt="" className="post-media" /> : null}
      <footer className="post-foot" aria-hidden>
        <Heart size={16} />
        <MessageCircle size={16} />
        <Repeat2 size={16} />
        <Bookmark size={16} />
        {n > 0 && (
          <span className={`count ${over ? "over" : ""}`} title={over ? `Over ${net?.name}'s limit` : "Characters"}>
            {n}
            {net?.limit ? ` / ${net.limit}` : ""}
          </span>
        )}
      </footer>
    </article>
  );
}
