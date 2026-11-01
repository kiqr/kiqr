// Accounts: everything Seemo signs in to for this project (an X account, a
// mail account, a WordPress site, an analytics property). Each has its own
// keys, what Seemo may do there (its permissions), and, where its service
// has several places to publish, its places: a subreddit, a branch, a list of
// people on a mail account. Every ability of the service shares the account.
// The pieces here are used by the Accounts page, the connect wizard and the
// ability window.
import { useEffect, useRef, useState } from "react";
import { Link } from "react-router-dom";
import { AnimatePresence, motion } from "motion/react";
import { Check, ChevronDown, ChevronRight, ExternalLink, Loader2, Pencil, Plus, RefreshCw, Star, Trash2, Users, X } from "lucide-react";
import { api, useEvents, SERVICE_GROUPS, type Ability, type Account, type AccountActivity, type Allow, type MailList, type Permission, type PlaceField, type ServiceCard } from "../api";
import { Switch } from "./Switch";
import { ago } from "./Project";
import { ConnectButton } from "./ConnectButton";
import { Hint, SecretField } from "./SecretField";
import { enumOf, FieldInput } from "./FieldInput";
import { serviceIcon } from "./abilityBits";
import { ConnectService } from "./Connect";
import "../settings.css";
import "../accounts.css";

export const ACCOUNT_EVENTS = ["connection_saved", "connection_deleted", "secret_set", "connect_finished"];
export const LIST_EVENTS = ["list_changed", "list_deleted", "list_sending"];

const ROLE_WORDS: Record<string, string> = { publish: "publishes", read: "reads", reply: "replies", manage: "manages", learn: "learns from it", measure: "reads its numbers" };
const DONE_WORDS: Record<string, string> = { publish: "Published", reply: "Replied", manage: "Changed", read: "Read" };

// ---- Services and groups

/** The service an ability belongs to, as the grid of things to connect shows it. Uses the daemon's card when it sends one. */
export function serviceOf(a: Ability): ServiceCard & { group: string; name: string } {
  const c = a.service_card;
  const group = c?.group || guessGroup(a);
  return { id: c?.id || a.service || a.id, name: c?.name || a.name, icon: c?.icon || a.icon, group, profile_hosts: c?.profile_hosts };
}

/** Where an ability without a service card sits, from what it says it does. */
function guessGroup(a: Ability): string {
  if (a.provides.some((p) => p.ability === "email") || a.categories.includes("email")) return "email";
  if (a.sources.length) return "numbers";
  if (a.channels.length) return a.categories.includes("social") ? "social" : "website";
  return "tools";
}

/** The service of an account: its card, else its ability's name. */
export function accountService(a: Account): ServiceCard & { group: string; name: string } {
  const c = a.service_card;
  return { id: c?.id || a.service || a.ability, name: c?.name || a.ability_name, icon: c?.icon || a.icon, group: c?.group || "", profile_hosts: c?.profile_hosts };
}

/** Group things by service group in the order SERVICE_GROUPS gives; unknown groups last, under "Other". */
export function byGroup<T>(items: T[], groupOf: (t: T) => string): { id: string; label: string; items: T[] }[] {
  const known = SERVICE_GROUPS.map((g) => ({ ...g, items: items.filter((t) => groupOf(t) === g.id) }));
  const rest = items.filter((t) => !SERVICE_GROUPS.some((g) => g.id === groupOf(t)));
  return [...known, { id: "other", label: "Other", items: rest }].filter((g) => g.items.length > 0);
}

/** Where the connect wizard opens, narrowed to a service or a group when given. */
export function connectPath(slug: string, at: { service?: string; group?: string } = {}) {
  const q = new URLSearchParams();
  if (at.service) q.set("service", at.service);
  if (at.group) q.set("group", at.group);
  const s = q.toString();
  return `/p/${slug}/accounts/connect${s ? `?${s}` : ""}`;
}

// ---- Permissions

const ROLE_LABEL: Record<string, string> = { read: "Read", reply: "Reply", publish: "Publish", manage: "Change settings and profile", learn: "Learn the business from it", measure: "Read its numbers into Performance" };
const ROLE_HELP: Record<string, string> = {
  read: "Look at what is there: posts, messages, pages, files.",
  reply: "Answer comments and messages.",
  publish: "Put new things out.",
  manage: "Change the account itself: its profile, its settings.",
  learn: "Read it now and then to keep the business profile current.",
  measure: "Pull its numbers once a day.",
};

