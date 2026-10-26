// Accounts: a project's accounts of a service, each with its own keys (two
// WordPress sites, two analytics properties), used by every ability of that
// service. For an ability that publishes, every account is a channel. Used by
// the ability window and the Channels page; installing an ability adds none,
// the project adds them here.
import { useEffect, useState } from "react";
import { Check, ExternalLink, Loader2, Pencil, Plus, Star, Trash2, Users, X } from "lucide-react";
import { api, useEvents, type Account, type AccountActivity, type ConnectInfo, type KindInfo } from "../api";
import { ago } from "./Project";
import { ConnectButton } from "./ConnectButton";
import { Hint, SecretField } from "./SecretField";
import { enumOf, FieldInput } from "./FieldInput";
import "../settings.css";

/** What an add form needs to know about the ability. */
export type AbilityInfo = { id: string; name: string; auth: { env: string; label: string; help: string; optional: boolean; options?: string[]; placeholder?: string }[]; connect?: ConnectInfo | null };

export const ACCOUNT_EVENTS = ["connection_saved", "connection_deleted", "secret_set", "connect_finished"];

const ROLE_WORDS: Record<string, string> = { publish: "publishes", read: "reads", reply: "replies", manage: "manages" };
const DONE_WORDS: Record<string, string> = { publish: "Published", reply: "Replied", manage: "Changed", read: "Read" };

/** "Used by Read posts · Reply (reads, replies)": the abilities sharing it, when more than one does. */
export function UsedBy({ a }: { a: Account }) {
  const by = a.used_by ?? [];
  if (by.length < 2) return null;
  return (
    <p className="acc-used" title="These abilities sign in once and share this account">
      <Users size={12} /> Used by{" "}
      {by.map((u, i) => (
        <span key={u.id}>
          {i > 0 && " · "}
          {u.name}
          {a.channel && u.does.length > 0 && <span className="dim"> ({u.does.map((d) => ROLE_WORDS[d] ?? d).join(", ")})</span>}
        </span>
      ))}
    </p>
  );
}

