// Marketing → Audience: everywhere the business's people are, in three
// sections (your pages, communities, lists), each reached through one of the
// project's accounts. A row says which account and whether it works; one with
// no account says so and lets you pick one. Seemo's suggestions wait in a
// banner for Review. A list opens its people (Lists.tsx). The asset actions and
// the add form share the helpers here: the Audience loaded once per project,
// an entry's icon, and the fields that name a spot.
import { useEffect, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { Check, ChevronDown, ChevronUp, ExternalLink, Globe, Loader2, Mail, Plus, Sparkles, Trash2, Users, X, type LucideIcon } from "lucide-react";
import { api, AUDIENCE_EVENTS, SECTIONS, useEvents, type AudienceEntry, type AudienceKind, type AudienceSection, type AudienceView, type SpecField } from "../api";
import { assetIcon } from "../assetIcons";
import { FieldInput, enumOf } from "./FieldInput";
import { ListPage } from "./Lists";
import { ago } from "./Project";
import "../profile.css";
import "../audience.css";

// ------------------------------------------------------------------ shared

const cache = new Map<string, Promise<AudienceView>>();

function load(slug: string): Promise<AudienceView> {
  let p = cache.get(slug);
  if (!p) {
    p = api.audience(slug);
    p.catch(() => cache.delete(slug));
    cache.set(slug, p);
  }
  return p;
}

/** The project's Audience, loaded once and shared by every block that needs it; reloaded when it or an account changes. */
export function useAudience(slug: string): { data: AudienceView | null; error: string | null; reload: () => void } {
  const [data, setData] = useState<AudienceView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [key, setKey] = useState(0);
  useEffect(() => {
    let live = true;
    load(slug).then(
      (v) => live && (setData(v), setError(null)),
      (e: Error) => live && setError(e.message),
    );
    return () => {
      live = false;
    };
  }, [slug, key]);
  const reload = () => {
    cache.delete(slug);
    setKey((k) => k + 1);
  };
  useEvents(slug, (e) => AUDIENCE_EVENTS.includes(e.type) && reload(), { demo: false });
  return { data, error, reload };
}

/** The connect wizard, at a service when one is named. */
export function connectHref(slug: string, service?: string) {
  return `/p/${slug}/accounts/connect${service ? `?service=${encodeURIComponent(service)}` : ""}`;
}

/** "r/selfhosted · via u/acme_dev"; the account once when it is the spot itself (an X profile). */
export function entryText(e: AudienceEntry) {
  return e.via && e.via.label !== e.label ? `${e.label} · via ${e.via.label}` : e.label;
}

const SECTION_ICON: Record<AudienceSection, LucideIcon> = { page: Globe, community: Users, list: Mail };

/** An entry's or kind's icon: a picture when it names one, a lucide icon by name, else its section's. */
export function EntryIcon({ icon, section, size = 15 }: { icon?: string; section: AudienceSection; size?: number }) {
  if (icon && /^(https?:|\/|data:)/.test(icon)) return <img src={icon} alt="" width={size} height={size} />;
  const named = assetIcon(icon);
  const Icon = icon && named !== assetIcon(undefined) ? named : SECTION_ICON[section];
  return <Icon size={size} />;
}

/** What an account that does not work now needs, in a few words. */
const STATE_WORDS: Record<string, string> = { signed_out: "signed out", needs_hand: "needs a hand", missing_keys: "needs its keys", needs_program: "needs a program", checking: "checking" };

/** The kind's fields that name a spot (a subreddit): a select for a fixed set or a list read through the account, text otherwise. */
export function SpotFields({ slug, kind, account, values, onChange }: { slug: string; kind: AudienceKind; account: string; values: Record<string, string>; onChange: (v: Record<string, string>) => void }) {
  if (kind.account_is_target) return null;
  return (
    <>
      {Object.entries(kind.target ?? {}).map(([name, spec]) => (
        <SpotField key={name} slug={slug} name={name} spec={spec} account={account} value={values[name] ?? ""} onChange={(v) => onChange({ ...values, [name]: v })} />
      ))}
    </>
  );
}

function SpotField({ slug, name, spec, account, value, onChange }: { slug: string; name: string; spec: SpecField; account: string; value: string; onChange: (v: string) => void }) {
  const [choices, setChoices] = useState<{ value: string; label: string }[] | null>(null);
  useEffect(() => {
    setChoices(null);
    if (!spec.choices || !account) return;
    let live = true;
    api.accountChoices(slug, account, name).then((r) => live && setChoices(r.choices), () => {});
    return () => {
      live = false;
    };
  }, [slug, account, name, spec.choices]);
  const options = choices?.map((c) => c.value) ?? enumOf(spec);
  const labels = choices ? Object.fromEntries(choices.map((c) => [c.value, c.label])) : undefined;
  return (
    <label className="acc-field">
      <span className="nm">
        {spec.label ?? name.replace(/_/g, " ")}
        {spec.optional && <small className="dim"> optional</small>}
      </span>
      <FieldInput value={value} onChange={onChange} options={options} labels={labels} search={!!choices && choices.length > 12} optional={spec.optional} placeholder={spec.placeholder} mono ariaLabel={spec.label ?? name} />
      {spec.help && <span className="ds muted">{spec.help}</span>}
    </label>
  );
}

/** Only the fields with a value, as the daemon wants them. */
export const filled = (v: Record<string, string>) => Object.fromEntries(Object.entries(v).filter(([, x]) => x.trim()).map(([k, x]) => [k, x.trim()]));

// ------------------------------------------------------------------ page

export function AudiencePage() {
  const { slug = "", id } = useParams();
  return id ? <ListPage slug={slug} id={decodeURIComponent(id)} /> : <AudienceIndex slug={slug} />;
}

/** One line per section when it is empty: what belongs there. */
const EMPTY: Record<AudienceSection, string> = {
  page: "Nothing yet. Your own profiles, company pages and blog go here.",
  community: "Nothing yet. The subreddits and groups where you take part go here.",
  list: "No lists yet. A list holds the people who asked to hear from you.",
};

function AudienceIndex({ slug }: { slug: string }) {
  const view = useAudience(slug);
  const [adding, setAdding] = useState(false);
  const [review, setReview] = useState(false);
  const entries = view.data?.entries ?? [];
  const kinds = view.data?.kinds ?? [];
  const suggestions = entries.filter((e) => e.suggested);
  const kept = entries.filter((e) => !e.suggested);
  return (
    <div className="prof au">
      <div className="cm-head">
        <div>
          <p className="cap">Marketing</p>
          <h1 className="d2">Audience</h1>
          <p className="muted">Everywhere your people are, and the account Seemo reaches each one with.</p>
        </div>
        {!adding && (
          <button className="btn p" onClick={() => setAdding(true)}>
            <Plus size={15} /> Add
          </button>
        )}
      </div>
      {view.error && <p className="err">{view.error}</p>}
      {suggestions.length > 0 && (
        <div className="pcard au-sugg">
          <button className="au-sugg-head" onClick={() => setReview((r) => !r)} aria-expanded={review}>
            <Sparkles size={15} />
            <span className="grow">
              Seemo found {suggestions.length} {suggestions.length === 1 ? "place" : "places"} worth your time
            </span>
            <span className="lnk">Review {review ? <ChevronUp size={13} /> : <ChevronDown size={13} />}</span>
          </button>
          {review && (
            <ul className="au-rows">
              {suggestions.map((e) => (
                <SuggestionRow key={e.id} slug={slug} e={e} onChange={view.reload} />
              ))}
            </ul>
          )}
        </div>
      )}
      {adding && <AddForm slug={slug} kinds={kinds} onDone={() => (setAdding(false), view.reload())} onCancel={() => setAdding(false)} />}
      {view.data &&
        SECTIONS.map((s) => {
          const rows = kept.filter((e) => e.section === s.id);
          return (
            <section key={s.id} className="au-sec">
              <h2>
                {s.label} {rows.length > 0 && <span className="n">{rows.length}</span>}
              </h2>
              <p className="muted sm">{s.hint}</p>
              {rows.length === 0 ? (
                <p className="au-empty muted sm">{EMPTY[s.id]}</p>
              ) : (
                <ul className="au-rows pcard">
                  {rows.map((e) => (
                    <EntryRow key={e.id} slug={slug} e={e} kind={kinds.find((k) => k.kind === e.kind)} onChange={view.reload} />
                  ))}
                </ul>
              )}
            </section>
          );
        })}
      {!view.data && !view.error && <Loader2 size={16} className="spin" />}
    </div>
  );
}

/** Which account reaches it and whether that works; none, with a pick of another account of the service. */
function Via({ slug, e, kind, onChange }: { slug: string; e: AudienceEntry; kind?: AudienceKind; onChange: () => void }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  if (e.via) {
    const word = e.via.ready ? "" : STATE_WORDS[e.via.state] ?? "needs a look";
    return (
      <Link className="au-via" to={`/p/${slug}/accounts#account=${encodeURIComponent(e.via.account)}`} title={e.via.service_name}>
        <span className={`dot ${e.via.ready ? "ok" : "warn"}`} />
        via {e.via.label}
        {word && <span className="warn"> · {word}</span>}
      </Link>
    );
  }
  const others = kind?.accounts ?? [];
  const pick = async (account: string) => {
    if (!account) return;
    setBusy(true);
    setError(null);
    try {
      await api.updateAudience(slug, e.id, { account });
      onChange();
    } catch (x) {
      setError((x as Error).message);
    } finally {
      setBusy(false);
    }
  };
  return (
    <span className="au-via none">
      <span className="dot bad" />
      <span className="bad">needs an account</span>
      {others.length > 0 ? (
        <select className="input sm" value="" disabled={busy} onChange={(x) => void pick(x.target.value)} aria-label="Reach it through">
          <option value="" disabled>
            Pick one…
          </option>
          {others.map((a) => (
            <option key={a.id} value={a.id}>
              {a.label}
            </option>
          ))}
        </select>
      ) : (
        <Link to={connectHref(slug, kind?.service)}>Connect one</Link>
      )}
      {busy && <Loader2 size={12} className="spin" />}
      {error && <span className="err sm">{error}</span>}
    </span>
  );
}

/** People on a list who still hear from the business. */
const reachable = (e: AudienceEntry) => (e.contacts ?? 0) - (e.unsubscribed ?? 0);

function EntryRow({ slug, e, kind, onChange }: { slug: string; e: AudienceEntry; kind?: AudienceKind; onChange: () => void }) {
  const [confirm, setConfirm] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const isList = e.section === "list";
  const remove = async () => {
    setBusy(true);
    setError(null);
    try {
      await api.removeAudience(slug, e.id);
      onChange();
    } catch (x) {
      setError((x as Error).message);
      setBusy(false);
    }
  };
  const name = (
    <>
      <span className="ic">
        <EntryIcon icon={e.icon} section={e.section} />
      </span>
      <span className="min0">
        <b>{e.label}</b>
        <small>{e.kind_name ?? kind?.name ?? e.kind}</small>
      </span>
    </>
  );
  const n = reachable(e);
  return (
    <li className="au-row">
      {isList ? (
        <Link className="what" to={`/p/${slug}/audience/${encodeURIComponent(e.id)}`}>
          {name}
        </Link>
      ) : (
        <span className="what">
          {name}
          {e.url && (
            <a className="dim" href={e.url} target="_blank" rel="noreferrer" aria-label="Open it">
              <ExternalLink size={12} />
            </a>
          )}
        </span>
      )}
      <Via slug={slug} e={e} kind={kind} onChange={onChange} />
      <span className="mono au-count">
        {isList ? `${n} ${n === 1 ? "person" : "people"}` : `${e.publications ?? 0} ${e.publications === 1 ? "post" : "posts"}${e.last_published ? ` · last ${ago(e.last_published)}` : ""}`}
      </span>
      {confirm ? (
        <span className="acc-confirm">
          <span className="muted sm">{isList ? "Delete the list and everyone on it?" : "Remove it from your Audience?"}</span>
          <button className="btn t sm bad" disabled={busy} onClick={() => void remove()}>
            {busy ? <Loader2 size={12} className="spin" /> : <Check size={12} />} {isList ? "Delete" : "Remove"}
          </button>
          <button className="btn t sm" onClick={() => setConfirm(false)}>
            Keep
          </button>
        </span>
      ) : (
        <button className="btn t sm" title={isList ? "Delete the list" : "Remove from your Audience"} aria-label="Remove" onClick={() => setConfirm(true)}>
          <Trash2 size={13} />
        </button>
      )}
      {error && <p className="err sm au-err">{error}</p>}
    </li>
  );
}

/** Something Seemo found: why, and Add or Dismiss. */
function SuggestionRow({ slug, e, onChange }: { slug: string; e: AudienceEntry; onChange: () => void }) {
  const [busy, setBusy] = useState<"add" | "dismiss" | null>(null);
  const [error, setError] = useState<string | null>(null);
  const run = async (what: "add" | "dismiss") => {
    setBusy(what);
    setError(null);
    try {
      await (what === "add" ? api.updateAudience(slug, e.id, { add: true }) : api.removeAudience(slug, e.id));
      onChange();
    } catch (x) {
      setError((x as Error).message);
      setBusy(null);
    }
  };
  return (
    <li className="au-row">
      <span className="what">
        <span className="ic">
          <EntryIcon icon={e.icon} section={e.section} />
        </span>
        <span className="min0">
          <b>
            {e.label}
            {e.url && (
              <a className="dim" href={e.url} target="_blank" rel="noreferrer" aria-label="Open it">
                {" "}
                <ExternalLink size={11} />
              </a>
            )}
          </b>
          <small>
            {e.kind_name ?? e.kind}
            {e.via ? ` · via ${e.via.label}` : ""}
          </small>
          {e.reason && <small className="ai">{e.reason}</small>}
        </span>
      </span>
      <button className="btn p sm" disabled={!!busy} onClick={() => void run("add")}>
        {busy === "add" ? <Loader2 size={13} className="spin" /> : <Plus size={13} />} Add
      </button>
      <button className="btn t sm" disabled={!!busy} onClick={() => void run("dismiss")}>
        {busy === "dismiss" ? <Loader2 size={13} className="spin" /> : <X size={13} />} Dismiss
      </button>
      {error && <p className="err sm au-err">{error}</p>}
    </li>
  );
}

/** Add to the Audience: a kind, the account that reaches it, the fields that name the spot; a list is a name and the mail account it is sent from. */
function AddForm({ slug, kinds, onDone, onCancel }: { slug: string; kinds: AudienceKind[]; onDone: () => void; onCancel: () => void }) {
  const navigate = useNavigate();
  const [kindId, setKindId] = useState(kinds.find((k) => k.section !== "list")?.kind ?? kinds[0]?.kind ?? "");
  const kind = kinds.find((k) => k.kind === kindId);
  const [account, setAccount] = useState("");
  const [fields, setFields] = useState<Record<string, string>>({});
  const [label, setLabel] = useState("");
  const [description, setDescription] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const isList = kind?.kind === "list";
  const accountId = account || kind?.accounts[0]?.id || "";
  const choose = (k: string) => {
    setKindId(k);
    setAccount("");
    setFields({});
    setError(null);
  };
  const missing = !kind || (!isList && kind.accounts.length === 0) || (isList ? !label.trim() : Object.entries(kind.target ?? {}).some(([n, s]) => !s.optional && !kind.account_is_target && !(fields[n] ?? "").trim()));
  const add = async () => {
    if (!kind) return;
    setBusy(true);
    setError(null);
    try {
      const e = await api.addAudience(slug, { kind: kind.kind, account: accountId || undefined, fields: filled(fields), label: label.trim() || undefined, description: description.trim() || undefined });
      onDone();
      if (isList) navigate(`/p/${slug}/audience/${encodeURIComponent(e.id)}`);
    } catch (x) {
      setError((x as Error).message);
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="pcard au-add">
      <p className="cap">Add to your Audience</p>
      <div className="au-form">
        <label className="acc-field">
          <span className="nm">What it is</span>
          <select className="input" value={kindId} onChange={(e) => choose(e.target.value)}>
            {SECTIONS.map((s) => {
              const of = kinds.filter((k) => k.section === s.id);
              return of.length ? (
                <optgroup key={s.id} label={s.label}>
                  {of.map((k) => (
                    <option key={k.kind} value={k.kind}>
                      {k.name || k.label}
                    </option>
                  ))}
                </optgroup>
              ) : null;
            })}
          </select>
        </label>
        {kind && isList && (
          <>
            <label className="acc-field">
              <span className="nm">Name</span>
              <input className="input" autoFocus value={label} placeholder="Newsletter" onChange={(e) => setLabel(e.target.value)} />
            </label>
            <label className="acc-field">
              <span className="nm">
                What it is for<small className="dim"> optional</small>
              </span>
              <input className="input" value={description} placeholder="Product news, once a month" onChange={(e) => setDescription(e.target.value)} />
            </label>
          </>
        )}
        {kind && kind.accounts.length > 0 && (
          <label className="acc-field">
            <span className="nm">{isList ? "Sent from" : "Through"}</span>
            <FieldInput value={accountId} onChange={setAccount} options={kind.accounts.map((a) => a.id)} labels={Object.fromEntries(kind.accounts.map((a) => [a.id, a.label]))} ariaLabel="Account" plain />
          </label>
        )}
        {kind && !isList && kind.accounts.length > 0 && <SpotFields slug={slug} kind={kind} account={accountId} values={fields} onChange={setFields} />}
        {kind && !isList && kind.accounts.length > 0 && (
          <label className="acc-field">
            <span className="nm">
              Name<small className="dim"> optional</small>
            </span>
            <input className="input" value={label} placeholder="As it shows here" onChange={(e) => setLabel(e.target.value)} />
          </label>
        )}
      </div>
      {kind && kind.accounts.length === 0 && (
        <p className="muted sm">
          {isList ? (
            <>
              A list can wait for its mail account; to send to it, <Link to={connectHref(slug)}>connect a mail account</Link>.
            </>
          ) : (
            <Link to={connectHref(slug, kind.service)}>Connect a {kind.service_name ?? kind.name} account first</Link>
          )}
        </p>
      )}
      {!kinds.length && <p className="muted sm">Nothing installed here can reach anywhere yet. <Link to={connectHref(slug)}>Connect an account</Link> first.</p>}
      <div className="acts">
        <button className="btn p sm" disabled={busy || missing} onClick={() => void add()}>
          {busy ? <Loader2 size={13} className="spin" /> : <Check size={13} />} {isList ? "Make the list" : "Add it"}
        </button>
        <button className="btn t sm" onClick={onCancel}>
          Cancel
        </button>
      </div>
      {error && <p className="err sm">{error}</p>}
    </div>
  );
}
