// What an ability may use: the credentials it asks for (by kind and host),
// the ones this project allowed it, the others in the Vault that would fit,
// and the one way to add another (the Vault's own Add panel, prefilled, or
// "Sign in with" for an ability whose provider signs in through kiqr.cloud).
// Also what a project picks once for an ability (a GA4 property, a site's
// address), read through the credential its calls go through. Used by the
// ability window, the Abilities list and Performance.
import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { AnimatePresence } from "motion/react";
import { Check, KeyRound, Loader2, Plus, Sparkles } from "lucide-react";
import { api, SERVICE_GROUPS, useAsync, useEvents, type Ability, type AccessCred, type AccessNeed, type CredStateWord, type ServiceCard, type SpecField } from "../api";
import { CredentialForm } from "./CredentialForm";
import { Favicon } from "./Favicon";
import { FieldInput, enumOf } from "./FieldInput";
import { Hint } from "./SecretField";
import { Switch } from "./Switch";
import { Drawer } from "./Vault";
import "../accounts.css";
import "../vault.css";

// ---- Small shared parts

/** The card an ability shows as (its name, icon, group), from what the daemon sends. */
export function serviceOf(a: Ability): ServiceCard & { group: string; name: string } {
  const c = a.service_card;
  const group = c?.group || guessGroup(a);
  return { id: c?.id || a.id, name: c?.name || a.name, icon: c?.icon || a.icon, group, profile_hosts: c?.profile_hosts };
}

function guessGroup(a: Ability): string {
  if (a.provides.some((p) => p.ability === "email") || a.categories.includes("email")) return "email";
  if (a.sources.length) return "numbers";
  if ((a.publishes_to ?? []).length || a.kinds?.length) return a.categories.includes("social") ? "social" : "website";
  return "tools";
}

/** Group things by group in the order SERVICE_GROUPS gives; unknown groups last, under "Other". */
export function byGroup<T>(items: T[], groupOf: (t: T) => string): { id: string; label: string; items: T[] }[] {
  const known = SERVICE_GROUPS.map((g) => ({ ...g, items: items.filter((t) => groupOf(t) === g.id) }));
  const rest = items.filter((t) => !SERVICE_GROUPS.some((g) => g.id === groupOf(t)));
  return [...known, { id: "other", label: "Other", items: rest }].filter((g) => g.items.length > 0);
}

/** "The site asks for a person: captcha" from the daemon's "the site asks for a person: captcha". */
export function sentence(m: string): string {
  const t = m.trim().replace(/[.\s]+$/, "");
  return t ? `${t.charAt(0).toUpperCase()}${t.slice(1)}` : "";
}

/** The chip for a credential's state, by the one readiness rule. */
export function credStateChip(s: CredStateWord | string | undefined): { text: string; cls: string; spin?: boolean } {
  switch (s) {
    case "ready":
      return { text: "Ready", cls: "ok" };
    case "signed_out":
      return { text: "Signed out", cls: "warn" };
    case "needs_hand":
      return { text: "Needs a hand", cls: "warn" };
    case "missing_fields":
      return { text: "Incomplete", cls: "warn" };
    case "needs_program":
      return { text: "Needs a program", cls: "warn" };
    case "expired":
      return { text: "Sign in again", cls: "warn" };
    case "checking":
      return { text: "Signing in", cls: "", spin: true };
    default:
      return { text: "Not checked yet", cls: "" };
  }
}

/** Where an ability is set up and given access: its window on the Abilities page. */
export function abilityHref(slug: string, id?: string) {
  return `/p/${slug}/abilities${id ? `#ability=${encodeURIComponent(id)}` : ""}`;
}

/** Where a credential is added: the Vault's Add panel. */
export function vaultAddHref(slug: string) {
  return `/p/${slug}/vault#add`;
}

// ---- Access

/**
 * An ability's Access: what it asks for, one block per need, each with the
 * credentials allowed here (a switch takes one back), others in the Vault
 * that fit (Allow), and Add one: the Vault's panel, prefilled for the need.
 */
