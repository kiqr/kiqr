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
import { AlertCircle, Check, ChevronDown, ChevronRight, ExternalLink, Hand, KeyRound, LogIn, LogOut, Loader2, Pencil, Plus, RefreshCw, Star, Trash2, Users, X } from "lucide-react";
import { api, useEvents, LOGIN_KEYS, SERVICE_GROUPS, type Ability, type Account, type AccountActivity, type Allow, type MailList, type Permission, type PlaceField, type PlaceKind, type ServiceCard, type SessionStatus } from "../api";
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

export const ACCOUNT_EVENTS = ["connection_saved", "connection_deleted", "secret_set", "connect_finished", "session_changed"];
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

/** Every kind of place the account can have. The daemon sends them; an older one only the publishing kind's fields. */
export function placeKinds(a: Account): PlaceKind[] {
  const lists = takesLists(a);
  const own = (k: PlaceKind) => (k.lists || (lists && k.kind === a.kind) ? { ...k, lists: true, verb: "Sends to" } : k);
  if (a.place_kinds) return a.place_kinds.map(own);
  if (!a.channel) return [];
  return [own({ kind: a.kind, label: a.kind_label ?? "", verb: "Publishes to", fields: a.place_fields ?? {}, needs: !!a.needs_place && !(a.places ?? []).length, lists })];
}

/** What to call a place of a kind: its first field's label ("GA4 property"), else the kind's own label. */
export function placeNoun(k: PlaceKind): string {
  return Object.values(k.fields ?? {})[0]?.label || k.label;
}

/** The places of one kind on the account; a list belongs to the kind that sends. */
export function placesOf(a: Account, k: PlaceKind) {
  return (a.places ?? []).filter((p) => p.kind === k.kind || (k.lists && p.kind === "list"));
}

/** A kind that is not where the account publishes: its places say what they are ("Acme site (GA4 property)"). */
function ownKind(k: PlaceKind) {
  return !k.lists && k.verb !== "Publishes to";
}

/** Whether the account's name is one somebody gave it, not the service's or an ability's name the daemon fell back on. */
export function ownLabel(a: Account): boolean {
  const label = a.label.trim().toLowerCase();
  const given = [a.ability_name, a.connection_name, accountService(a).name, ...(a.used_by ?? []).map((u) => u.name)].filter(Boolean).map((s) => s.toLowerCase());
  return !!label && !given.includes(label);
}

/** Who the account is: the name the user gave it, else its places, else what the daemon says it is (a mail address, a host). Never an ability's name. */
export function accountIdentity(a: Account, lists: MailList[] = []): string {
  if (ownLabel(a)) return a.label.trim();
  const places = (a.places ?? []).map((p) => placeText(p, lists));
  if (places.length) return places.join(" · ");
  return a.identity || a.destination_text || "";
}

/** A place field read from a list through the account, or with a fixed set of values, or typed. */
export function PlaceFieldsForm({ slug, account, fields, values, onChange, onChoices }: { slug: string; account: string; fields: Record<string, PlaceField>; values: Record<string, string>; onChange: (k: string, v: string) => void; onChoices?: (k: string, c: { value: string; label: string }[]) => void }) {
  return (
    <>
      {Object.entries(fields).map(([k, f]) =>
        f.choices ? (
          <ChoiceField key={k} slug={slug} account={account} name={k} f={f} value={values[k] ?? ""} onChange={(v) => onChange(k, v)} onLoaded={(c) => onChoices?.(k, c)} />
        ) : (
          <Fields key={k} fields={placeFields({ [k]: f })} values={values} onChange={onChange} />
        ),
      )}
    </>
  );
}

