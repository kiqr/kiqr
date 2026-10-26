// Accounts: a project's setups of one installed ability, each with its own
// keys (two WordPress sites, two analytics properties). For an ability that
// publishes, every account is a channel. Used by the ability window and the
// Channels page; installing an ability adds none, the project adds them here.
import { useEffect, useState } from "react";
import { Check, Loader2, Pencil, Plus, Star, Trash2, X } from "lucide-react";
import { api, useEvents, type Account, type ConnectInfo, type KindInfo } from "../api";
import { ConnectButton } from "./ConnectButton";
import { SecretField } from "./SecretField";
import "../settings.css";

/** What an add form needs to know about the ability. */
export type AbilityInfo = { id: string; name: string; auth: { env: string; label: string; help: string; optional: boolean }[]; connect?: ConnectInfo | null };

export const ACCOUNT_EVENTS = ["connection_saved", "connection_deleted", "secret_set", "connect_finished"];

/** The account's keys: sign in again with the service's button, or type them. */
export function AccountKeys({ slug, a, onChange }: { slug: string; a: Account; onChange: () => void }) {
  return (
    <div className="acc-keys">
      {a.connect && (
        <>
          <ConnectButton slug={slug} ability={a.ability} account={a.id} connect={a.connect} auth={a.auth} label={a.ready ? "Connect again" : a.connect.label} onDone={onChange} />
          <p className="ab-or">Or enter the keys yourself</p>
        </>
      )}
      <div className="set-keys">
        {a.auth.map((k) => (
          <SecretField key={k.env} name={k.env} label={k.label || k.env} help={k.help} set={k.set} optional={k.optional} slug={slug} account={a.id} onSaved={onChange} />
        ))}
      </div>
    </div>
  );
}

