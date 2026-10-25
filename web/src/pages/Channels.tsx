// Channels: where this project's content goes out. A channel is a connection
// that can publish plus a destination; publishing itself always asks first.
import { useState } from "react";
import { Link, useParams } from "react-router-dom";
import { Check, Loader2, Mail, Plus, Send, Sparkles, Trash2, X } from "lucide-react";
import { Switch } from "./Switch";
import { ConnectButton } from "./ConnectButton";
import { api, useAsync, useEvents, type ChannelKindInfo } from "../api";
import { ago } from "./Project";
import "../profile.css";
import "../chat.css";

export function ChannelsPage() {
  const { slug = "" } = useParams();
  const data = useAsync(() => api.channels(slug), [slug]);
  const [adding, setAdding] = useState(false);
  useEvents(slug, (e) => ["channel_saved", "channel_deleted", "asset_published"].includes(e.type) && data.reload(), { demo: false });
  useEvents(null, (e) => ["secret_set", "extension_installed", "extension_removed"].includes(e.type) && data.reload(), { demo: false });
  const list = data.data?.channels ?? [];
  // Every installed publishing ability can become a channel; the account is set up here, inside the project.
  const kinds = data.data?.kinds ?? [];
  const ready = kinds;

  return (
    <div className="prof">
      <div className="cm-head">
        <div>
          <p className="cap">Setup</p>
          <h1 className="d2">Channels</h1>
          <p className="muted">Where this project publishes: an email address, a blog, an account. The account behind a channel belongs to this project. Seemo can publish to a channel, but every send asks you first.</p>
        </div>
        {!adding && ready.length > 0 && (
          <button className="btn p" onClick={() => setAdding(true)}>
            <Plus size={15} /> New channel
          </button>
        )}
      </div>

      {adding && <ChannelForm slug={slug} kinds={ready} onDone={() => (setAdding(false), data.reload())} />}

      {data.data && list.length === 0 && !adding && (
        <div className="pcard none">
          <Send size={22} className="ac2" />
          <p className="t">No channels yet</p>
          {ready.length > 0 ? (
            <>
              <p className="muted">Add one here, or ask Seemo in a chat: “email the pricing post to me” and it proposes a channel and the send.</p>
              <button className="btn g" onClick={() => setAdding(true)}>
                <Plus size={14} /> New channel
              </button>
            </>
          ) : (
            <>
              <p className="muted">Nothing can publish yet. Install a publishing ability, such as Email, and then add a channel here with this project's own account.</p>
              <Link className="btn g" to={`/p/${slug}/abilities#marketplace`}>
                <Sparkles size={14} /> Open the abilities marketplace
              </Link>
            </>
          )}
        </div>
      )}

      <div className="channels">
        {list.map((c) => (
          <ChannelRow key={c.id} slug={slug} c={c} onChange={data.reload} />
        ))}
      </div>

      {kinds.length > 0 && (
        <p className="muted sm" style={{ marginTop: 18 }}>
          Kinds available: {kinds.map((k) => `${k.label} via ${k.connection_name}${k.ready ? "" : " (account set up when you add one)"}`).join("; ")}.{" "}
          <Link to={`/p/${slug}/abilities#marketplace`}>More in the marketplace</Link>
        </p>
      )}
    </div>
  );
}