/** One field picked from what the account has (its properties, its sites): read once, a search field when long. */
function ChoiceField({ slug, account, name, f, value, onChange, onLoaded }: { slug: string; account: string; name: string; f: PlaceField; value: string; onChange: (v: string) => void; onLoaded: (c: { value: string; label: string }[]) => void }) {
  const [list, setList] = useState<{ value: string; label: string }[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    setList(null);
    setError(null);
    api.accountChoices(slug, account, name).then(
      (r) => (setList(r.choices), onLoaded(r.choices), r.choices.length === 1 && !value && onChange(r.choices[0].value)),
      (e) => (setError((e as Error).message), setList([])),
    );
  }, [slug, account, name]);
  return (
    <label className="acc-field">
      <span className="nm">
        {f.label || name}
        {f.optional && <small className="dim"> optional</small>}
      </span>
      {list === null ? (
        <span className="muted sm">
          <Loader2 size={12} className="spin" /> Reading what the account has…
        </span>
      ) : list.length ? (
        <FieldInput options={list.map((c) => c.value)} labels={Object.fromEntries(list.map((c) => [c.value, c.label]))} search={list.length > 12} optional={f.optional} value={value} onChange={onChange} ariaLabel={f.label || name} />
      ) : (
        <>
          <FieldInput value={value} onChange={onChange} placeholder={f.placeholder ?? ""} ariaLabel={f.label || name} />
          <span className="muted sm">{error ? `The list could not be read (${error}); type it instead.` : "Nothing to choose from; type it instead."}</span>
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

/** The label a new place is saved with: the picked option's label when a field was picked from a list. */
export function chosenLabel(fields: Record<string, PlaceField>, values: Record<string, string>, read: Record<string, { value: string; label: string }[]>): string | undefined {
  for (const [k, f] of Object.entries(fields)) {
    if (!f.choices || !values[k]) continue;
    const hit = read[k]?.find((c) => c.value === values[k]);
    if (hit) return hit.label;
  }
  return undefined;
}

/** Whether every required field has a value, and at least one is given. */
export function placeReady(fields: Record<string, PlaceField>, values: Record<string, string>) {
  const e = Object.entries(fields);
  return e.every(([k, f]) => f.optional || (values[k] ?? "").trim()) && e.some(([k]) => (values[k] ?? "").trim());
}

/**
 * The places inside an account, one line per verb: "Publishes to: r/rust, r/selfhosted", "Sends to:
 * Newsletter (412)", "Measures: Acme site (GA4 property) · sc-domain:acme.test (Search Console property)",
 * each with add and remove; a kind that still needs one says "Pick a …".
 */
function Places({ slug, a, lists, onChange }: { slug: string; a: Account; lists: MailList[]; onChange: () => void }) {
  // Only a kind with somewhere inside it to choose, or places already, shows.
  const kinds = placeKinds(a).filter((k) => k.lists || Object.keys(k.fields ?? {}).length > 0 || placesOf(a, k).length > 0);
  const verbs = [...new Set(kinds.map((k) => k.verb))];
  if (!kinds.length) return null;
  return (
    <>
      {verbs.map((v) => (
        <PlaceRow key={v} slug={slug} a={a} verb={v} kinds={kinds.filter((k) => k.verb === v)} lists={lists} onChange={onChange} />
      ))}
    </>
  );
}

function PlaceRow({ slug, a, verb, kinds, lists, onChange }: { slug: string; a: Account; verb: string; kinds: PlaceKind[]; lists: MailList[]; onChange: () => void }) {
  const [adding, setAdding] = useState<PlaceKind | null>(null);
  const [values, setValues] = useState<Record<string, string>>({});
  const [read, setRead] = useState<Record<string, { value: string; label: string }[]>>({});
  const [list, setList] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const places = kinds.flatMap((k) => placesOf(a, k).map((p) => ({ p, k })));
  const run = async (what: string, f: () => Promise<unknown>) => {
    setBusy(what);
    setError(null);
    try {
      await f();
      setAdding(null);
      setValues({});
      setRead({});
      setList("");
      onChange();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(null);
    }
  };
  const open = (k: PlaceKind) => (setAdding(k), setValues({}), setRead({}), setList(""), setError(null));
  const free = lists.filter((l) => !(a.places ?? []).some((p) => p.fields?.list === l.id));
  const ok = adding ? (adding.lists ? !!list : placeReady(adding.fields, values)) : false;
  const add = (k: PlaceKind) =>
    run("add", () => api.addPlace(slug, a.id, k.lists ? { list } : { kind: k.kind, fields: values, label: chosenLabel(k.fields, values, read) }));
  const needs = kinds.filter((k) => k.needs && !placesOf(a, k).length);
  const addable = kinds.filter((k) => (k.lists ? free.length > 0 || lists.length === 0 : Object.keys(k.fields ?? {}).length > 0) && !needs.includes(k));
  const several = kinds.length > 1;
  return (
    <div className="acx-places">
      <span className="lb">{verb}:</span>
      {places.length === 0 && !needs.length && <span className="muted sm">{kinds.some((k) => k.lists) ? "the people named on each send" : verb === "Publishes to" ? "the account itself" : "nothing chosen yet"}</span>}
      {places.map(({ p, k }) => (
        <span key={p.id} className={`acx-place ${p.default ? "def" : ""}`} title={p.default && !ownKind(k) ? "Used when nothing names another" : undefined}>
          {placeText(p, lists)}
          {ownKind(k) && <span className="of">({placeNoun(k)})</span>}
          {!ownKind(k) && !p.default && placesOf(a, k).length > 1 && (
            <button className="x" title="Make it the default" aria-label="Make it the default" disabled={!!busy} onClick={() => void run(`def-${p.id}`, () => api.updatePlace(slug, a.id, p.id, { default: true }))}>
              <Star size={10} />
            </button>
          )}
          <button className="x" title="Remove" aria-label={`Remove ${p.label}`} disabled={!!busy} onClick={() => void run(`rm-${p.id}`, () => api.removePlace(slug, a.id, p.id))}>
            {busy === `rm-${p.id}` ? <Loader2 size={10} className="spin" /> : <X size={10} />}
          </button>
        </span>
      ))}
      {!adding &&
        needs.map((k) => (
          <button key={k.kind} className="acx-need" onClick={() => open(k)} title={`Nothing goes out${ownKind(k) ? ` and nothing is read for ${k.label}` : ""} until one is chosen`}>
            <AlertCircle size={12} /> Pick a {placeNoun(k)}
          </button>
        ))}
      {!adding &&
        addable.map((k) => (
          <button key={k.kind} className="lnk-btn" onClick={() => open(k)}>
            <Plus size={11} /> {k.lists ? "Add a list" : several || ownKind(k) ? `Add a ${placeNoun(k)}` : "Add a place"}
          </button>
        ))}
      <AnimatePresence>
        {adding && (
          <motion.div className="acx-add" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: "auto" }} exit={{ opacity: 0, height: 0 }}>
            {adding.lists ? (
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
              <PlaceFieldsForm slug={slug} account={a.id} fields={adding.fields} values={values} onChange={(k, v) => setValues((cur) => ({ ...cur, [k]: v }))} onChoices={(k, c) => setRead((cur) => ({ ...cur, [k]: c }))} />
            )}
            <div className="acts">
              <button className="btn p sm" disabled={!ok || !!busy} onClick={() => void add(adding)}>
                {busy === "add" ? <Loader2 size={13} className="spin" /> : <Check size={13} />} Add
              </button>
              <button className="btn t sm" onClick={() => setAdding(null)}>
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
export function UsedBy({ a, slug }: { a: Account; slug?: string }) {
  const by = a.used_by ?? [];
  if (by.length < 2) return null;
  return (
    <p className="acc-used" title="These abilities sign in once and share this account">
      <Users size={12} /> Used by{" "}
      {by.map((u, i) => (
        <span key={u.id}>
          {i > 0 && " · "}
          {slug ? <Link to={`/p/${slug}/abilities#ability=${encodeURIComponent(u.id)}`}>{u.name}</Link> : u.name}
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
            <SecretField key={k.env} name={k.env} label={k.label || k.env} help={k.help} placeholder={k.placeholder} set={k.set} optional={k.optional} options={k.options} choices={k.choices && signedIn} grant={a.connect?.grant} slug={slug} account={a.id} onSaved={onChange} />
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

// ---- Signing in on a hosted program

/** The service whose site is signed in to by hand only: it has no login steps, the browser keeps the sign-in. */
export function signsInByHand(a: Account): boolean {
  return accountService(a).id === "browser";
}

/** The chip for how the sign-in stands. */
export function sessionChip(st: SessionStatus | undefined): { text: string; cls: string; spin?: boolean } {
  switch (st?.state) {
    case "signed_in":
      return { text: "Signed in", cls: "acc" };
    case "signed_out":
      return { text: "Signed out", cls: "" };
    case "needs_hand":
      return { text: "Needs a hand", cls: "warn" };
    case "checking":
      return { text: "Checking…", cls: "", spin: true };
    default:
      return { text: "Not signed in yet", cls: "" };
  }
}

/** "The site asks for a person: captcha" from the daemon's "the site asks for a person: captcha". */
export function sentence(m: string): string {
  const t = m.trim().replace(/[.\s]+$/, "");
  return t ? `${t.charAt(0).toUpperCase()}${t.slice(1)}` : "";
}

/** How the sign-in stands as it changes: the account's, then each `session_changed` for it. */
export function useSession(slug: string, a: Pick<Account, "id" | "session">): [SessionStatus | undefined, (s: SessionStatus) => void] {
  const [live, setLive] = useState<SessionStatus | undefined>(undefined);
  useEffect(() => setLive(undefined), [a.session?.state, a.session?.checked_at]);
  useEvents(
    slug,
    (e) => {
      if (e.type !== "session_changed" || e.connection !== a.id) return;
      setLive((cur) => ({ ...(cur ?? a.session ?? { checked_at: null, signed_in_at: null }), state: e.state as SessionStatus["state"], message: String(e.message ?? "") }));
    },
    { demo: false },
  );
  return [live ?? a.session, setLive];
}

/**
 * Sign in, sign out, and when a person is needed the line that says why with
 * "Take the controls", which opens the live view.
 */
export function SessionBar({ slug, a, onChange }: { slug: string; a: Account; onChange: () => void }) {
  const [st, setSt] = useSession(slug, a);
  const [busy, setBusy] = useState<string | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [confirm, setConfirm] = useState(false);
  const [hands, setHands] = useState(false);
  const state = st?.state ?? "unknown";
  const signIn = async () => {
    setBusy("in");
    setError(null);
    setNote(null);
    try {
      const r = await api.accountSignIn(slug, a.id);
      if (r.started) setSt({ ...(st ?? { message: "", checked_at: null, signed_in_at: null }), state: "checking", message: "" });
      else if (r.note) setNote(sentence(r.note) + ".");
      onChange();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(null);
    }
  };
  const signOut = async () => {
    setBusy("out");
    setError(null);
    try {
      const r = await api.accountSignOut(slug, a.id);
      setSt(r.session);
      setConfirm(false);
      onChange();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(null);
    }
  };
  if (!a.has_session || !a.ready) return null;
  return (
    <>
      {state === "needs_hand" && (
        <div className="acx-hand">
          <p>
            {st?.message ? `${sentence(st.message)}. ` : ""}Finish the sign-in in the live view.
          </p>
          {a.live_view && (
            <div className="acts">
              <button className="btn p sm" onClick={() => setHands(true)}>
                <Hand size={13} /> Take the controls
              </button>
            </div>
          )}
        </div>
      )}
      <div className="acx-session">
        {state !== "signed_in" && state !== "checking" && (
          <button className={`btn ${state === "needs_hand" || signsInByHand(a) ? "t" : "g"} sm`} disabled={!!busy} onClick={() => void signIn()} title={signsInByHand(a) ? "See whether the browser is still signed in" : undefined}>
            {busy === "in" ? <Loader2 size={13} className="spin" /> : <LogIn size={13} />} {signsInByHand(a) ? "Check the sign-in" : state === "needs_hand" ? "Try again" : "Sign in"}
          </button>
        )}
        {state !== "signed_in" && state !== "needs_hand" && signsInByHand(a) && a.live_view && (
          <button className="btn g sm" onClick={() => setHands(true)}>
            <Hand size={13} /> Sign in in the live view
          </button>
        )}
        {state === "signed_in" && st?.signed_in_at && <span className="muted sm">Signed in {ago(st.signed_in_at)}{st.checked_at ? `, checked ${ago(st.checked_at)}` : ""}.</span>}
        {state === "checking" && (
          <span className="muted sm">
            <Loader2 size={12} className="spin" /> Checking the sign-in…
          </span>
        )}
        <span className="grow" />
        {state === "signed_in" && a.live_view && (
          <button className="btn t sm" onClick={() => setHands(true)} title="See what the browser shows and drive it">
            <Hand size={13} /> Live view
          </button>
        )}
        {state !== "unknown" && state !== "checking" && state !== "signed_out" &&
          (confirm ? (
            <span className="acc-confirm">
              <span className="muted sm">Sign out and forget what the browser kept?</span>
              <button className="btn t sm bad" disabled={!!busy} onClick={() => void signOut()}>
                {busy === "out" ? <Loader2 size={13} className="spin" /> : <Check size={13} />} Sign out
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
      {note && <p className="muted sm">{note}</p>}
      {error && <p className="err sm">{error}</p>}
      {hands && <Hands slug={slug} account={a.id} name={accountIdentity(a) || accountService(a).name} startUrl={startUrlOf(a.identity, a.destination_text)} onClose={(signedIn) => (setHands(false), signedIn && setSt({ ...(st ?? { message: "", checked_at: null, signed_in_at: null }), state: "checking", message: "" }), onChange())} />}
    </>
  );
}

const LOGIN_FIELDS: { key: (typeof LOGIN_KEYS)[number]; label: string; help?: string; secret: boolean; optional?: boolean }[] = [
  { key: "LOGIN_USERNAME", label: "User name", secret: false },
  { key: "LOGIN_PASSWORD", label: "Password", secret: true },
  { key: "LOGIN_TOTP_SECRET", label: "One-time code secret", help: "The secret your authenticator app was set up with. KIQR makes the six-digit codes from it.", secret: true, optional: true },
];

/**
 * Where the account's password is. Now only "Saved in KIQR": a user name, a
 * password and an optional one-time code secret, saved to the account's own
 * keys and never shown again (dots say which are there).
 */
export function LoginEditor({ slug, a, onSaved, onChange, ask = true }: { slug: string; a: Account; onSaved?: () => void; onChange: () => void; /** Ask "Where's your password?" (the wizard asks it in its title). */ ask?: boolean }) {
  const set = new Set(a.login_set ?? []);
  const saved = a.login?.store === "kiqr";
  const [editing, setEditing] = useState(!saved);
  const [values, setValues] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [confirm, setConfirm] = useState(false);
  useEffect(() => setEditing(a.login?.store !== "kiqr"), [a.login?.store]);
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
      await api.updateAccount(slug, a.id, { login: { store: "kiqr" } });
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
      await api.updateAccount(slug, a.id, { login: { store: "" } });
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
      {ask && (
        <>
          <p className="q">Where's your password?</p>
          <div className="seg" role="radiogroup" aria-label="Where the password is">
            <button role="radio" aria-checked className="on auto">
              <KeyRound size={12} /> Saved in KIQR
            </button>
          </div>
        </>
      )}
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
 * One account: the service and its identity, whether it works, one line for
 * what Seemo may do with one switch, its places, and Details (the full
 * permissions, how it connects, activity, rename, default, disconnect).
 */
export function AccountCard({ slug, a, lists, several, highlight, onChange }: { slug: string; a: Account; lists: MailList[]; several: boolean; highlight?: boolean; onChange: () => void }) {
  const svc = accountService(a);
  const Icon = serviceIcon(svc.icon, svc.group);
  const status = accountStatus(a);
  const [session] = useSession(slug, a);
  const chip = a.has_session && a.ready ? sessionChip(session) : null;
  const identity = accountIdentity(a, lists);
  const [details, setDetails] = useState(false);
  const [keys, setKeys] = useState(!a.ready);
  const [naming, setNaming] = useState(false);
  // The name field starts on the user's own name, empty when the daemon named it after the service.
  const own = ownLabel(a);
  const [name, setName] = useState(own ? a.label : "");
  const [confirm, setConfirm] = useState(false);
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
  return (
    <motion.div ref={ref} layout className={`pcard acx ${a.ready ? "" : "off"} ${highlight ? "hi" : ""}`} initial={{ opacity: 0, y: 6 }} animate={{ opacity: 1, y: 0 }}>
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
              <button className="btn t sm" onClick={() => (setNaming(false), setName(own ? a.label : ""))} aria-label="Keep the name">
                <X size={13} />
              </button>
            </span>
          ) : (
            identity && <p className="who" title={identity}>{identity}</p>
          )}
        </div>
        {chip ? (
          <span className={`chip sm ${chip.cls}`} title={session?.state === "needs_hand" && session.message ? sentence(session.message) : undefined}>
            {chip.spin ? <Loader2 size={11} className="spin" /> : <b />} {chip.text}
          </span>
        ) : (
          <span className={`chip sm ${status.tone}`}>
            <b /> {status.text}
          </span>
        )}
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
      <SessionBar slug={slug} a={a} onChange={onChange} />
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
            {a.has_session && (
              <section>
                <p className="cap">Login</p>
                <LoginEditor slug={slug} a={a} onChange={onChange} />
              </section>
            )}
            <section>
              <p className="cap">How it connects</p>
              <UsedBy a={a} slug={slug} />
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