export function AbilityAccess({ slug, a, onChange }: { slug: string; a: Ability; onChange: () => void }) {
  const needs = a.access ?? [];
  const hosts = (a.hosts ?? []).filter((h) => !h.includes("$"));
  return (
    <div className="ax">
      {hosts.length > 0 && (
        <section>
          <p className="cap">Where it works</p>
          <div className="ax-hosts">
            {hosts.map((h) => (
              <span key={h} className="ax-host">
                <Favicon domain={h} size={16} /> <span className="mono">{h}</span>
              </span>
            ))}
          </div>
          <p className="muted sm">It calls these sites and no others. A credential's values only ever go to that credential's own domains.</p>
        </section>
      )}
      {needs.length === 0 ? (
        <p className="muted">{a.name} asks for nothing from the Vault.</p>
      ) : (
        needs.map((n) => <NeedBlock key={n.id} slug={slug} a={a} n={n} onChange={onChange} />)
      )}
    </div>
  );
}

function NeedBlock({ slug, a, n, onChange }: { slug: string; a: Ability; n: AccessNeed; onChange: () => void }) {
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);
  const templates = useAsync(() => (adding ? api.vault().then((v) => v.templates) : Promise.resolve([])), [adding]);
  const toggle = async (c: AccessCred, on: boolean) => {
    setBusy(c.credential);
    setError(null);
    try {
      await (on ? api.grant(slug, a.id, c.credential) : api.revoke(slug, a.id, c.credential));
      onChange();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(null);
    }
  };
  const title = n.describe.charAt(0).toUpperCase() + n.describe.slice(1);
  return (
    <section className="ax-need">
      <div className="ax-need-hd">
        <span className="ic">
          <KeyRound size={15} />
        </span>
        <div className="min0">
          <b>
            {title}
            {n.optional && <small className="dim"> optional</small>}
          </b>
          {n.for && <span className="muted sm">{n.for}</span>}
        </div>
        {!n.granted.length && !n.optional && <span className="chip warn sm">Needed</span>}
      </div>
      {n.granted.length + n.fitting.length > 0 && (
        <div className="ax-creds">
          {[...n.granted.map((c) => ({ c, on: true })), ...n.fitting.map((c) => ({ c, on: false }))].map(({ c, on }) => (
            <div key={c.credential} className={`ax-cred ${on ? "on" : ""}`}>
              <Favicon domain={c.domain ?? undefined} size={26} />
              <span className="min0 nm">
                <b>{c.label}</b>
                <small>
                  {c.identity && c.identity !== c.label ? c.identity : c.domain}
                  {!c.complete && <span className="warn"> · incomplete</span>}
                </small>
              </span>
              {busy === c.credential ? <Loader2 size={14} className="spin" /> : null}
              <Switch on={on} disabled={busy === c.credential} onChange={(v) => void toggle(c, v)} label={on ? `Stop ${a.name} using ${c.label}` : `Let ${a.name} use ${c.label}`} />
            </div>
          ))}
        </div>
      )}
      {n.granted.length === 0 && n.fitting.length === 0 && <p className="muted sm">Nothing in the Vault fits yet.</p>}
      {n.fitting.length > 0 && n.granted.length === 0 && <p className="muted sm">Switch one on to let {a.name} use it in this project.</p>}
      {error && <p className="err sm">{error}</p>}
      {n.connect && !n.connect.from_channel && <NeedConnect slug={slug} a={a} n={n} onTypeIt={() => setAdding(true)} onDone={onChange} />}
      {n.connect?.from_channel && (
        <p className="muted sm">
          {n.connect.label} from your website's channel, under <Link to={`/p/${slug}/channels`}>Marketing → Channels</Link>: pick {a.name} as how it is published, and the site's address comes from there.
        </p>
      )}
      <button className={`btn ${n.connect ? "t" : "g"} sm ax-add`} onClick={() => setAdding(true)}>
        {n.provider ? <Sparkles size={13} /> : <Plus size={13} />} {n.provider ? `Sign in with ${n.provider_name || "kiqr.cloud"}` : `${n.connect ? "Or add" : "Add"} ${n.describe}`}
      </button>
      <AnimatePresence>
        {adding && (
          <Drawer key="add" title={n.provider ? `Sign in with ${n.provider_name || "kiqr.cloud"}` : `Add ${n.describe}`} iconNode={<KeyRound size={22} />} sub={`For ${a.name}; it may use it in this project once added`} onClose={() => setAdding(false)}>
            {templates.data?.length ? (
              <CredentialForm
                templates={templates.data}
                domains={n.hosts}
                template={n.template[0]}
                forDomains={n.hosts.length ? n.hosts : undefined}
                grant={[{ project: slug, ability: a.id }]}
                onSaved={(c) => {
                  // A credential typed here is allowed at once; a sign-in makes its grant when it comes back.
                  void api
                    .grant(slug, a.id, c.id)
                    .catch(() => {})
                    .finally(() => (setAdding(false), onChange()));
                }}
                onCancel={() => setAdding(false)}
              />
            ) : (
              <Loader2 size={16} className="spin" />
            )}
          </Drawer>
        )}
      </AnimatePresence>
    </section>
  );
}