/** What the abilities did through an account, newest first, with today's count against the cap. */
export function ActivityLog({ slug, a }: { slug: string; a: Account }) {
  const [log, setLog] = useState<AccountActivity | null>(null);
  const load = () => api.accountActivity(slug, a.id).then(setLog).catch(() => setLog(null));
  useEffect(() => {
    void load();
  }, [slug, a.id]);
  useEvents(slug, (e) => ["account_acted", "action_refused"].includes(e.type) && void load(), { demo: false });
  const name = (id: string) => a.used_by?.find((u) => u.id === id)?.name ?? id;
  const items = log?.activity ?? [];
  return (
    <div className="acc-log">
      <p className="muted sm">
        {log?.today ?? 0} action{log?.today === 1 ? "" : "s"} today{log?.daily_cap != null ? ` of ${log.daily_cap} allowed` : ""}
        {log?.daily_cap != null && (a.used_by?.length ?? 0) > 1 ? ", shared by every ability that uses it" : ""}.
      </p>
      {items.length === 0 ? (
        <p className="muted sm">Nothing done through it yet.</p>
      ) : (
        <ul>
          {items.map((x, i) => (
            <li key={i}>
              <span className="when mono">{ago(x.at)}</span>
              <span>
                {DONE_WORDS[x.role] ?? x.role}
                {x.target ? <span className="mono"> {x.target}</span> : null}
                <span className="dim"> · {name(x.ability)}</span>
              </span>
              {x.url && (
                <a className="lnk sm" href={x.url} target="_blank" rel="noreferrer" aria-label="Open">
                  <ExternalLink size={12} />
                </a>
              )}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

/**
 * The account's keys. With a sign-in button: the button, and the keys only
 * behind "Enter the keys yourself". `only` narrows them to what is still
 * missing (pasting what the service showed).
 */
export function AccountKeys({ slug, a, onChange, only }: { slug: string; a: Account; onChange: () => void; only?: "missing" }) {
  const [open, setOpen] = useState(!a.connect || only === "missing");
  const [more, setMore] = useState(false);
  const shown = a.auth.filter((k) => (only === "missing" ? a.missing.includes(k.env) : more || !k.optional || k.set));
  const hidden = only !== "missing" && !more && a.auth.some((k) => k.optional && !k.set);
  return (
    <div className="acc-keys">
      {a.connect && only !== "missing" && <ConnectButton slug={slug} ability={a.ability} account={a.id} connect={a.connect} auth={a.auth} label={a.ready ? "Connect again" : a.connect.label} onDone={onChange} />}
      {open ? (
        <div className="set-keys">
          {shown.map((k) => (
            <SecretField key={k.env} name={k.env} label={k.label || k.env} help={k.help} placeholder={k.placeholder} set={k.set} optional={k.optional} options={k.options} slug={slug} account={a.id} onSaved={onChange} />
          ))}
        </div>
      ) : null}
      <p className="acc-links">
        {!open && (
          <button className="lnk-btn" onClick={() => setOpen(true)}>
            Enter the keys yourself
          </button>
        )}
        {open && hidden && (
          <button className="lnk-btn" onClick={() => setMore(true)}>
            More
          </button>
        )}
      </p>
    </div>
  );
}

/** One account in a list: its name, whether it works, the default, its keys, remove. */
export function AccountCard({ slug, a, several, onChange }: { slug: string; a: Account; several: boolean; onChange: () => void }) {
  const [open, setOpen] = useState(!a.ready);
  const [log, setLog] = useState(false);
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
      <UsedBy a={a} />
      {open && <AccountKeys slug={slug} a={a} onChange={onChange} />}
      <span className="acc-links">
        {!open && (
          <button className="lnk sm acc-more" onClick={() => setOpen(true)}>
            {a.ready ? "Keys and sign-in" : "Connect it"}
          </button>
        )}
        {a.ready && (
          <button className="lnk sm acc-more" onClick={() => setLog((o) => !o)}>
            {log ? "Hide activity" : "Activity"}
          </button>
        )}
      </span>
      {log && <ActivityLog slug={slug} a={a} />}
      {error && <p className="err sm">{error}</p>}
    </div>
  );
}

/** A short name from the first web address among the values: "whitening.se". */
function hostOf(values: string[]): string {
  for (const v of values) {
    try {
      const u = new URL(v.trim());
      if (u.hostname) return u.hostname.replace(/^www\./, "");
    } catch {
      /* not a URL */
    }
  }
  return "";
}

type Field = { key: string; label: string; help?: string; placeholder?: string; optional?: boolean; options?: string[]; secret?: boolean };

function Fields({ fields, values, onChange }: { fields: Field[]; values: Record<string, string>; onChange: (k: string, v: string) => void }) {
  return (
    <>
      {fields.map((f) => (
        <label key={f.key} className="acc-field">
          <span className="nm">
            {f.label}
            {f.optional && <small className="dim"> optional</small>}
          </span>
          <FieldInput mono={f.secret} secret={f.secret} options={f.options} optional={f.optional} value={values[f.key] ?? ""} onChange={(v) => onChange(f.key, v)} placeholder={f.placeholder ?? ""} ariaLabel={f.label} />
          {f.help && (
            <span className="ds">
              <Hint text={f.help} />
            </span>
          )}
        </label>
      ))}
    </>
  );
}

/**
 * Add an account, or a channel when `kinds` is given (it publishes). Only
 * what is needed shows: with a sign-in button, what it asks first (the site)
 * and the button; otherwise the required keys. Sharing another account, typing
 * the keys by hand, and everything optional (the name too) wait behind links.
 */
export function AccountForm({ slug, ability, kinds, shareable, onDone, onCancel }: { slug: string; ability: AbilityInfo; kinds?: KindInfo[]; shareable?: { id: string; label: string }[]; onDone: () => void; onCancel: () => void }) {
  const [kind, setKind] = useState<KindInfo | undefined>(kinds?.[0]);
  const [label, setLabel] = useState("");
  const [dest, setDest] = useState<Record<string, string>>({});
  const [keys, setKeys] = useState<Record<string, string>>({});
  const [mode, setMode] = useState<"connect" | "keys" | "share">(ability.connect ? "connect" : "keys");
  const [share, setShare] = useState<string>(shareable?.[0]?.id ?? "");
  const [more, setMore] = useState(false);
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

  const destFields: Field[] = Object.entries(kind?.destination ?? {}).map(([k, spec]) => {
    const s = spec as { label?: string; help?: string; placeholder?: string; optional?: boolean };
    return { key: k, label: s.label ?? k, help: s.help, placeholder: s.placeholder, optional: s.optional, options: enumOf(spec) };
  });
  const keyField = (a: AbilityInfo["auth"][number]): Field => ({ key: a.env, label: a.label || a.env, help: a.help, placeholder: a.placeholder, optional: a.optional, options: a.options, secret: /PASSWORD|KEY|TOKEN|SECRET/.test(a.env) });
  const asked = mode === "connect" ? ability.auth.filter((a) => ability.connect?.needs.includes(a.env)) : mode === "keys" ? ability.auth.filter((a) => !a.optional) : [];
  const optionalKeys = mode === "keys" ? ability.auth.filter((a) => a.optional) : [];
  const required = [...destFields.filter((f) => !f.optional), ...asked.map(keyField)];
  const optional = [...destFields.filter((f) => f.optional), ...optionalKeys.map(keyField)];
  const values = { ...dest, ...keys };
  const set = (k: string, v: string) => (destFields.some((f) => f.key === k) ? setDest({ ...dest, [k]: v }) : setKeys({ ...keys, [k]: v }));
  const ok = required.every((f) => (values[f.key] ?? "").trim()) && (mode !== "share" || !!share);
  const name = label.trim() || hostOf(Object.values(values));
  const noun = kind ? "channel" : "account";

  const save = async () => {
    setSaving(true);
    setError(null);
    try {
      const secrets = mode === "keys" ? Object.fromEntries(Object.entries(keys).filter(([, v]) => v.trim())) : undefined;
      const same_as = mode === "share" ? share : undefined;
      if (kind) await api.createChannel(slug, { label: name, kind: kind.kind, connection: ability.id, destination: dest, secrets, same_as });
      else await api.createAccount(slug, { ability: ability.id, label: name, secrets, same_as });
      onDone();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setSaving(false);
    }
  };

  if (created)
    return (
      <div className="pcard acc-form">
        <p className="cap">
          {created.label}: {created.ready ? "connected" : "finish connecting"}
        </p>
        {!created.ready && (
          <p className="muted sm">
            {paste ? (
              "Approve in the tab that opened. It shows what KIQR needs once, because it only sends it back over https: paste it below."
            ) : (
              <>
                <Loader2 size={12} className="spin" /> Waiting for you to approve in the tab that opened.
              </>
            )}
          </p>
        )}
        <AccountKeys slug={slug} a={created} only={paste ? "missing" : undefined} onChange={() => void reloadCreated(created.id)} />
        <div className="acts">
          <button className="btn p sm" onClick={onDone}>
            <Check size={13} /> Done
          </button>
        </div>
      </div>
    );

  const others: { id: string; label: string }[] = shareable ?? [];
  return (
    <div className="pcard acc-form">
      <p className="cap">
        New {noun} · {kind?.label ?? ability.name}
      </p>
      {kinds && kinds.length > 1 && (
        <div className="scope">
          {kinds.map((k) => (
            <button key={k.kind} className={`chip ${kind?.kind === k.kind ? "acc" : ""}`} onClick={() => (setKind(k), setDest({}))}>
              {k.label}
            </button>
          ))}
        </div>
      )}
      {mode === "share" && (
        <label className="acc-field">
          <span className="nm">Same account as</span>
          <FieldInput options={others.map((o) => o.label)} value={others.find((o) => o.id === share)?.label ?? ""} onChange={(v) => setShare(others.find((o) => o.label === v)?.id ?? "")} ariaLabel="Same account as" />
        </label>
      )}
      <Fields fields={required} values={values} onChange={set} />
      {more && (
        <>
          <label className="acc-field">
            <span className="nm">
              Name<small className="dim"> optional</small>
            </span>
            <FieldInput value={label} placeholder={hostOf(Object.values(values)) || kind?.label || ability.name} onChange={setLabel} ariaLabel="Name" />
          </label>
          <Fields fields={optional} values={values} onChange={set} />
        </>
      )}
      {error && <p className="err sm">{error}</p>}
      <div className="acts">
        {mode === "connect" && ability.connect ? (
          <ConnectButton slug={slug} ability={ability.id} connect={ability.connect} auth={ability.auth.map((k) => ({ ...k, set: null }))} values={keys} disabled={!ok} fresh={{ label: name, kind: kind?.kind, destination: dest }} onStarted={(id, p) => (setPaste(p), void reloadCreated(id))} onDone={onDone} />
        ) : (
          <button className="btn p sm" disabled={!ok || saving} onClick={() => void save()}>
            {saving ? <Loader2 size={13} className="spin" /> : <Check size={13} />} Add the {noun}
          </button>
        )}
        <button className="btn t sm" onClick={onCancel}>
          Cancel
        </button>
      </div>
      <p className="acc-links">
        {mode !== "share" && others.length > 0 && (
          <button className="lnk-btn" onClick={() => setMode("share")}>
            Same account as {others.length === 1 ? others[0].label : "another"}
          </button>
        )}
        {mode !== "keys" && ability.auth.length > 0 && (
          <button className="lnk-btn" onClick={() => setMode("keys")}>
            Enter the keys yourself
          </button>
        )}
        {mode !== "connect" && ability.connect && (
          <button className="lnk-btn" onClick={() => setMode("connect")}>
            {ability.connect.label} instead
          </button>
        )}
        {!more && (
          <button className="lnk-btn" onClick={() => setMore(true)}>
            More
          </button>
        )}
      </p>
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
  // Other abilities of the same service use these accounts too: one sign-in for all of them.
  const others = [...new Map((list ?? []).flatMap((a) => a.used_by ?? []).filter((u) => u.id !== ability.id).map((u) => [u.id, u.name])).values()];
  return (
    <div className="ab-con">
      <p className="muted sm">
        {publishes
          ? `Each channel has its own account: add one per ${kinds.map((k) => k.label.toLowerCase()).join(" or ")}. They belong to this project.`
          : "Each account has its own keys and belongs to this project. With more than one, Seemo uses the default unless it names another."}
      </p>
      {others.length > 0 && (
        <p className="acc-used">
          <Users size={12} /> Shared with {others.length === 1 ? others[0] : `${others.slice(0, -1).join(", ")} and ${others[others.length - 1]}`}: {list && list.length === 1 ? "this account is" : "these accounts are"} theirs too, signed in once.
        </p>
      )}
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
