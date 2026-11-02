// Adding a credential to the Vault: the one way in, wherever it is opened
// from (the Vault, a channel's publisher, an ability's access). "Sign in with"
// comes first: the providers kiqr.cloud signs in to, one click each. Below,
// "Add your own": a login, an API key, a mail server or anything with fields,
// with the domains it is for. Values are typed here and never shown again.
import { useEffect, useMemo, useState } from "react";
import { Check, KeyRound, Loader2, LogIn, Mail, Plus, Shapes, User, X } from "lucide-react";
import { api, useAsync, useEvents, type Credential, type CredentialDraft, type CredentialTemplate, type SignInProvider } from "../api";
import { Favicon } from "./Favicon";
import { SignInById } from "./SignIns";

const TEMPLATE_ICON: Record<string, typeof User> = { login: User, api_key: KeyRound, smtp: Mail, custom: Shapes };

export type CredentialFormProps = {
  templates: CredentialTemplate[];
  /** Start with these domains (the ones a channel or an ability needs). */
  domains?: string[];
  /** Start with this kind (a login for a site, a key for an API). */
  template?: string;
  /** Show only the providers that cover these domains, and say so. */
  forDomains?: string[];
  /** Grants to make when a "Sign in with" comes back (the ability that asked). */
  grant?: { project: string; ability: string }[];
  onSaved: (c: Credential | { id: string }) => void;
  onCancel?: () => void;
};

export function CredentialForm({ templates, domains: startDomains, template: startTemplate, forDomains, grant, onSaved, onCancel }: CredentialFormProps) {
  const providers = useAsync(() => api.signInProviders(), []);
  useEvents(null, (e) => e.type === "sign_in_changed" && providers.reload(), { demo: false });
  const shown = useMemo(() => {
    const all = providers.data?.providers ?? [];
    if (!forDomains?.length) return all;
    return all.filter((p) => p.domains.some((d) => forDomains.some((f) => sameSite(d, f))));
  }, [providers.data, forDomains]);
  const [picked, setPicked] = useState<SignInProvider | null>(null);
  const own = templates.filter((t) => t.id !== "oauth");
  const [template, setTemplate] = useState<string | null>(startTemplate && startTemplate !== "oauth" ? startTemplate : null);

  return (
    <div className="vf">
      {(shown.length > 0 || providers.loading) && !template && (
        <section className="vf-sec">
          <p className="cap">Sign in with</p>
          {providers.loading && !providers.data && (
            <p className="muted sm">
              <Loader2 size={13} className="spin" /> Asking kiqr.cloud what it signs in to…
            </p>
          )}
          {!picked && (
            <div className="vf-tiles">
              {shown.map((p) => (
                <button key={p.id} className="vf-tile prov" onClick={() => setPicked(p)}>
                  <Favicon domain={p.domains[0]} size={30} />
                  <span className="min0">
                    <b>{p.name}</b>
                    <small>One click, through kiqr.cloud</small>
                  </span>
                </button>
              ))}
            </div>
          )}
          {picked && <SignInWith p={picked} cloudSignedIn={!!providers.data?.cloud_signed_in} grant={grant} onBack={() => setPicked(null)} onDone={(id) => onSaved({ id })} />}
        </section>
      )}

      {!picked && (
        <section className="vf-sec">
          <p className="cap">{shown.length > 0 ? "Or add your own" : "Add your own"}</p>
          {!template && (
            <div className="vf-tiles">
              {own.map((t) => {
                const Icon = TEMPLATE_ICON[t.id] ?? Shapes;
                return (
                  <button key={t.id} className="vf-tile" onClick={() => setTemplate(t.id)}>
                    <span className="vf-ic">
                      <Icon size={16} />
                    </span>
                    <span className="min0">
                      <b>{t.label}</b>
                      <small>{templateHint(t.id)}</small>
                    </span>
                  </button>
                );
              })}
            </div>
          )}
          {template && <OwnForm t={templates.find((x) => x.id === template)!} startDomains={startDomains ?? []} onBack={() => setTemplate(null)} onSaved={onSaved} />}
        </section>
      )}

      {onCancel && !template && !picked && (
        <div className="vf-acts">
          <button className="btn t" onClick={onCancel}>
            Cancel
          </button>
        </div>
      )}
    </div>
  );
}

function templateHint(id: string): string {
  switch (id) {
    case "login":
      return "A username and password, and a two-factor key if it has one";
    case "api_key":
      return "A key a service gave you";
    case "smtp":
      return "The server your email goes out through";
    default:
      return "Any fields you name";
  }
}

