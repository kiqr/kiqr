// Accounts: who Seemo signs in as for this project (a Reddit login, a Google
// account, a mail sender). Each has its own keys or a password manager item,
// what Seemo may do there (its permissions) and one state by the one
// readiness rule. An account never says where content goes: that is the
// Audience, and the card only says which Audience entries it reaches. Every
// ability of the service shares the account. The pieces here are used by the
// Accounts page, the connect wizard, the ability window and Performance.
import { useEffect, useRef, useState } from "react";
import { Link } from "react-router-dom";
import { AnimatePresence, motion } from "motion/react";
import { Check, ChevronDown, ChevronRight, ExternalLink, Hand, KeyRound, LogIn, LogOut, Loader2, Pencil, Plus, RefreshCw, Star, Trash2, Users, X } from "lucide-react";
import { api, useEvents, ACCOUNT_EVENTS, LOGIN_KEYS, SERVICE_GROUPS, type Ability, type Account, type AccountActivity, type AccountState, type AccountStateWord, type Allow, type Permission, type ServiceCard, type SpecField } from "../api";
import { Hands, startUrlOf } from "./Hands";
import { Switch } from "./Switch";
import { ago } from "./Project";
import { ConnectButton } from "./ConnectButton";
import { Hint, SecretField } from "./SecretField";
import { enumOf, FieldInput } from "./FieldInput";
import { serviceIcon } from "./abilityBits";
import { ConnectService } from "./Connect";
import "../settings.css";
import "../accounts.css";

export { ACCOUNT_EVENTS };
export const LIST_EVENTS = ["list_changed", "list_deleted", "list_sending", "audience_saved", "audience_deleted"];

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
  if ((a.publishes_to ?? []).length || a.kinds?.length) return a.categories.includes("social") ? "social" : "website";
  return "tools";
}