/**
 * An ability's own one-click sign-in ("Connect with WordPress"): the settings
 * it asks first (the site's address), then the service's page in a new tab;
 * what it sends back becomes a credential, allowed to the ability here. When
 * the service can only call back over https and none is reachable, its page
 * shows the values and the user types them into the Vault's form instead.
 */
function NeedConnect({ slug, a, n, onTypeIt, onDone }: { slug: string; a: Ability; n: AccessNeed; onTypeIt: () => void; onDone: () => void }) {
  const c = n.connect!;
  const [values, setValues] = useState<Record<string, string>>(() => Object.fromEntries(c.needs.map((k) => [k, String(a.setting?.[k] ?? "")])));
  const [state, setState] = useState<"idle" | "starting" | "waiting" | "typing">("idle");
  const [error, setError] = useState<string | null>(null);
  useEvents(slug, (e) => {
    if (e.type !== "credential_connected" || e.provider !== a.id || state !== "waiting") return;
    if (e.ok) {
      setState("idle");
      onDone();
    } else {
      setState("idle");
      setError(String(e.error ?? "the sign-in did not finish"));
    }
  }, { demo: false });
  const start = async () => {
    setState("starting");
    setError(null);
    try {
      const r = await api.abilityConnect(slug, a.id, { origin: window.location.origin, need: n.id, values });
      window.open(r.url, "_blank", "noopener");
      setState(r.callback ? "waiting" : "typing");
    } catch (x) {
      setError((x as Error).message);
      setState("idle");
    }
  };
  const missing = c.needs.some((k) => !values[k]?.trim());
  return (
    <div className="ax-connect">
      {c.needs.map((k) => {
        const f = a.settings?.[k];
        return (
          <label key={k} className="acc-field">
            <span className="nm">{f?.label ?? k.replace(/_/g, " ")}</span>
            <FieldInput value={values[k] ?? ""} onChange={(v) => setValues({ ...values, [k]: v })} placeholder={f?.placeholder ?? ""} ariaLabel={f?.label ?? k} />
          </label>
        );
      })}
      <div className="vf-acts">
        <button className="btn p sm" disabled={missing || state === "starting"} onClick={() => void start()}>
          {state === "starting" ? <Loader2 size={13} className="spin" /> : <Sparkles size={13} />} {c.label}
        </button>
        {state === "waiting" && (
          <span className="muted sm">
            <Loader2 size={12} className="spin" /> Waiting for you to approve it in the new tab…
          </span>
        )}
      </div>
      {state === "typing" && (
        <p className="sm">
          The page shows the values once. <button className="lnk-btn" onClick={onTypeIt}>Type them into the Vault</button>; this KIQR is not reachable over https for it to send them back.
        </p>
      )}
      {error && <p className="err sm">{error}</p>}
    </div>
  );
}

/** What an ability still needs, in a line for its row: "Needs a login for x.com", or nothing. */
export function accessLine(a: Ability): { text: string; ok: boolean } | null {
  const needs = (a.access ?? []).filter((n) => !n.optional);
  if (!needs.length) return null;
  const missing = needs.filter((n) => !n.granted.some((c) => c.complete));
  if (missing.length) return { text: `Needs ${missing[0].describe}`, ok: false };
  const first = needs[0].granted[0];
  return { text: `Uses ${first.identity && first.identity !== first.label ? first.identity : first.label}`, ok: true };
}

// ---- Settings: what the project picks once for an ability

type Choice = { value: string; label: string };

/** The values a setting can take, read through the credential the ability's calls go through; null while reading. */
function useChoices(slug: string, ability: string, name: string, on: boolean): { list: Choice[] | null; error: string | null } {
  const [list, setList] = useState<Choice[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    if (!on) return;
    let gone = false;
    setList(null);
    setError(null);
    api.abilityChoices(slug, ability, name).then(
      (r) => !gone && setList(r.choices),
      (e) => !gone && (setError((e as Error).message), setList([])),
    );
    return () => {
      gone = true;
    };
  }, [slug, ability, name, on]);
  return { list, error };
}