/** A provider picked: what it may read (all ticked), then the sign-in in a new tab. */
function SignInWith({ p, cloudSignedIn, grant, onBack, onDone }: { p: SignInProvider; cloudSignedIn: boolean; grant?: { project: string; ability: string }[]; onBack: () => void; onDone: (id: string) => void }) {
  const [scopes, setScopes] = useState<string[]>(p.scopes.map((s) => s.scope));
  const [busy, setBusy] = useState(false);
  const [waiting, setWaiting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEvents(null, (e) => {
    if (e.type !== "credential_connected" || e.provider !== p.id || !waiting) return;
    setWaiting(false);
    if (e.ok && e.credential) onDone(String(e.credential));
    else setError(String(e.error ?? "the sign-in did not finish"));
  }, { demo: false });
  const start = async () => {
    setBusy(true);
    setError(null);
    try {
      const r = await api.vaultConnect(p.id, { origin: window.location.origin, scopes, grant });
      window.open(r.url, "_blank", "noopener");
      setWaiting(true);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="vf-sign">
      <div className="vf-sign-hd">
        <Favicon domain={p.domains[0]} size={30} />
        <div className="min0">
          <b>{p.name}</b>
          <small className="mono">{p.domains.join(", ")}</small>
        </div>
        <button className="x" onClick={onBack} aria-label="Back">
          <X size={15} />
        </button>
      </div>
      {!cloudSignedIn && (
        <div className="vf-note">
          <p>kiqr.cloud finishes this sign-in for KIQR. Sign in to it first; it keeps no token.</p>
          <SignInById id="kiqr-cloud" label="Sign in to kiqr.cloud" />
        </div>
      )}
      {p.scopes.length > 1 && (
        <div className="vf-scopes">
          <p className="cap">What it's for</p>
          {p.scopes.map((s) => (
            <label key={s.scope} className="vf-check">
              <input type="checkbox" checked={scopes.includes(s.scope)} onChange={(e) => setScopes(e.target.checked ? [...scopes, s.scope] : scopes.filter((x) => x !== s.scope))} />
              <span>{s.label}</span>
            </label>
          ))}
        </div>
      )}
      {error && <p className="err">{error}</p>}
      <div className="vf-acts">
        <button className="btn p" disabled={busy || !cloudSignedIn || scopes.length === 0} onClick={() => void start()}>
          {busy ? <Loader2 size={14} className="spin" /> : <LogIn size={14} />} Sign in with {p.name}
        </button>
        {waiting && (
          <span className="muted sm">
            <Loader2 size={13} className="spin" /> Finish in the tab that opened…
          </span>
        )}
      </div>
    </div>
  );
}

/** A credential of your own: its domains, a name, the template's fields. */
function OwnForm({ t, startDomains, onBack, onSaved }: { t: CredentialTemplate; startDomains: string[]; onBack: () => void; onSaved: (c: Credential) => void }) {
  const [domains, setDomains] = useState<string[]>(startDomains);
  const [domainText, setDomainText] = useState("");
  const [label, setLabel] = useState("");
  const [values, setValues] = useState<Record<string, string>>({});
  const [extra, setExtra] = useState<{ name: string; label: string; secret: boolean }[]>(t.id === "custom" ? [{ name: "", label: "", secret: true }] : []);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => setDomains(startDomains), [startDomains.join(",")]); // eslint-disable-line react-hooks/exhaustive-deps

  const addDomain = (raw: string) => {
    const parts = raw.split(/[\s,]+/).map(host).filter((d): d is string => !!d && !domains.includes(d));
    if (parts.length) setDomains([...domains, ...parts]);
    setDomainText("");
  };
  const fields = [...t.fields, ...extra.filter((f) => f.name).map((f) => ({ name: slugField(f.name), label: f.label || f.name, kind: f.secret ? ("secret" as const) : ("text" as const), optional: false }))];
  const missing = fields.filter((f) => !f.optional && !values[f.name]?.trim() && !(t.id === "login" && f.name === "password"));
  const allDomains = domainText.trim() ? [...domains, ...domainText.split(/[\s,]+/).map(host).filter((d): d is string => !!d)] : domains;
  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      const d: CredentialDraft = {
        template: t.id,
        label: label.trim() || undefined,
        domains: allDomains,
        values: Object.fromEntries(Object.entries(values).filter(([, v]) => v.trim())),
        fields: extra.filter((f) => f.name).map((f) => ({ name: slugField(f.name), label: f.label || f.name, kind: f.secret ? "secret" : "text", optional: false })),
      };
      onSaved(await api.createCredential(d));
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="vf-own">
      <div className="vf-sign-hd">
        <span className="vf-ic">{(() => {
          const Icon = TEMPLATE_ICON[t.id] ?? Shapes;
          return <Icon size={16} />;
        })()}</span>
        <div className="min0">
          <b>{t.label}</b>
          <small>{templateHint(t.id)}</small>
        </div>
        <button className="x" onClick={onBack} aria-label="Back">
          <X size={15} />
        </button>
      </div>

      <label className="set-key">
        <span className="lb">Where it is used</span>
        <span className="vf-domains">
          {domains.map((d, i) => (
            <span key={d} className={`vf-dom ${i === 0 ? "first" : ""}`}>
              <Favicon domain={d} size={16} /> <span className="mono">{d}</span>
              <button aria-label={`Remove ${d}`} onClick={() => setDomains(domains.filter((x) => x !== d))}>
                <X size={11} />
              </button>
            </span>
          ))}
          <input
            className="vf-dom-in mono"
            placeholder={domains.length ? "Another domain" : "x.com"}
            value={domainText}
            onChange={(e) => setDomainText(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" || e.key === "," || e.key === " ") {
                e.preventDefault();
                addDomain(domainText);
              } else if (e.key === "Backspace" && !domainText && domains.length) setDomains(domains.slice(0, -1));
            }}
            onBlur={() => domainText.trim() && addDomain(domainText)}
          />
        </span>
        <small className="muted">The first is its home: its icon, and where Seemo looks for it first. Its values are only ever sent to these domains.</small>
      </label>

      <label className="set-key">
        <span className="lb">Name it</span>
        <input className="input" placeholder={t.id === "login" ? "Your username, if left empty" : allDomains[0] ?? ""} value={label} onChange={(e) => setLabel(e.target.value)} />
      </label>

      {t.fields.map((f) => (
        <label key={f.name} className="set-key">
          <span className="lb">
            {f.label} {f.optional && <small>optional</small>}
          </span>
          <input
            className={`input ${f.kind === "text" ? "" : "mono"}`}
            type={f.kind === "secret" || f.kind === "totp" ? "password" : f.kind === "number" ? "number" : "text"}
            autoComplete="off"
            placeholder={f.kind === "totp" ? "The setup key (base32), not a six-digit code" : ""}
            value={values[f.name] ?? ""}
            onChange={(e) => setValues({ ...values, [f.name]: e.target.value })}
          />
          {t.id === "login" && f.name === "password" && <small className="muted">Leave it empty to sign in by hand once, in a live view; Seemo then keeps the session.</small>}
        </label>
      ))}

      {extra.map((f, i) => (
        <div key={i} className="vf-extra">
          <input className="input" placeholder="Field name, e.g. Site ID" value={f.label} onChange={(e) => setExtra(extra.map((x, j) => (j === i ? { ...x, label: e.target.value, name: e.target.value } : x)))} />
          <input
            className="input mono"
            type={f.secret ? "password" : "text"}
            autoComplete="off"
            placeholder="Value"
            disabled={!f.name}
            value={values[slugField(f.name)] ?? ""}
            onChange={(e) => setValues({ ...values, [slugField(f.name)]: e.target.value })}
          />
          <label className="vf-check sm">
            <input type="checkbox" checked={f.secret} onChange={(e) => setExtra(extra.map((x, j) => (j === i ? { ...x, secret: e.target.checked } : x)))} /> Secret
          </label>
          <button className="btn t sm" aria-label="Remove field" onClick={() => setExtra(extra.filter((_, j) => j !== i))}>
            <X size={13} />
          </button>
        </div>
      ))}
      {t.id === "custom" && (
        <button className="btn t sm vf-addf" onClick={() => setExtra([...extra, { name: "", label: "", secret: true }])}>
          <Plus size={13} /> Add a field
        </button>
      )}

      {error && <p className="err">{error}</p>}
      <div className="vf-acts">
        <button className="btn p" disabled={busy || allDomains.length === 0 || missing.length > 0} onClick={() => void save()}>
          {busy ? <Loader2 size={14} className="spin" /> : <Check size={14} />} Save to the Vault
        </button>
        {allDomains.length === 0 && <span className="muted sm">Add the domain it is for.</span>}
      </div>
    </div>
  );
}

/** A domain as typed: `https://www.X.com/path` → `x.com`. */
export function host(raw: string): string | null {
  const s = raw.trim().toLowerCase().replace(/^[a-z]+:\/\//, "").split(/[/?#]/)[0].replace(/^.*@/, "").replace(/:\d+$/, "").replace(/\.$/, "").replace(/^www\./, "");
  return /^[a-z0-9.-]+\.[a-z0-9-]+$/.test(s) || s === "localhost" ? s : null;
}

/** Whether two domains belong together: the same, or one under the other. Enough to choose what to show; the daemon decides what is sent where. */
export function sameSite(a: string, b: string): boolean {
  const x = host(a) ?? a;
  const y = host(b) ?? b;
  return x === y || x.endsWith(`.${y}`) || y.endsWith(`.${x}`);
}

function slugField(name: string): string {
  return name.trim().toLowerCase().replace(/[^a-z0-9]+/g, "_").replace(/^_+|_+$/g, "").replace(/^(\d)/, "f_$1").slice(0, 48);
}