/** The verb for publishing on this account, by its service: post, send or publish. */
function publishVerb(a: Account): { now: string; s: string } {
  const g = accountService(a).group;
  if (g === "email" || a.takes_lists) return { now: "send", s: "sends" };
  if (g === "social") return { now: "post", s: "posts" };
  return { now: "publish", s: "publishes" };
}
function roleVerb(a: Account, role: string): string {
  if (role === "publish") return publishVerb(a).now;
  if (role === "reply") return "reply";
  if (role === "manage") return "change things";
  return role;
}
function roleLabel(a: Account, role: string): string {
  if (role === "publish") return publishVerb(a).now === "send" ? "Send" : publishVerb(a).now === "post" ? "Post" : "Publish";
  return ROLE_LABEL[role] ?? role;
}
function joinOr(words: string[], and = "or") {
  return words.length < 2 ? (words[0] ?? "") : `${words.slice(0, -1).join(", ")} ${and} ${words[words.length - 1]}`;
}
function third(v: string) {
  return v === "reply" ? "replies" : v === "change things" ? "changes things" : `${v}s`;
}

/** The acting roles that are not off. */
function acting(a: Account): Permission[] {
  return (a.allow ?? []).filter((p) => p.acts && p.allow !== "off");
}

/** One plain line for what Seemo may do here. */
export function permissionSummary(a: Account): string {
  const on = acting(a);
  if (!on.length) return (a.allow ?? []).some((p) => p.role === "read" && p.allow === "off") ? "Seemo does nothing here" : "Seemo only reads here";
  const own = on.filter((p) => p.allow === "auto").map((p) => roleVerb(a, p.role));
  const asks = on.filter((p) => p.allow === "ask").map((p) => roleVerb(a, p.role));
  if (!asks.length) return `Seemo ${joinOr(own.map(third), "and")} here on its own`;
  if (!own.length) return `Seemo asks before it ${joinOr(asks.map(third))} here`;
  return `Seemo ${joinOr(own.map(third), "and")} here on its own and asks before it ${joinOr(asks.map(third))}`;
}