/** Whether every required setting of an ability is picked. */
export function settingsPicked(a: Ability): boolean {
  return Object.entries(a.settings ?? {}).every(([k, f]) => f.optional || !!String(a.setting?.[k] ?? "").trim());
}

/** The values to save for an ability's settings: what it has, with the changes over them. */
export function settingValues(a: Ability, changes: Record<string, string> = {}): Record<string, string> {
  const out: Record<string, string> = {};
  for (const [k, v] of Object.entries(a.setting ?? {})) if (k !== "credential" && v != null && String(v).trim()) out[k] = String(v);
  for (const [k, v] of Object.entries(changes)) if (v.trim()) out[k] = v.trim();
  return out;
}

/**
 * An ability's settings, one line each: "GA4 property Acme site · Change",
 * or the field to pick it, a list read through its credential when it has one.
 */
export function AbilitySettings({ slug, a, lead, onChange }: { slug: string; a: Ability; /** The line's first words: "Measured by". */ lead?: string; onChange: () => void }) {
  const fields = Object.entries(a.settings ?? {});
  if (!fields.length) return null;
  return (
    <>
      {fields.map(([k, f]) => (
        <SettingLine key={k} slug={slug} a={a} name={k} f={f} lead={lead} onChange={onChange} />
      ))}
    </>
  );
}

function SettingLine({ slug, a, name, f, lead, onChange }: { slug: string; a: Ability; name: string; f: SpecField; lead?: string; onChange: () => void }) {
  const cur = String(a.setting?.[name] ?? "");
  const [editing, setEditing] = useState(false);
  const open = editing || !cur;
  const { list } = useChoices(slug, a.id, name, !!f.choices && !!cur && !editing);
  const shown = list?.find((c) => c.value === cur)?.label ?? cur;
  const label = f.label || name;
  return (
    <div className="ab-setting">
      {!open ? (
        <p className="sm">
          {lead && <span className="muted">{lead} · </span>}
          {label} <b className="mono">{shown}</b> <span className="muted">·</span>{" "}
          <button className="lnk-btn" onClick={() => setEditing(true)}>
            Change
          </button>
        </p>
      ) : (
        <SettingPick slug={slug} a={a} name={name} f={f} cur={cur} onCancel={cur ? () => setEditing(false) : undefined} onSaved={() => (setEditing(false), onChange())} />
      )}
    </div>
  );
}

/** One setting's input: a select of what the credential can see (a search field when long), a select of fixed values, or typed. */
export function SettingField({ slug, ability, name, f, value, onChange }: { slug: string; ability: string; name: string; f: SpecField; value: string; onChange: (v: string) => void }) {
  const { list, error } = useChoices(slug, ability, name, !!f.choices);
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
          <Loader2 size={12} className="spin" /> Reading what it can see…
        </span>
      ) : f.choices && list && list.length ? (
        <FieldInput options={list.map((c) => c.value)} labels={Object.fromEntries(list.map((c) => [c.value, c.label]))} search={list.length > 12} optional={f.optional} value={value} onChange={onChange} ariaLabel={label} />
      ) : (
        <>
          <FieldInput options={enumOf(f)} optional={f.optional} value={value} onChange={onChange} placeholder={f.placeholder ?? ""} ariaLabel={label} />
          {f.choices && <span className="muted sm">{error ? `The list could not be read (${sentence(error)}); type it instead.` : "Nothing to choose from; type it instead."}</span>}
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

function SettingPick({ slug, a, name, f, cur, onCancel, onSaved }: { slug: string; a: Ability; name: string; f: SpecField; cur: string; onCancel?: () => void; onSaved: () => void }) {
  const [value, setValue] = useState(cur);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      await api.setAbilitySettings(slug, a.id, String(a.setting?.credential ?? ""), settingValues(a, { [name]: value }));
      onSaved();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="ab-pick">
      {!cur && <p className="sm">Pick the {(f.label || name).toLowerCase()}</p>}
      <SettingField slug={slug} ability={a.id} name={name} f={f} value={value} onChange={setValue} />
      {error && <p className="err sm">{error}</p>}
      <div className="acts">
        <button className="btn p sm" disabled={busy || !value.trim() || value === cur} onClick={() => void save()}>
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