/** One account in a list: its name, whether it works, the default, its keys, remove. */
export function AccountCard({ slug, a, several, onChange }: { slug: string; a: Account; several: boolean; onChange: () => void }) {
  const [open, setOpen] = useState(!a.ready);
  const [naming, setNaming] = useState(false);
  const [name, setName] = useState(a.label);
  const [confirm, setConfirm] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const run = async (f: () => Promise<unknown>) => {
    setBusy(true);
    setError(null);
    try {
      await f();
      onChange();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className={`acc ${a.ready ? "" : "off"}`}>
      <div className="acc-head">
        {naming ? (
          <span className="acc-name">
            <input className="input" value={name} autoFocus onChange={(e) => setName(e.target.value)} onKeyDown={(e) => e.key === "Enter" && name.trim() && void run(() => api.updateAccount(slug, a.id, { label: name.trim() })).then(() => setNaming(false))} />
            <button className="btn t sm" disabled={busy || !name.trim()} onClick={() => void run(() => api.updateAccount(slug, a.id, { label: name.trim() })).then(() => setNaming(false))} aria-label="Save the name">
              <Check size={13} />
            </button>
            <button className="btn t sm" onClick={() => (setNaming(false), setName(a.label))} aria-label="Keep the name">
              <X size={13} />
            </button>
          </span>
        ) : (
          <button className="acc-name lnk-plain" onClick={() => setOpen((o) => !o)}>
            <b>{a.label}</b>
            {a.destination_text && <span className="mono dim">{a.destination_text}</span>}
          </button>
        )}
        <span className={`chip sm ${a.ready ? "ok" : "warn"}`}>
          <b /> {a.ready ? "connected" : "not connected"}
        </span>
        {several &&
          (a.default ? (
            <span className="chip sm acc-def" title="Seemo uses this one when it does not name another">
              <Star size={11} /> default
            </span>
          ) : (
            <button className="btn t sm" disabled={busy} title="Use this one when nothing names another" onClick={() => void run(() => api.defaultAccount(slug, a.id))}>
              <Star size={12} /> Make default
            </button>
          ))}
        <span className="grow" />
        {!naming && (
          <button className="btn t sm" onClick={() => setNaming(true)} aria-label="Rename" title="Rename">
            <Pencil size={13} />
          </button>
        )}
        {confirm ? (
          <span className="acc-confirm">
            <span className="muted sm">Remove it and its keys?</span>
            <button className="btn t sm bad" disabled={busy} onClick={() => void run(() => api.deleteAccount(slug, a.id))}>
              {busy ? <Loader2 size={13} className="spin" /> : <Check size={13} />} Yes
            </button>
            <button className="btn t sm" onClick={() => setConfirm(false)}>
              Keep
            </button>
          </span>
        ) : (
          <button className="btn t sm" onClick={() => setConfirm(true)} aria-label="Remove" title="Remove">
            <Trash2 size={13} />
          </button>
        )}
      </div>
      {open && <AccountKeys slug={slug} a={a} onChange={onChange} />}
      {!open && (
        <button className="lnk sm acc-more" onClick={() => setOpen(true)}>
          {a.ready ? "Keys and sign-in" : "Connect it"}
        </button>
      )}
      {error && <p className="err sm">{error}</p>}
    </div>
  );
}

/**
 * Add an account, or a channel when `kinds` is given (it publishes): its
 * name, a channel's destination, then its account: signed in with the
 * service's button, typed keys, or the same account as another channel.
 */
export function AccountForm({ slug, ability, kinds, shareable, onDone, onCancel }: { slug: string; ability: AbilityInfo; kinds?: KindInfo[]; shareable?: { id: string; label: string }[]; onDone: () => void; onCancel: () => void }) {
  const [kind, setKind] = useState<KindInfo | undefined>(kinds?.[0]);
  const [label, setLabel] = useState("");
  const [dest, setDest] = useState<Record<string, string>>({});
  const [share, setShare] = useState<string>("");
  const [keys, setKeys] = useState<Record<string, string>>({});
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // A sign-in adds the account as it starts; from then on the form shows that account.
  const [created, setCreated] = useState<Account | null>(null);
  const [paste, setPaste] = useState(false);
  const reloadCreated = async (id: string) => {
    const r = await api.accounts(slug, ability.id);
    const a = r.connections.find((c) => c.id === id) ?? null;
    setCreated(a);
    if (a?.ready) onDone();
  };
  useEvents(slug, (e) => created && ACCOUNT_EVENTS.includes(e.type) && void reloadCreated(created.id), { demo: false });
  const fields = Object.entries(kind?.destination ?? {}) as [string, { label?: string; help?: string; optional?: boolean }][];
  const destOk = fields.every(([f, spec]) => spec.optional || (dest[f] ?? "").trim());
  const keysOk = ability.auth.every((k) => k.optional || (keys[k.env] ?? "").trim());
  const ok = destOk && (share ? true : keysOk);
  const save = async () => {
    setSaving(true);
    setError(null);
    try {
      const secrets = share ? undefined : Object.fromEntries(Object.entries(keys).filter(([, v]) => v.trim()));
      if (kind) await api.createChannel(slug, { label, kind: kind.kind, connection: ability.id, destination: dest, secrets, same_as: share || undefined });
      else await api.createAccount(slug, { ability: ability.id, label, secrets, same_as: share || undefined });
      onDone();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setSaving(false);
    }
  };
  const noun = kind ? "channel" : "account";

  if (created)
    return (
      <div className="pcard teach-form acc-form">
        <p className="cap">
          {created.label}: {created.ready ? "connected" : "finish connecting"}
        </p>
        {!created.ready && (
          <p className="muted sm">
            {paste ? (
              "Approve in the tab that opened. It shows what KIQR needs once, because it only sends it back over https: paste it into the fields below."
            ) : (
              <>
                <Loader2 size={12} className="spin" /> Waiting for you to approve in the tab that opened.
              </>
            )}
          </p>
        )}
        <AccountKeys slug={slug} a={created} onChange={() => void reloadCreated(created.id)} />
        <div className="acts">
          <button className="btn p" onClick={onDone}>
            <Check size={14} /> Done
          </button>
        </div>
      </div>
    );

  return (
    <div className="pcard teach-form acc-form">
      <p className="cap">
        New {noun} · {ability.name}
      </p>
      {kinds && kinds.length > 1 && (
        <div className="scope">
          <span className="nm">Kind</span>
          {kinds.map((k) => (
            <button key={k.kind} className={`chip ${kind?.kind === k.kind ? "acc" : ""}`} onClick={() => (setKind(k), setDest({}))}>
              {k.label}
            </button>
          ))}
        </div>
      )}
      <div className="grid">
        <label>
          <span className="nm">Name</span>
          <input className="input" value={label} placeholder={kind ? kind.label : ability.name} onChange={(e) => setLabel(e.target.value)} />
          <span className="ds">Optional. How it shows up here and in chats.</span>
        </label>
        {fields.map(([f, s]) => (
          <label key={f}>
            <span className="nm">
              {s.label ?? f}
              {s.optional && <small className="dim"> optional</small>}
            </span>
            <input className="input" value={dest[f] ?? ""} onChange={(e) => setDest({ ...dest, [f]: e.target.value })} placeholder={s.help ?? ""} />
            {s.help && <span className="ds">{s.help}</span>}
          </label>
        ))}
      </div>
      {ability.auth.length > 0 && (
        <>
          {shareable && shareable.length > 0 && (
            <div className="scope">
              <span className="nm">Account</span>
              <button className={`chip ${!share ? "acc" : ""}`} onClick={() => setShare("")}>
                Its own
              </button>
              {shareable.map((s) => (
                <button key={s.id} className={`chip ${share === s.id ? "acc" : ""}`} onClick={() => setShare(s.id)}>
                  Same as {s.label}
                </button>
              ))}
            </div>
          )}
          {!share && (
            <>
              {ability.connect &&
                (destOk ? (
                  <ConnectButton slug={slug} ability={ability.id} connect={ability.connect} auth={ability.auth.map((k) => ({ ...k, set: null }))} fresh={{ label, kind: kind?.kind, destination: dest }} onStarted={(id, p) => (setPaste(p), void reloadCreated(id))} onDone={onDone} />
                ) : (
                  <p className="muted sm">Fill in where it goes first, then sign in with “{ability.connect.label}”.</p>
                ))}
              {ability.connect && <p className="ab-or">Or enter the keys yourself</p>}
              <div className="grid">
                {ability.auth.map((a) => (
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
              <p className="muted sm">Saved with this {noun} only, never shown again.</p>
            </>
          )}
        </>
      )}
      {error && <p className="err">{error}</p>}
      <div className="acts">
        <button className="btn p" disabled={!ok || saving} onClick={() => void save()}>
          {saving ? <Loader2 size={14} className="spin" /> : <Check size={14} />} Add the {noun}
        </button>
        <button className="btn t" onClick={onCancel}>
          Cancel
        </button>
      </div>
    </div>
  );
}

/** An ability's accounts in this project, and adding one. Channels when it publishes. */
export function AccountList({ slug, ability, kinds, onChange }: { slug: string; ability: AbilityInfo; kinds: KindInfo[]; onChange: () => void }) {
  const [list, setList] = useState<Account[] | null>(null);
  const [adding, setAdding] = useState(false);
  const load = () =>
    api
      .accounts(slug, ability.id)
      .then((r) => setList(r.connections))
      .catch(() => setList([]));
  useEffect(() => {
    void load();
  }, [slug, ability.id]);
  useEvents(slug, (e) => ACCOUNT_EVENTS.includes(e.type) && void load(), { demo: false });
  const changed = () => (void load(), onChange());
  const publishes = kinds.length > 0;
  const noun = publishes ? "channel" : "account";
  const shareable = (list ?? []).filter((a) => a.ready).map((a) => ({ id: a.id, label: a.label }));
  return (
    <div className="ab-con">
      <p className="muted sm">
        {publishes
          ? `Each channel has its own account: add one per ${kinds.map((k) => k.label.toLowerCase()).join(" or ")}. They belong to this project.`
          : "Each account has its own keys and belongs to this project. With more than one, Seemo uses the default unless it names another."}
      </p>
      {list === null && <Loader2 size={14} className="spin" />}
      {list?.map((a) => <AccountCard key={a.id} slug={slug} a={a} several={list.length > 1} onChange={changed} />)}
      {list && list.length === 0 && !adding && <p className="muted sm">No {noun} yet.</p>}
      {adding ? (
        <AccountForm slug={slug} ability={ability} kinds={publishes ? kinds : undefined} shareable={shareable} onDone={() => (setAdding(false), changed())} onCancel={() => setAdding(false)} />
      ) : (
        <button className="btn g sm acc-add" onClick={() => setAdding(true)}>
          <Plus size={13} /> Add {list && list.length ? "another" : "a"} {noun}
        </button>
      )}
    </div>
  );
}