/** The service of an account: its card, else its service id. */
export function accountService(a: Account): ServiceCard & { group: string; name: string } {
  const c = a.service_card;
  return { id: c?.id || a.service, name: c?.name || a.service, icon: c?.icon, group: c?.group || "", profile_hosts: c?.profile_hosts };
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

// ---- The one state

/** The chip for an account's state, by the one readiness rule. */
export function stateChip(s: Pick<AccountState, "state"> | AccountStateWord | undefined): { text: string; cls: string; spin?: boolean } {
  switch (typeof s === "string" ? s : s?.state) {
    case "ready":
      return { text: "Ready", cls: "ok" };
    case "signed_out":
      return { text: "Signed out", cls: "warn" };
    case "needs_hand":
      return { text: "Needs a hand", cls: "warn" };
    case "missing_keys":
      return { text: "Missing keys", cls: "warn" };
    case "needs_program":
      return { text: "Needs a program", cls: "warn" };
    case "checking":
      return { text: "Signing in", cls: "", spin: true };
    default:
      return { text: "Not checked yet", cls: "" };
  }
}

export function StateChip({ s }: { s: AccountState }) {
  const c = stateChip(s);
  return (
    <span className={`chip sm ${c.cls}`} title={s.says ? sentence(s.says) : undefined}>
      {c.spin ? <Loader2 size={11} className="spin" /> : <b />} {c.text}
    </span>
  );
}

/** "The site asks for a person: captcha" from the daemon's "the site asks for a person: captcha". */
export function sentence(m: string): string {
  const t = m.trim().replace(/[.\s]+$/, "");
  return t ? `${t.charAt(0).toUpperCase()}${t.slice(1)}` : "";
}

/** Who the account is: the name the user gave it, else what the daemon says it is (a mail address, a host), else its label. Never an ability's name. */
export function accountIdentity(a: Account): string {
  if (a.label_set) return a.label.trim();
  return a.identity || a.label.trim();
}

/** Whether the account's name is one somebody gave it, not the service's name the daemon fell back on. */
export function ownLabel(a: Account): boolean {
  return !!a.label_set;
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
  if (g === "email") return { now: "send", s: "sends" };
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

/** The one switch: every acting role that is not off goes on its own, or back to asking. On its own only ever applies in the Audience. */
export function ActOnOwn({ slug, a, onChange }: { slug: string; a: Account; onChange: () => void }) {
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
        <span>
          Let Seemo act here on its own
          <small>Only in your Audience; anywhere else Seemo asks first</small>
        </span>
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

// ---- Fields

export type Field = { key: string; label: string; help?: string; placeholder?: string; optional?: boolean; options?: string[]; secret?: boolean };

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

// ---- Where it reaches

/** "Reaches: @acme, r/selfhosted (+2)", linking to the Audience; nothing when it reaches nowhere yet. */
export function Reaches({ slug, a }: { slug: string; a: Account }) {
  const all = a.audience ?? [];
  if (!all.length) return null;
  const shown = all.slice(0, 3);
  return (
    <p className="acx-reach">
      <span className="lb">Reaches:</span>{" "}
      <Link to={`/p/${slug}/audience`} title="Open your Audience">
        {shown.map((x) => x.label).join(", ")}
        {all.length > shown.length && <span className="dim"> (+{all.length - shown.length})</span>}
      </Link>
    </p>
  );
}

// ---- Ability settings: what the project picks once for an ability (a GA4 property)

type Choice = { value: string; label: string };

/** The values a setting can take, read through the account; null while reading. */
function useChoices(slug: string, account: string | undefined, name: string, on: boolean): { list: Choice[] | null; error: string | null } {
  const [list, setList] = useState<Choice[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    if (!on || !account) return;
    let gone = false;
    setList(null);
    setError(null);
    api.accountChoices(slug, account, name).then(
      (r) => !gone && setList(r.choices),
      (e) => !gone && (setError((e as Error).message), setList([])),
    );
    return () => {
      gone = true;
    };
  }, [slug, account, name, on]);
  return { list, error };
}

/** The account an ability's settings are read through: the one it was picked with, else the service's default that works, else any. */
export function settingAccount(a: Ability): { id: string; label: string } | null {
  const all = a.accounts ?? [];
  const picked = a.setting?.account ? all.find((x) => x.id === a.setting!.account) : undefined;
  const works = (x: (typeof all)[number]) => x.works ?? x.state === "ready";
  const c = picked ?? all.find((x) => x.default && works(x)) ?? all.find(works) ?? all.find((x) => x.default) ?? all[0];
  return c ? { id: c.id, label: c.label } : a.setting?.account ? { id: a.setting.account, label: a.setting.account } : null;
}

/** Whether every required setting of an ability is picked. */
export function settingsPicked(a: Ability): boolean {
  return Object.entries(a.settings ?? {}).every(([k, f]) => f.optional || !!String(a.setting?.[k] ?? "").trim());
}

/**
 * An ability's settings, one line each: "Measured by Google · GA4 property
 * Acme site · Change", or "Pick the GA4 property" with the select read
 * through the account. Saved with the other values as they are.
 */
export function AbilitySettings({ slug, a, lead = "Through", onChange }: { slug: string; a: Ability; /** The line's first words: "Measured by". */ lead?: string; onChange: () => void }) {
  const fields = Object.entries(a.settings ?? {});
  const acc = settingAccount(a);
  if (!fields.length) return null;
  if (!acc)
    return (
      <p className="ab-setting muted sm">
        {a.name} reads through an account of its service. <Link to={connectPath(slug, { service: a.service ?? a.id })}>Connect one</Link> first.
      </p>
    );
  return (
    <>
      {fields.map(([k, f]) => (
        <SettingLine key={k} slug={slug} a={a} account={acc} name={k} f={f} lead={lead} onChange={onChange} />
      ))}
    </>
  );
}

function SettingLine({ slug, a, account, name, f, lead, onChange }: { slug: string; a: Ability; account: { id: string; label: string }; name: string; f: SpecField; lead: string; onChange: () => void }) {
  const cur = String(a.setting?.[name] ?? "");
  const [editing, setEditing] = useState(false);
  const open = editing || !cur;
  // The label of the current value, read once when the field is a list.
  const { list } = useChoices(slug, account.id, name, !!f.choices && !!cur && !editing);
  const shown = list?.find((c) => c.value === cur)?.label ?? cur;
  const label = f.label || name;
  return (
    <div className="ab-setting">
      {!open ? (
        <p className="sm">
          <span className="muted">
            {lead} {account.label} ·
          </span>{" "}
          {label} <b className="mono">{shown}</b> <span className="muted">·</span>{" "}
          <button className="lnk-btn" onClick={() => setEditing(true)}>
            Change
          </button>
        </p>
      ) : (
        <SettingPick slug={slug} a={a} account={account} name={name} f={f} cur={cur} onCancel={cur ? () => setEditing(false) : undefined} onSaved={() => (setEditing(false), onChange())} />
      )}
    </div>
  );
}

/** One setting's input: a select of what the account has (a search field when long), a select of fixed values, or typed. */
export function SettingField({ slug, account, name, f, value, onChange }: { slug: string; account: { id: string; label: string }; name: string; f: SpecField; value: string; onChange: (v: string) => void }) {
  const { list, error } = useChoices(slug, account.id, name, !!f.choices);
  const label = f.label || name;
  useEffect(() => {
    if (list?.length === 1 && !value) onChange(list[0].value);
  }, [list]);
  return (
    <label className="acc-field">
      <span className="nm">
        {label}
        {f.optional && <small className="dim"> optional</small>}
      </span>
      {f.choices && list === null ? (
        <span className="muted sm">
          <Loader2 size={12} className="spin" /> Reading what {account.label} has…
        </span>
      ) : f.choices && list && list.length ? (
        <FieldInput options={list.map((c) => c.value)} labels={Object.fromEntries(list.map((c) => [c.value, c.label]))} search={list.length > 12} optional={f.optional} value={value} onChange={onChange} ariaLabel={label} />
      ) : (
        <>
          <FieldInput options={enumOf(f)} optional={f.optional} value={value} onChange={onChange} placeholder={f.placeholder ?? ""} ariaLabel={label} />
          {f.choices && <span className="muted sm">{error ? `The list could not be read (${error}); type it instead.` : "Nothing to choose from; type it instead."}</span>}
        </>
      )}
      {f.help && (
        <span className="ds">
          <Hint text={f.help} />
        </span>
      )}
    </label>
  );
}

/** The values to save for an ability's settings: what it has, with the changes over them. */
export function settingValues(a: Ability, changes: Record<string, string> = {}): Record<string, string> {
  const out: Record<string, string> = {};
  for (const [k, v] of Object.entries(a.setting ?? {})) if (k !== "account" && v != null && String(v).trim()) out[k] = String(v);
  for (const [k, v] of Object.entries(changes)) if (v.trim()) out[k] = v.trim();
  return out;
}

/**
 * Changing one setting: which account it is read through (only when the
 * service has more than one: two Google logins), its input, Use it, Cancel.
 * Another account starts the value over: what one login sees, another may not.
 */
function SettingPick({ slug, a, account, name, f, cur, onCancel, onSaved }: { slug: string; a: Ability; account: { id: string; label: string }; name: string; f: SpecField; cur: string; onCancel?: () => void; onSaved: () => void }) {
  const all = a.accounts ?? [];
  const [through, setThrough] = useState(account.id);
  const via = all.find((x) => x.id === through) ?? account;
  const [value, setValue] = useState(cur);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const moved = through !== account.id;
  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      // Through another account, the other settings are picked again there too.
      await api.setAbilitySettings(slug, a.id, through, moved ? { [name]: value.trim() } : settingValues(a, { [name]: value }));
      onSaved();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="ab-pick">
      {!cur && <p className="sm">Pick the {f.label || name}</p>}
      {all.length > 1 && (
        <label className="acc-field">
          <span className="nm">Through</span>
          <FieldInput
            options={all.map((x) => x.id)}
            labels={Object.fromEntries(all.map((x) => [x.id, x.label]))}
            value={through}
            onChange={(v) => (setThrough(v), setValue(v === account.id ? cur : ""))}
            ariaLabel="Account"
            plain
          />
        </label>
      )}
      <SettingField key={through} slug={slug} account={{ id: via.id, label: via.label }} name={name} f={f} value={value} onChange={setValue} />
      {error && <p className="err sm">{error}</p>}
      <div className="acts">
        <button className="btn p sm" disabled={busy || !value.trim() || (value === cur && !moved)} onClick={() => void save()}>
          {busy ? <Loader2 size={13} className="spin" /> : <Check size={13} />} Use it
        </button>
        {onCancel && (
          <button className="btn t sm" disabled={busy} onClick={onCancel}>
            Cancel
          </button>
        )}
      </div>
    </div>
  );
}

// ---- Smaller parts

/** "Used by Read posts · Reply (reads, replies)": the abilities sharing it. */
export function UsedBy({ a, slug }: { a: Account; slug?: string }) {
  const by = a.used_by ?? [];
  if (!by.length) return null;
  return (
    <p className="acc-used" title="These abilities sign in once and share this account">
      <Users size={12} /> Used by{" "}
      {by.map((u, i) => (
        <span key={u.id}>
          {i > 0 && " · "}
          {slug ? <Link to={`/p/${slug}/abilities#ability=${encodeURIComponent(u.id)}`}>{u.name}</Link> : u.name}
          {u.does.length > 0 && <span className="dim"> ({u.does.map((d) => ROLE_WORDS[d] ?? d).join(", ")})</span>}
        </span>
      ))}
    </p>
  );
}

/** "Kept in KIQR", or "Kept in 1Password: Acme Reddit". */
export function SecretsLine({ a }: { a: Account }) {
  const s = a.secrets;
  return (
    <p className="muted sm acx-kept">
      <KeyRound size={12} /> {!s || s.in_kiqr ? "Kept in KIQR, on this machine" : `Kept in ${s.store_label}${s.item ? `: ${s.item}` : ""}`}
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

// The ability whose sign-in button an account of a service uses: read once per project from the abilities list.
const signInWith = new Map<string, Promise<Map<string, string>>>();
function useSignInAbility(slug: string, a: Account): string | undefined {
  const [id, setId] = useState<string | undefined>(undefined);
  useEffect(() => {
    if (!a.connect) return;
    let p = signInWith.get(slug);
    if (!p) {
      p = api.abilities(slug).then(
        (v) => new Map(v.abilities.filter((x) => x.installed && x.connect).map((x) => [serviceOf(x).id, x.id] as [string, string]).reverse()),
        () => new Map<string, string>(),
      );
      signInWith.set(slug, p);
      // Read again next time: installs change it.
      window.setTimeout(() => signInWith.delete(slug), 30_000);
    }
    let gone = false;
    void p.then((m) => !gone && setId(m.get(accountService(a).id) ?? a.used_by?.[0]?.id));
    return () => {
      gone = true;
    };
  }, [slug, a.id, !!a.connect]);
  return id;
}

/**
 * The account's keys. With a sign-in button: the button, and the keys only
 * behind "Enter the keys yourself". `only` narrows them to what is still
 * missing (pasting what the service showed).
 */
export function AccountKeys({ slug, a, onChange, only }: { slug: string; a: Account; onChange: () => void; only?: "missing" }) {
  const signedIn = a.signed_in ?? a.auth.some((k) => k.written && k.set);
  const missing = a.state.keys ?? [];
  const ability = useSignInAbility(slug, a);
  // Worked out on every render: the sign-in finishes after this mounts, and what is left must appear then.
  const [opened, setOpen] = useState(false);
  const open = opened || !a.connect || only === "missing" || (signedIn && missing.length > 0);
  const [more, setMore] = useState(false);
  const typed = a.auth.filter((k) => !k.written);
  const shown = typed.filter((k) => (only === "missing" ? missing.includes(k.env) : more || !k.optional || k.set));
  const hidden = only !== "missing" && !more && typed.some((k) => k.optional && !k.set);
  return (
    <div className="acc-keys">
      {a.connect && ability && only !== "missing" && <ConnectButton slug={slug} ability={ability} account={a.id} connect={a.connect} auth={a.auth} label={a.state.ready ? "Sign in again" : a.connect.label} onDone={onChange} />}
      {open && shown.length > 0 ? (
        <div className="set-keys">
          {shown.map((k) => (
            <SecretField key={k.env} name={k.env} label={k.label || k.env} help={k.help} placeholder={k.placeholder} set={k.set} optional={k.optional} options={k.options} choices={k.choices && signedIn} grant={a.connect?.grant} slug={slug} account={a.id} onSaved={onChange} />
          ))}
        </div>
      ) : null}
      <p className="acc-links">
        {!open && typed.length > 0 && (
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
      ) : a.state.ready ? (
        "Not read yet."
      ) : (
        "Read once it is ready."
      )}
      {a.state.ready && !learning && (
        <button className="lnk-btn" onClick={() => api.learnAccount(slug, a.id).then(() => setLearning(true), (e) => setError((e as Error).message))}>
          <RefreshCw size={11} /> Learn now
        </button>
      )}
      {error && <span className="err"> {error}</span>}
    </p>
  );
}

// ---- Signing in on a hosted program

/** The service whose site is signed in to by hand only: it has no login steps, the browser keeps the sign-in. */
export function signsInByHand(a: Account): boolean {
  return !!a.by_hand;
}

/** Sign in on the hosted program now: the account's state goes to checking, and `account_state_changed` says how it went. */
export async function signInNow(slug: string, a: Account): Promise<string | null> {
  const r = await api.accountSignIn(slug, a.id);
  return !r.started && r.note ? `${sentence(r.note)}.` : null;
}

/**
 * What the card offers for its state: Sign in when signed out, the line
 * that says why with Take the controls when a person is needed, the keys
 * when some are missing, the program it waits for.
 */
function StateFix({ slug, a, onHands, onChange }: { slug: string; a: Account; onHands: () => void; onChange: () => void }) {
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const s = a.state.state;
  const signIn = async () => {
    setBusy(true);
    setError(null);
    setNote(null);
    try {
      setNote(await signInNow(slug, a));
      onChange();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  const tail = (
    <>
      {note && <p className="muted sm">{note}</p>}
      {error && <p className="err sm">{error}</p>}
    </>
  );
  if (s === "signed_out")
    return (
      <div className="acx-fix">
        <p className="muted sm">{a.state.says ? `${sentence(a.state.says)}.` : "Signed out."}</p>
        <div className="acts">
          {a.has_session && !signsInByHand(a) ? (
            <button className="btn p sm" disabled={busy} onClick={() => void signIn()}>
              {busy ? <Loader2 size={13} className="spin" /> : <LogIn size={13} />} Sign in
            </button>
          ) : a.has_session && a.live_view ? (
            <button className="btn p sm" onClick={onHands}>
              <Hand size={13} /> Sign in in the live view
            </button>
          ) : (
            <AccountKeys slug={slug} a={a} onChange={onChange} />
          )}
        </div>
        {tail}
      </div>
    );
  if (s === "needs_hand")
    return (
      <div className="acx-hand">
        <p>
          {a.session?.message ? `${sentence(a.session.message)}. ` : ""}Finish the sign-in in the live view.
        </p>
        <div className="acts">
          {a.live_view && (
            <button className="btn p sm" onClick={onHands}>
              <Hand size={13} /> Take the controls
            </button>
          )}
          {!signsInByHand(a) && (
            <button className="btn t sm" disabled={busy} onClick={() => void signIn()}>
              {busy ? <Loader2 size={13} className="spin" /> : <LogIn size={13} />} Try again
            </button>
          )}
        </div>
        {tail}
      </div>
    );
  if (s === "missing_keys")
    return (
      <div className="acx-fix">
        <p className="muted sm">{a.signed_in ? "Signed in. One more step:" : "Sign in, or enter its keys:"}</p>
        <AccountKeys slug={slug} a={a} only={a.signed_in ? "missing" : undefined} onChange={onChange} />
      </div>
    );
  if (s === "needs_program")
    return (
      <div className="acx-fix">
        <p className="muted sm">
          {a.state.says ? `${sentence(a.state.says)}.` : "It needs a program on this computer."}{" "}
          {a.used_by?.[0] && <Link to={`/p/${slug}/abilities#ability=${encodeURIComponent(a.used_by[0].id)}`}>Install it</Link>}
        </p>
      </div>
    );
  return null;
}

/** Signed in since when, the live view, and Sign out, for an account on a hosted program. */
export function SessionBar({ slug, a, onHands, onChange }: { slug: string; a: Account; onHands: () => void; onChange: () => void }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirm, setConfirm] = useState(false);
  if (!a.has_session) return null;
  const st = a.session;
  const signOut = async () => {
    setBusy(true);
    setError(null);
    try {
      await api.accountSignOut(slug, a.id);
      setConfirm(false);
      onChange();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  return (
    <>
      <div className="acx-session">
        {a.state.state === "ready" && st?.signed_in_at ? (
          <span className="muted sm">
            Signed in {ago(st.signed_in_at)}
            {st.checked_at ? `, checked ${ago(st.checked_at)}` : ""}.
          </span>
        ) : (
          <span className="muted sm">{stateChip(a.state).text}.</span>
        )}
        <span className="grow" />
        {a.live_view && (
          <button className="btn t sm" onClick={onHands} title="See what the browser shows and drive it">
            <Hand size={13} /> Live view
          </button>
        )}
        {a.state.state !== "signed_out" &&
          (confirm ? (
            <span className="acc-confirm">
              <span className="muted sm">Sign out and forget what the browser kept?</span>
              <button className="btn t sm bad" disabled={busy} onClick={() => void signOut()}>
                {busy ? <Loader2 size={13} className="spin" /> : <Check size={13} />} Sign out
              </button>
              <button className="btn t sm" onClick={() => setConfirm(false)}>
                Keep
              </button>
            </span>
          ) : (
            <button className="btn t sm" onClick={() => setConfirm(true)}>
              <LogOut size={13} /> Sign out
            </button>
          ))}
      </div>
      {error && <p className="err sm">{error}</p>}
    </>
  );
}

const LOGIN_FIELDS: { key: (typeof LOGIN_KEYS)[number]; label: string; help?: string; secret: boolean; optional?: boolean }[] = [
  { key: "LOGIN_USERNAME", label: "User name", secret: false },
  { key: "LOGIN_PASSWORD", label: "Password", secret: true },
  { key: "LOGIN_TOTP_SECRET", label: "One-time code secret", help: "The secret your authenticator app was set up with. KIQR makes the six-digit codes from it.", secret: true, optional: true },
];

/**
 * A login saved in KIQR: a user name, a password and an optional one-time
 * code secret, saved to the account's own keys and never shown again (dots
 * say which are there).
 */
export function LoginEditor({ slug, a, onSaved, onChange }: { slug: string; a: Account; onSaved?: () => void; onChange: () => void }) {
  const set = new Set(a.login_set ?? []);
  const saved = set.has("LOGIN_USERNAME") && set.has("LOGIN_PASSWORD");
  const [editing, setEditing] = useState(!saved);
  const [values, setValues] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [confirm, setConfirm] = useState(false);
  useEffect(() => setEditing(!saved), [saved]);
  if (signsInByHand(a)) return <p className="muted sm">This site is signed in by hand in the live view; the browser keeps it.</p>;
  const has = (k: string) => set.has(k) || !!(values[k] ?? "").trim();
  const ok = has("LOGIN_USERNAME") && has("LOGIN_PASSWORD") && LOGIN_FIELDS.some((f) => (values[f.key] ?? "").trim());
  const save = async () => {
    setBusy("save");
    setError(null);
    try {
      for (const f of LOGIN_FIELDS) {
        const v = values[f.key] ?? "";
        // A password is kept as typed; the rest without stray spaces.
        if (v.trim()) await api.setAccountSecret(slug, a.id, f.key, f.key === "LOGIN_PASSWORD" ? v : v.trim());
      }
      setValues({});
      setEditing(false);
      onChange();
      onSaved?.();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(null);
    }
  };
  const forget = async () => {
    setBusy("forget");
    setError(null);
    try {
      for (const k of LOGIN_KEYS) await api.setAccountSecret(slug, a.id, k, "");
      setValues({});
      setConfirm(false);
      setEditing(true);
      onChange();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(null);
    }
  };
  return (
    <div className="acx-login">
      {saved && !editing ? (
        <>
          {LOGIN_FIELDS.map((f) => (
            <span key={f.key} className="saved">
              {f.label}: {set.has(f.key) ? <span className="dots">••••••</span> : <span className="dim">{f.optional ? "not set" : "missing"}</span>}
            </span>
          ))}
          <div className="acts">
            <button className="btn t sm" onClick={() => setEditing(true)}>
              <Pencil size={13} /> Change
            </button>
            <span className="grow" />
            {confirm ? (
              <span className="acc-confirm">
                <span className="muted sm">Forget the user name, password and code secret?</span>
                <button className="btn t sm bad" disabled={!!busy} onClick={() => void forget()}>
                  {busy === "forget" ? <Loader2 size={13} className="spin" /> : <Check size={13} />} Forget
                </button>
                <button className="btn t sm" onClick={() => setConfirm(false)}>
                  Keep
                </button>
              </span>
            ) : (
              <button className="btn t sm bad" onClick={() => setConfirm(true)}>
                <Trash2 size={13} /> Forget the login
              </button>
            )}
          </div>
        </>
      ) : (
        <>
          <p className="muted sm">Seemo never sees them: KIQR types them into the site's sign-in page itself, and only then.</p>
          {LOGIN_FIELDS.map((f) => (
            <label key={f.key} className="acc-field">
              <span className="nm">
                {f.label}
                {f.optional && <small className="dim"> optional</small>}
                {set.has(f.key) && <small className="dim"> saved; type to replace</small>}
              </span>
              <FieldInput mono={f.secret} secret={f.secret} value={values[f.key] ?? ""} onChange={(v) => setValues((cur) => ({ ...cur, [f.key]: v }))} placeholder={set.has(f.key) ? "••••••" : ""} ariaLabel={f.label} />
              {f.help && <span className="ds muted">{f.help}</span>}
            </label>
          ))}
          <div className="acts">
            <button className="btn p sm" disabled={!ok || !!busy} onClick={() => void save()}>
              {busy === "save" ? <Loader2 size={13} className="spin" /> : <Check size={13} />} Save it in KIQR
            </button>
            {saved && (
              <button className="btn t sm" onClick={() => (setEditing(false), setValues({}))}>
                Cancel
              </button>
            )}
          </div>
        </>
      )}
      {error && <p className="err sm">{error}</p>}
    </div>
  );
}

// ---- The card

/**
 * One account: the service and who it is, its state with what fixes it, one
 * line for what Seemo may do with one switch, where it reaches, and Details
 * (the full permissions, used by, where its secrets are kept, keys,
 * activity, rename, default, disconnect).
 */
export function AccountCard({ slug, a, several, highlight, onChange }: { slug: string; a: Account; several: boolean; highlight?: boolean; onChange: () => void }) {
  const svc = accountService(a);
  const Icon = serviceIcon(svc.icon, svc.group);
  const identity = accountIdentity(a);
  const [details, setDetails] = useState(false);
  const [naming, setNaming] = useState(false);
  const [name, setName] = useState(a.label_set ? a.label : "");
  const [confirm, setConfirm] = useState(false);
  const [hands, setHands] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [brokenAvatar, setBrokenAvatar] = useState(false);
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
  const ready = a.state.ready;
  return (
    <motion.div ref={ref} layout className={`pcard acx ${ready ? "" : "off"} ${highlight ? "hi" : ""}`} initial={{ opacity: 0, y: 6 }} animate={{ opacity: 1, y: 0 }}>
      <div className="acx-head">
        <span className="ic">{a.avatar && !brokenAvatar ? <img src={a.avatar} alt="" onError={() => setBrokenAvatar(true)} /> : <Icon size={17} />}</span>
        <div className="min0">
          <h3>{svc.name}</h3>
          {naming ? (
            <span className="acc-name">
              <input className="input" value={name} placeholder={identity || svc.name} autoFocus onChange={(e) => setName(e.target.value)} onKeyDown={(e) => e.key === "Enter" && rename()} />
              <button className="btn t sm" disabled={busy || !name.trim()} onClick={rename} aria-label="Save the name">
                <Check size={13} />
              </button>
              <button className="btn t sm" onClick={() => (setNaming(false), setName(a.label_set ? a.label : ""))} aria-label="Keep the name">
                <X size={13} />
              </button>
            </span>
          ) : (
            identity && (
              <p className="who" title={a.identity && a.identity !== identity ? `${identity} · ${a.identity}` : identity}>
                {identity}
              </p>
            )
          )}
        </div>
        <StateChip s={a.state} />
        {several && a.default && (
          <span className="chip sm acc-def" title="Seemo uses this one when it does not name another">
            <Star size={11} /> default
          </span>
        )}
      </div>

      <StateFix slug={slug} a={a} onHands={() => setHands(true)} onChange={onChange} />

      <div className="acx-say">
        <p>{permissionSummary(a)}.</p>
        <ActOnOwn slug={slug} a={a} onChange={onChange} />
      </div>

      <Reaches slug={slug} a={a} />
      <Learning slug={slug} a={a} onChange={onChange} />

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
            {(a.used_by?.length ?? 0) > 0 && (
              <section>
                <p className="cap">Used by</p>
                <UsedBy a={a} slug={slug} />
              </section>
            )}
            <section>
              <p className="cap">Sign-in and keys</p>
              <SecretsLine a={a} />
              {a.has_session && <SessionBar slug={slug} a={a} onHands={() => setHands(true)} onChange={onChange} />}
              {a.has_session && (a.secrets?.in_kiqr ?? true) && <LoginEditor slug={slug} a={a} onChange={onChange} />}
              {a.state.state !== "missing_keys" && (a.auth.length > 0 || a.connect) && <AccountKeys slug={slug} a={a} onChange={onChange} />}
            </section>
            {ready && (
              <section>
                <p className="cap">Activity</p>
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
      {hands && <Hands slug={slug} account={a.id} name={identity || svc.name} startUrl={startUrlOf(a.start_url, a.identity)} onClose={() => (setHands(false), onChange())} />}
    </motion.div>
  );
}

/** This project's accounts of one service, for the ability window: the cards, and Connect, the wizard's steps in place for that service. */
export function ServiceAccounts({ slug, ability, onChange }: { slug: string; ability: Ability; onChange: () => void }) {
  const svc = serviceOf(ability);
  const [adding, setAdding] = useState(false);
  const [list, setList] = useState<Account[] | null>(null);
  const load = () =>
    api
      .accounts(slug, svc.id)
      .then((r) => setList(r.accounts.filter((c) => accountService(c).id === svc.id)))
      .catch(() => setList([]));
  useEffect(() => {
    void load();
  }, [slug, svc.id]);
  useEvents(slug, (e) => ACCOUNT_EVENTS.includes(e.type) && void load(), { demo: false });
  return (
    <div className="ab-con">
      <p className="muted sm">
        {ability.name} works through your {svc.name} account. Every ability of {svc.name} shares it: sign in once, and choose there what Seemo may do.
      </p>
      {list === null && <Loader2 size={14} className="spin" />}
      {list?.map((a) => <AccountCard key={a.id} slug={slug} a={a} several={list.length > 1} onChange={load} />)}
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