function ChannelRow({ slug, c, onChange }: { slug: string; c: import("../api").Channel; onChange: () => void }) {
  const [confirm, setConfirm] = useState(false);
  const [busy, setBusy] = useState(false);
  const [trustBusy, setTrustBusy] = useState(false);
  const setTrust = async (own: boolean) => {
    setTrustBusy(true);
    try {
      await api.updateChannel(slug, c.id, { asks_first: !own });
      onChange();
    } finally {
      setTrustBusy(false);
    }
  };
  const remove = async () => {
    setBusy(true);
    try {
      await api.deleteChannel(slug, c.id);
      onChange();
    } finally {
      setBusy(false);
      setConfirm(false);
    }
  };
  return (
    <div className={`pcard channel ${c.ready ? "" : "off"}`}>
      <div className="row">
        <span className="ic">{c.kind === "email" ? <Mail size={16} /> : <Send size={16} />}</span>
        <div className="min0">
          <h3>
            {c.label}
            <span className="chip">{c.kind}</span>
            {!c.ready && (
              <span className="chip warn" title="Add a channel of this kind to set up the account">
                <b /> {c.connection_name}: account not set up
              </span>
            )}
          </h3>
          <p className="meta">
            <span className="mono">{c.destination_text}</span>
            <span>via {c.connection_name}</span>
            <span>
              {c.publications} published{c.last_published ? `, last ${ago(c.last_published)}` : ""}
            </span>
          </p>
          <label className="trust" title={c.asks_first ? "Every publish Seemo proposes shows a card and waits for your yes" : "Seemo publishes here when it decides to; the chat records what went out"}>
            <Switch on={!c.asks_first} disabled={trustBusy || !c.ready} onChange={(v) => void setTrust(v)} />
            <span>Seemo may publish here on its own</span>
          </label>
        </div>
        <div className="acts">
          {confirm ? (
            <>
              <button className="btn t bad" disabled={busy} onClick={() => void remove()} aria-label="Yes, remove">
                {busy ? <Loader2 size={15} className="spin" /> : <Check size={15} />}
              </button>
              <button className="btn t" onClick={() => setConfirm(false)} aria-label="Keep it">
                <X size={15} />
              </button>
            </>
          ) : (
            <button className="btn t" onClick={() => setConfirm(true)} aria-label="Remove" title="Remove">
              <Trash2 size={15} />
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

function ChannelForm({ slug, kinds, onDone }: { slug: string; kinds: ChannelKindInfo[]; onDone: () => void }) {
  const [picked, setKind] = useState(kinds[0]);
  // The page reloads the kinds when a key lands (typed or signed in); read the fresh copy.
  const kind = kinds.find((k) => k.connection === picked?.connection && k.kind === picked?.kind) ?? picked;
  const [label, setLabel] = useState("");
  const [dest, setDest] = useState<Record<string, string>>({});
  const [keys, setKeys] = useState<Record<string, string>>({});
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const fields = Object.entries(kind?.destination ?? {});
  // The account behind the channel: keys the ability needs that this project does not have yet.
  const needed = (kind?.auth ?? []).filter((a) => !a.set);
  const ok = fields.every(([f, spec]) => (spec as { optional?: boolean }).optional || (dest[f] ?? "").trim()) && needed.every((a) => a.optional || (keys[a.env] ?? "").trim());
  const save = async () => {
    if (!kind) return;
    setSaving(true);
    setError(null);
    try {
      for (const a of needed) {
        const v = (keys[a.env] ?? "").trim();
        if (v) await api.setProjectSecret(slug, a.env, v);
      }
      await api.createChannel(slug, { label, kind: kind.kind, connection: kind.connection, destination: dest });
      onDone();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setSaving(false);
    }
  };
  return (
    <div className="pcard teach-form">
      <p className="cap">New channel</p>
      {kinds.length > 1 && (
        <div className="scope">
          <span className="nm">Kind</span>
          {kinds.map((k) => (
            <button key={`${k.connection}-${k.kind}`} className={`chip ${kind?.connection === k.connection && kind?.kind === k.kind ? "acc" : ""}`} onClick={() => (setKind(k), setDest({}), setKeys({}))}>
              {k.label} via {k.connection_name}
            </button>
          ))}
        </div>
      )}
      {needed.length > 0 && kind?.connect && <ConnectButton slug={slug} ability={kind.connection} connect={kind.connect} auth={kind.auth ?? []} scope="project" />}
      {needed.length > 0 && (
        <>
          <p className="muted sm">
            The {kind?.label.toLowerCase()} account this project sends from. Saved to the project's own <code>.env</code>, never shown again.
          </p>
          <div className="grid">
            {needed.map((a) => (
              <label key={a.env}>
                <span className="nm">
                  {a.label || a.env}
                  {a.optional && <small className="dim"> optional</small>}
                </span>
                <input className="input mono" type={/PASSWORD|KEY|TOKEN|SECRET/.test(a.env) ? "password" : "text"} autoComplete="off" spellCheck={false} value={keys[a.env] ?? ""} onChange={(e) => setKeys({ ...keys, [a.env]: e.target.value })} placeholder={a.help && !/^https?:/.test(a.help) ? a.help : ""} />
                {a.help && /^https?:/.test(a.help) && (
                  <a className="ds" href={a.help} target="_blank" rel="noreferrer">
                    where to get it
                  </a>
                )}
              </label>
            ))}
          </div>
        </>
      )}
      <div className="grid">
        {fields.map(([f, spec]) => {
          const s = spec as { label?: string; help?: string; optional?: boolean };
          return (
            <label key={f}>
              <span className="nm">
                {s.label ?? f}
                {s.optional && <small className="dim"> optional</small>}
              </span>
              <input className="input" value={dest[f] ?? ""} onChange={(e) => setDest({ ...dest, [f]: e.target.value })} placeholder={s.help ?? ""} />
              {s.help && <span className="ds">{s.help}</span>}
            </label>
          );
        })}
        <label>
          <span className="nm">Name</span>
          <input className="input" value={label} placeholder={`${kind?.label ?? "Channel"} to …`} onChange={(e) => setLabel(e.target.value)} />
          <span className="ds">Optional. How it shows up here and in chats.</span>
        </label>
      </div>
      {error && <p className="err">{error}</p>}
      <div className="acts">
        <button className="btn p" disabled={!ok || saving} onClick={() => void save()}>
          {saving ? <Loader2 size={14} className="spin" /> : <Check size={14} />} Add the channel
        </button>
        <button className="btn t" onClick={onDone}>
          Cancel
        </button>
      </div>
    </div>
  );
}
