// An asset shown as it will look, with what you can do with it: a post as
// its feed card, anything else as a tile. Kept live while images land on it
// and as its status moves. Under Seemo's messages in the chat and on the To
// do page; the Library has its own grid.
import { Link } from "react-router-dom";
import { api, useAsync, useEvents, type Asset } from "../api";
import { PostCard } from "./PostCard";
import { AssetActions, StatusChip } from "./AssetActions";
import { assetIcon } from "../assetIcons";

export function AssetEmbed({ slug, type, id, compact, given, onChange }: { slug: string; type: string; id: string; compact?: boolean; given?: Asset; onChange?: () => void }) {
  const asset = useAsync(() => api.asset(slug, type, id).catch(() => null), [slug, type, id]);
  const types = useAsync(() => api.assetTypes(), []);
  const brand = useAsync(() => api.brand(slug), [slug]);
  const kit = useAsync(() => api.brandKit(slug).catch(() => null), [slug]);
  useEvents(slug, (e) => (e.type === "asset_saved" || e.type === "media_created") && e.asset_type === type && e.id === id && asset.reload(), { demo: false });
  const a: Asset | null | undefined = asset.data ?? given;
  if (!a) return null;
  const t = types.data?.find((x) => x.id === a.type);
  const reload = () => {
    asset.reload();
    onChange?.();
  };
  const Icon = assetIcon(t?.icon);
  const image = a.files.find((f) => /\.(png|jpe?g|webp|gif|svg|avif)$/i.test(f.path));
  return (
    <div className={`chat-embed ${t?.preview === "post" ? "post" : "tile"}`}>
      {t?.preview === "post" ? (
        <PostCard slug={slug} a={a} brand={brand.data ?? null} accent={kit.data?.palette?.accent} />
      ) : (
        <Link className="chat-tile" to={`/p/${slug}/library/${a.type}/${a.id}`}>
          {image ? (
            <img src={api.assetFile(slug, a, image.path)} alt="" />
          ) : (
            <span className="ic">
              <Icon size={18} />
            </span>
          )}
          <span className="min0">
            <b>{a.title}</b>
            <small>
              {t?.name ?? a.type}
              {a.summary ? ` · ${a.summary}` : ""}
            </small>
          </span>
        </Link>
      )}
      <div className="foot">
        {a.status !== "ready" && a.status !== "scheduled" && <StatusChip a={a} />}
        <AssetActions slug={slug} a={a} compact={compact ?? true} onChange={reload} />
        <Link className="open" to={`/p/${slug}/library/${a.type}/${a.id}`}>
          Open in the library
        </Link>
      </div>
    </div>
  );
}
