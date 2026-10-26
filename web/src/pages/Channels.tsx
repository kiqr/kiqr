// Channels: where this project's content goes out. A channel is one account
// of a publishing ability (its own keys) plus a destination; a project can
// have as many as it wants of each kind. Publishing itself always asks first.
import { useState } from "react";
import { Link, useParams } from "react-router-dom";
import { Check, Loader2, Plus, Send, Sparkles, Trash2, X } from "lucide-react";
import { Switch } from "./Switch";
import { AccountForm, AccountKeys, ACCOUNT_EVENTS, ActivityLog, UsedBy } from "./Accounts";
import { kindIcon } from "./abilityBits";
import { api, useAsync, useEvents, type ChannelKindInfo } from "../api";
import { ago } from "./Project";
import "../profile.css";
import "../chat.css";

export function ChannelsPage() {
  const { slug = "" } = useParams();
  const data = useAsync(() => api.channels(slug), [slug]);
  const [adding, setAdding] = useState(false);
  useEvents(slug, (e) => [...ACCOUNT_EVENTS, "asset_published"].includes(e.type) && data.reload(), { demo: false });
  useEvents(null, (e) => ["extension_installed", "extension_removed"].includes(e.type) && data.reload(), { demo: false });
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
          <p className="muted">Where this project publishes: an email address, a blog, an account. Each channel has its own account, so two blogs are two channels. Seemo can publish to a channel, but every send asks you first.</p>
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
          <ChannelRow key={c.id} slug={slug} c={c} onChange={data.reload} icon={kinds.find((k) => k.kind === c.kind)?.icon} />
        ))}
      </div>

      {kinds.length > 0 && (
        <p className="muted sm" style={{ marginTop: 18 }}>
          <Link to={`/p/${slug}/abilities#marketplace`}>More places to publish to in the marketplace</Link>
        </p>
      )}
    </div>
  );
}

function ChannelRow({ slug, c, icon, onChange }: { slug: string; c: import("../api").Channel; icon?: string; onChange: () => void }) {
  const KindIcon = kindIcon(icon);
  const [confirm, setConfirm] = useState(false);
  const [busy, setBusy] = useState(false);
  const [trustBusy, setTrustBusy] = useState(false);
  const [keys, setKeys] = useState(false);
  const [log, setLog] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const setPublisher = async (id: string) => {
    setError(null);
    try {
      await api.updateChannel(slug, c.id, { publisher: id });
      onChange();
    } catch (e) {
      setError((e as Error).message);
    }
  };
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
        <span className="ic"><KindIcon size={16} /></span>
        <div className="min0">
          <h3>
            {c.label}
            <span className="chip">{c.kind}</span>
            {!c.ready && (
              <span className="chip warn" title="Its account has no keys yet">
                <b /> not connected
              </span>
            )}
          </h3>
          <p className="meta">
            <span className="mono">{c.destination_text}</span>
            {(c.publishers?.length ?? 0) > 1 ? (
              <label className="ch-pub" title="Several abilities can publish here; this one does">
                via
                <select className="input sm" value={c.publisher ?? ""} onChange={(e) => void setPublisher(e.target.value)}>
                  {c.publishers!.map((p) => (
                    <option key={p.id} value={p.id}>
                      {p.name}
                    </option>
                  ))}
                </select>
              </label>
            ) : (
              <span>via {c.publisher_name ?? c.connection_name}</span>
            )}
            <span>
              {c.publications} published{c.last_published ? `, last ${ago(c.last_published)}` : ""}
            </span>
          </p>
          <label className="trust" title={c.asks_first ? "Every publish Seemo proposes shows a card and waits for your yes" : "Seemo publishes here when it decides to; the chat records what went out"}>
            <Switch on={!c.asks_first} disabled={trustBusy || !c.ready} onChange={(v) => void setTrust(v)} />
            <span>Seemo may publish here on its own</span>
          </label>
          <UsedBy a={c} />
          {error && <p className="err sm">{error}</p>}
          <span className="acc-links">
            {c.ready && !keys && (
              <button className="lnk sm acc-more" onClick={() => setKeys(true)}>
                Account
              </button>
            )}
            {c.ready && (
              <button className="lnk sm acc-more" onClick={() => setLog((o) => !o)}>
                {log ? "Hide activity" : "Activity"}
              </button>
            )}
          </span>
          {(!c.ready || keys) && <AccountKeys slug={slug} a={c} onChange={onChange} />}
          {log && <ActivityLog slug={slug} a={c} />}
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

/** Pick a kind (and the ability behind it), then the shared form: name, destination, the channel's own account. */
function ChannelForm({ slug, kinds, onDone }: { slug: string; kinds: ChannelKindInfo[]; onDone: () => void }) {
  const [picked, setPicked] = useState(kinds[0]);
  const kind = kinds.find((k) => k.connection === picked?.connection && k.kind === picked?.kind) ?? picked;
  if (!kind) return null;
  return (
    <>
      {kinds.length > 1 && (
        <div className="scope chan-kinds">
          <span className="nm">Kind</span>
          {kinds.map((k) => (
            <button key={`${k.connection}-${k.kind}`} className={`chip ${kind.connection === k.connection && kind.kind === k.kind ? "acc" : ""}`} onClick={() => setPicked(k)}>
              {kinds.filter((o) => o.label === k.label).length > 1 ? `${k.label} (${k.connection_name})` : k.label}
            </button>
          ))}
        </div>
      )}
      <AccountForm
        key={`${kind.connection}-${kind.kind}`}
        slug={slug}
        ability={{ id: kind.connection, name: kind.connection_name, auth: (kind.auth ?? []).map((a) => ({ env: a.env, label: a.label, help: a.help, optional: a.optional, options: a.options, placeholder: a.placeholder })), connect: kind.connect }}
        kinds={[{ kind: kind.kind, label: kind.label, name: kind.name ?? kind.label, destination: kind.destination }]}
        shareable={kind.accounts}
        onDone={onDone}
        onCancel={onDone}
      />
    </>
  );
}