/** The one switch: every acting role that is not off goes on its own, or back to asking. */
function ActOnOwn({ slug, a, onChange }: { slug: string; a: Account; onChange: () => void }) {
  const on = acting(a);
  const own = on.length > 0 && on.every((p) => p.allow === "auto");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  if (!on.length) return null;
  const flip = async (want: boolean) => {
    setBusy(true);
    setError(null);
    try {
      await api.setAllow(slug, a.id, Object.fromEntries(on.map((p) => [p.role, want ? "auto" : "ask"])) as Record<string, Allow>);
      onChange();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  return (
    <>
      <label className="acx-own" title={own ? "Seemo acts when it decides to; each card stays in the chat as the record" : "Every action Seemo proposes waits for your yes"}>
        <Switch on={own} disabled={busy} onChange={(v) => void flip(v)} />
        <span>Let Seemo act here on its own</span>
      </label>
      {error && <p className="err sm">{error}</p>}
    </>
  );
}

/** Every role, with a segmented choice: On its own / Ask me first / Off, or On / Off for one that never acts. */
export function PermissionMatrix({ slug, a, onChange }: { slug: string; a: Account; onChange: () => void }) {
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const set = async (role: string, v: Allow) => {
    setBusy(role);
    setError(null);
    try {
      await api.setAllow(slug, a.id, { [role]: v });
      onChange();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(null);
    }
  };
  const perms = a.allow ?? [];
  if (!perms.length) return <p className="muted sm">Nothing to choose here: Seemo only reads through it.</p>;
  return (
    <div className="acx-perms">
      {perms.map((p) => {
        const choices: { v: Allow; label: string }[] = p.acts
          ? [
              { v: "auto", label: "On its own" },
              { v: "ask", label: "Ask me first" },
              { v: "off", label: "Off" },
            ]
          : [
              { v: "auto", label: "On" },
              { v: "off", label: "Off" },
            ];
        return (
          <div key={p.role} className="acx-perm">
            <div className="min0">
              <b>{roleLabel(a, p.role)}</b>
              <span className="muted sm">{p.role === "learn" && a.learn?.label ? a.learn.label : ROLE_HELP[p.role] ?? ""}</span>
            </div>
            <div className="seg" role="radiogroup" aria-label={roleLabel(a, p.role)}>
              {choices.map((c) => (
                <button key={c.v} role="radio" aria-checked={p.allow === c.v} className={p.allow === c.v ? `on ${c.v}` : ""} disabled={busy === p.role} onClick={() => p.allow !== c.v && void set(p.role, c.v)}>
                  {c.label}
                </button>
              ))}
            </div>
          </div>
        );
      })}
      {error && <p className="err sm">{error}</p>}
    </div>
  );
}

/** Choose permissions before anything is saved (the connect wizard): the same choices, kept in a map. */
export function PermissionChoices({ perms, value, verb, onChange }: { perms: { role: string; acts: boolean }[]; value: Record<string, Allow>; verb?: (role: string) => string; onChange: (v: Record<string, Allow>) => void }) {
  return (
    <div className="acx-perms">
      {perms.map((p) => {
        const cur = value[p.role] ?? (p.acts ? "ask" : "auto");
        const choices: { v: Allow; label: string }[] = p.acts
          ? [
              { v: "auto", label: "On its own" },
              { v: "ask", label: "Ask me first" },
              { v: "off", label: "Off" },
            ]
          : [
              { v: "auto", label: "On" },
              { v: "off", label: "Off" },
            ];
        return (
          <div key={p.role} className="acx-perm">
            <div className="min0">
              <b>{verb?.(p.role) ?? ROLE_LABEL[p.role] ?? p.role}</b>
              <span className="muted sm">{ROLE_HELP[p.role] ?? ""}</span>
            </div>
            <div className="seg" role="radiogroup">
              {choices.map((c) => (
                <button key={c.v} role="radio" aria-checked={cur === c.v} className={cur === c.v ? `on ${c.v}` : ""} onClick={() => onChange({ ...value, [p.role]: c.v })}>
                  {c.label}
                </button>
              ))}
            </div>
          </div>
        );
      })}
    </div>
  );
}

// ---- Places

export type Field = { key: string; label: string; help?: string; placeholder?: string; optional?: boolean; options?: string[]; secret?: boolean };

/** A kind's place fields as form fields, labels from the manifest. */
export function placeFields(spec: Record<string, PlaceField> | undefined): Field[] {
  return Object.entries(spec ?? {}).map(([k, s]) => ({ key: k, label: s.label || k, help: s.help, placeholder: s.placeholder, optional: s.optional, options: enumOf(s) }));
}

export function Fields({ fields, values, onChange }: { fields: Field[]; values: Record<string, string>; onChange: (k: string, v: string) => void }) {
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

/** Whether an account's places are lists of people: the daemon says so, or a place already is one. */
export function takesLists(a: Account): boolean {
  return !!a.takes_lists || (a.places ?? []).some((p) => p.kind === "list");
}

/** "Newsletter (412)": a place's label, with the list's count when it is a list. */
export function placeText(p: { kind: string; label: string; fields?: Record<string, unknown> }, lists: MailList[]) {
  if (p.kind !== "list") return p.label;
  const l = lists.find((x) => x.id === p.fields?.list);
  return l ? `${p.label} (${l.contacts - l.unsubscribed})` : p.label;
}

/** The places inside an account, with add and remove: "Publishes to: r/rust, r/selfhosted", or for a mail account "Sends to: Newsletter (412)". */
function Places({ slug, a, lists, onChange }: { slug: string; a: Account; lists: MailList[]; onChange: () => void }) {
  const isMail = takesLists(a);
  const fields = placeFields(a.place_fields);
  const [adding, setAdding] = useState(false);
  const [values, setValues] = useState<Record<string, string>>({});
  const [list, setList] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const places = a.places ?? [];
  // Only a service with somewhere inside it to choose has places.
  if (!a.channel || (!isMail && fields.length === 0 && places.length === 0)) return null;
  const run = async (what: string, f: () => Promise<unknown>) => {
    setBusy(what);
    setError(null);
    try {
      await f();
      setAdding(false);
      setValues({});
      setList("");
      onChange();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(null);
    }
  };
  const free = lists.filter((l) => !places.some((p) => p.fields?.list === l.id));
  const ok = isMail ? !!list : fields.every((f) => f.optional || (values[f.key] ?? "").trim()) && fields.some((f) => (values[f.key] ?? "").trim());
  const verb = isMail ? "Sends to" : "Publishes to";
  return (
    <div className="acx-places">
      <span className="lb">{verb}:</span>
      {places.length === 0 && <span className="muted sm">{a.needs_place ? "nowhere yet; add a place before anything goes out" : isMail ? "the people named on each send" : "the account itself"}</span>}
      {places.map((p) => (
        <span key={p.id} className={`acx-place ${p.default ? "def" : ""}`} title={p.default ? "Used when nothing names another" : undefined}>
          {placeText(p, lists)}
          {!p.default && places.length > 1 && (
            <button className="x" title="Make it the default" aria-label="Make it the default" disabled={!!busy} onClick={() => void run(`def-${p.id}`, () => api.updatePlace(slug, a.id, p.id, { default: true }))}>
              <Star size={10} />
            </button>
          )}
          <button className="x" title="Remove" aria-label={`Remove ${p.label}`} disabled={!!busy} onClick={() => void run(`rm-${p.id}`, () => api.removePlace(slug, a.id, p.id))}>
            {busy === `rm-${p.id}` ? <Loader2 size={10} className="spin" /> : <X size={10} />}
          </button>
        </span>
      ))}
      {!adding && (isMail ? free.length > 0 || lists.length === 0 : fields.length > 0) && (
        <button className="lnk-btn" onClick={() => setAdding(true)}>
          <Plus size={11} /> {isMail ? "Add a list" : "Add a place"}
        </button>
      )}
      <AnimatePresence>
        {adding && (
          <motion.div className="acx-add" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: "auto" }} exit={{ opacity: 0, height: 0 }}>
            {isMail ? (
              free.length ? (
                <label className="acc-field">
                  <span className="nm">List</span>
                  <FieldInput options={free.map((l) => l.id)} labels={Object.fromEntries(free.map((l) => [l.id, `${l.label} (${l.contacts - l.unsubscribed})`]))} value={list} onChange={setList} ariaLabel="List" />
                </label>
              ) : (
                <p className="muted sm">
                  No list to add yet. <Link to={`/p/${slug}/audience`}>Make one under Audience</Link>.
                </p>
              )
            ) : (
              <Fields fields={fields} values={values} onChange={(k, v) => setValues({ ...values, [k]: v })} />
            )}
            <div className="acts">
              <button className="btn p sm" disabled={!ok || !!busy} onClick={() => void run("add", () => api.addPlace(slug, a.id, isMail ? { list } : { fields: values }))}>
                {busy === "add" ? <Loader2 size={13} className="spin" /> : <Check size={13} />} Add
              </button>
              <button className="btn t sm" onClick={() => setAdding(false)}>
                Cancel
              </button>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
      {error && <p className="err sm">{error}</p>}
    </div>
  );
}

// ---- Smaller parts

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
 * missing (pasting what the service showed, or choosing the property).
 */
export function AccountKeys({ slug, a, onChange, only }: { slug: string; a: Account; onChange: () => void; only?: "missing" }) {
  // Signed in (a token is kept) but something is still to choose (the property): open on that.
  const signedIn = a.signed_in ?? a.auth.some((k) => k.written && k.set);
  // Worked out on every render, not once: the sign-in finishes after this mounts, and the property picker must appear then.
  const [opened, setOpen] = useState(false);
  const open = opened || !a.connect || only === "missing" || (signedIn && a.missing.length > 0);
  const [more, setMore] = useState(false);
  const typed = a.auth.filter((k) => !k.written);
  const shown = typed.filter((k) => (only === "missing" ? a.missing.includes(k.env) : more || !k.optional || k.set));
  const hidden = only !== "missing" && !more && typed.some((k) => k.optional && !k.set);
  return (
    <div className="acc-keys">
      {a.connect && only !== "missing" && <ConnectButton slug={slug} ability={a.ability} account={a.id} connect={a.connect} auth={a.auth} label={a.ready ? "Connect again" : a.connect.label} onDone={onChange} />}
      {open ? (
        <div className="set-keys">
          {shown.map((k) => (
            <SecretField key={k.env} name={k.env} label={k.label || k.env} help={k.help} placeholder={k.placeholder} set={k.set} optional={k.optional} options={k.options} choices={k.choices && signedIn} slug={slug} account={a.id} onSaved={onChange} />
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

/** Connected, not connected, or signed in with something still to do. */
export function accountStatus(a: Account): { text: string; tone: "ok" | "warn" | "bad" } {
  if (a.error) return { text: "Needs a hand", tone: "bad" };
  if (a.ready) return { text: "Connected", tone: "ok" };
  if (a.signed_in) return { text: "Needs a hand", tone: "warn" };
  return { text: "Not connected", tone: "warn" };
}

/** Learning from it: when it last read, and Learn now. */
function Learning({ slug, a, onChange }: { slug: string; a: Account; onChange: () => void }) {
  const [learning, setLearning] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEvents(slug, (e) => e.type === "job_finished" && (e as { job?: string }).job === "learn" && (setLearning(false), onChange()), { demo: false });
  if (!a.can_learn && !a.learn) return null;
  if (!a.source) return null;
  return (
    <p className="muted sm acc-learned">
      {learning ? (
        <>
          <Loader2 size={12} className="spin" /> Learning from it…
        </>
      ) : a.last_learned ? (
        `Learned from ${a.learned ?? 0} document${a.learned === 1 ? "" : "s"} ${ago(a.last_learned)}; read again every week.`
      ) : a.ready ? (
        "Not read yet."
      ) : (
        "Read once it is connected."
      )}
      {a.ready && !learning && (
        <button className="lnk-btn" onClick={() => api.learnAccount(slug, a.id).then(() => setLearning(true), (e) => setError((e as Error).message))}>
          <RefreshCw size={11} /> Learn now
        </button>
      )}
      {error && <span className="err"> {error}</span>}
    </p>
  );
}

// ---- The card

/**
 * One account: the service and its identity, whether it works, one line for
 * what Seemo may do with one switch, its places, and Details (the full
 * permissions, how it connects, activity, rename, default, disconnect).
 */
export function AccountCard({ slug, a, lists, several, highlight, onChange }: { slug: string; a: Account; lists: MailList[]; several: boolean; highlight?: boolean; onChange: () => void }) {
  const svc = accountService(a);
  const Icon = serviceIcon(svc.icon, svc.group);
  const status = accountStatus(a);
  const [details, setDetails] = useState(false);
  const [keys, setKeys] = useState(!a.ready);
  const [naming, setNaming] = useState(false);
  const [name, setName] = useState(a.label);
  const [confirm, setConfirm] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (highlight) ref.current?.scrollIntoView({ block: "center", behavior: "smooth" });
  }, [highlight]);
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
  const rename = () => name.trim() && void run(() => api.updateAccount(slug, a.id, { label: name.trim() })).then(() => setNaming(false));
  return (
    <motion.div ref={ref} layout className={`pcard acx ${a.ready ? "" : "off"} ${highlight ? "hi" : ""}`} initial={{ opacity: 0, y: 6 }} animate={{ opacity: 1, y: 0 }}>
      <div className="acx-head">
        <span className="ic">
          <Icon size={17} />
        </span>
        <div className="min0">
          <p className="svc">{svc.name}</p>
          {naming ? (
            <span className="acc-name">
              <input className="input" value={name} autoFocus onChange={(e) => setName(e.target.value)} onKeyDown={(e) => e.key === "Enter" && rename()} />
              <button className="btn t sm" disabled={busy || !name.trim()} onClick={rename} aria-label="Save the name">
                <Check size={13} />
              </button>
              <button className="btn t sm" onClick={() => (setNaming(false), setName(a.label))} aria-label="Keep the name">
                <X size={13} />
              </button>
            </span>
          ) : (
            <h3>{a.label}</h3>
          )}
        </div>
        <span className={`chip sm ${status.tone}`}>
          <b /> {status.text}
        </span>
        {several && a.default && (
          <span className="chip sm acc-def" title="Seemo uses this one when it does not name another">
            <Star size={11} /> default
          </span>
        )}
      </div>

      <div className="acx-say">
        <p>{permissionSummary(a)}.</p>
        <ActOnOwn slug={slug} a={a} onChange={onChange} />
      </div>

      <Places slug={slug} a={a} lists={lists} onChange={onChange} />
      <Learning slug={slug} a={a} onChange={onChange} />

      {!a.ready && (
        <div className="acx-fix">
          <p className="muted sm">{a.error ? `It cannot be used: ${a.error}.` : a.signed_in ? "Signed in. One more step:" : "Not connected yet. Sign in, or enter its keys:"}</p>
          {!a.error && <AccountKeys slug={slug} a={a} only={a.signed_in ? "missing" : undefined} onChange={onChange} />}
        </div>
      )}

      <button className="acx-more" onClick={() => setDetails((d) => !d)} aria-expanded={details}>
        {details ? <ChevronDown size={13} /> : <ChevronRight size={13} />} Details
      </button>
      <AnimatePresence initial={false}>
        {details && (
          <motion.div className="acx-details" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: "auto" }} exit={{ opacity: 0, height: 0 }} transition={{ duration: 0.2 }}>
            <section>
              <p className="cap">What Seemo may do here</p>
              <PermissionMatrix slug={slug} a={a} onChange={onChange} />
            </section>
            {(a.publishers?.length ?? 0) > 1 && (
              <section>
                <p className="cap">Who publishes</p>
                <label className="acx-pub">
                  <span className="muted sm">Several abilities can publish here; this one does:</span>
                  <FieldInput options={a.publishers!.map((p) => p.id)} labels={Object.fromEntries(a.publishers!.map((p) => [p.id, p.name]))} value={a.publisher ?? ""} onChange={(v) => void run(() => api.updateAccount(slug, a.id, { publisher: v }))} ariaLabel="The ability that publishes here" />
                </label>
              </section>
            )}
            <section>
              <p className="cap">How it connects</p>
              <UsedBy a={a} />
              {a.ready && !keys ? (
                <button className="lnk-btn" onClick={() => setKeys(true)}>
                  Keys and sign-in
                </button>
              ) : a.ready ? (
                <AccountKeys slug={slug} a={a} onChange={onChange} />
              ) : (
                <p className="muted sm">See above.</p>
              )}
            </section>
            {a.ready && (
              <section>
                <p className="cap">Activity</p>
                <p className="muted sm">
                  {a.publications} published{a.last_published ? `, last ${ago(a.last_published)}` : ""}.
                </p>
                <ActivityLog slug={slug} a={a} />
              </section>
            )}
            <div className="acx-acts">
              {!naming && (
                <button className="btn t sm" onClick={() => setNaming(true)}>
                  <Pencil size={13} /> Rename
                </button>
              )}
              {several && !a.default && (
                <button className="btn t sm" disabled={busy} title="Seemo uses this one when nothing names another" onClick={() => void run(() => api.defaultAccount(slug, a.id))}>
                  <Star size={13} /> Make default
                </button>
              )}
              <span className="grow" />
              {confirm ? (
                <span className="acc-confirm">
                  <span className="muted sm">Disconnect it and forget its keys?</span>
                  <button className="btn t sm bad" disabled={busy} onClick={() => void run(() => api.deleteAccount(slug, a.id))}>
                    {busy ? <Loader2 size={13} className="spin" /> : <Check size={13} />} Disconnect
                  </button>
                  <button className="btn t sm" onClick={() => setConfirm(false)}>
                    Keep
                  </button>
                </span>
              ) : (
                <button className="btn t sm bad" onClick={() => setConfirm(true)}>
                  <Trash2 size={13} /> Disconnect
                </button>
              )}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
      {error && <p className="err sm">{error}</p>}
    </motion.div>
  );
}

/** This project's accounts of one service, for the ability window: the cards, and Connect, the wizard's steps in place for that service. */
export function ServiceAccounts({ slug, ability, onChange }: { slug: string; ability: Ability; onChange: () => void }) {
  const svc = serviceOf(ability);
  const [adding, setAdding] = useState(false);
  const [list, setList] = useState<Account[] | null>(null);
  const [lists, setLists] = useState<MailList[]>([]);
  const load = () =>
    api
      .accounts(slug)
      .then((r) => setList(r.connections.filter((c) => accountService(c).id === svc.id || c.ability === ability.id)))
      .catch(() => setList([]));
  useEffect(() => {
    void load();
    api.lists(slug).then((r) => setLists(r.lists), () => setLists([]));
  }, [slug, svc.id]);
  useEvents(slug, (e) => ACCOUNT_EVENTS.includes(e.type) && void load(), { demo: false });
  return (
    <div className="ab-con">
      <p className="muted sm">
        {ability.name} works through your {svc.name} account. Every ability of {svc.name} shares it: sign in once, and choose there what Seemo may do.
      </p>
      {list === null && <Loader2 size={14} className="spin" />}
      {list?.map((a) => <AccountCard key={a.id} slug={slug} a={a} lists={lists} several={list.length > 1} onChange={load} />)}
      {list && list.length === 0 && !adding && <p className="muted sm">No {svc.name} account in this project yet.</p>}
      {adding ? (
        <ConnectService slug={slug} ability={ability} onInstalled={onChange} onCancel={() => setAdding(false)} onDone={() => (setAdding(false), void load(), onChange())} />
      ) : (
        <button className={`btn ${list && list.length ? "g" : "p"} sm acc-add`} onClick={() => setAdding(true)}>
          <Plus size={13} /> Connect {list && list.length ? "another" : "your"} {svc.name} account
        </button>
      )}
    </div>
  );
}
