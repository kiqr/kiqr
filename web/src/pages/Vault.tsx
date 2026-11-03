// Setup → Vault: the credentials Seemo may be given, for the whole machine.
// Each is a login, a key, a signed-in app or a mail server, scoped to the
// domains it is for (the first is its icon). The Vault knows nothing else:
// which abilities may use a credential is allowed per project, and a channel
// picks the one it publishes as. Values go in and never come back out.
// `#credential=<id>` opens one; `#add` opens the Add panel.
import { useEffect, useMemo, useState } from "react";
import { createPortal } from "react-dom";
import { useLocation, useNavigate, useParams } from "react-router-dom";
import { AnimatePresence, motion } from "motion/react";
import { AlertTriangle, Check, Hand, KeyRound, LogIn, LogOut, Loader2, Lock, Plus, Star, Trash2, X } from "lucide-react";
import { api, SESSION_EVENTS, useAsync, useEvents, VAULT_EVENTS, type Credential, type CredentialTemplate } from "../api";
import { credStateChip, sentence } from "./Access";
import { CredentialAbilities } from "./Recommended";
import { Hands } from "./Hands";
import { CredentialForm, host } from "./CredentialForm";
import { Favicon } from "./Favicon";
import "../profile.css";
import "../accounts.css";
import "../vault.css";

export function VaultPage() {
  const { slug = "" } = useParams();
  const location = useLocation();
  const navigate = useNavigate();
  const vault = useAsync(() => api.vault(slug), [slug]);
  useEvents(slug, (e) => SESSION_EVENTS.includes(e.type) && vault.reload(), { demo: false });
  const access = useAsync(() => api.access(slug), [slug]);
  useEvents(null, (e) => VAULT_EVENTS.includes(e.type) && (vault.reload(), access.reload()), { demo: false });
  const open = decodeURIComponent(/#credential=([^&]+)/.exec(location.hash)?.[1] ?? "");
  const adding = location.hash === "#add";
  const setHash = (h: string) => navigate({ hash: h }, { replace: true });

  const all = vault.data?.credentials ?? [];
  const used = useMemo(() => new Set(Object.values(access.data?.grants ?? {}).flat()), [access.data]);
  const here = all.filter((c) => used.has(c.id));
  const other = all.filter((c) => !used.has(c.id));
  const current = all.find((c) => c.id === open);
  const status = vault.data?.status;

  return (
    <div className="prof">
      <div className="cm-head">
        <div>
          <p className="cap">Setup</p>
          <h1 className="d2">Vault</h1>
          <p className="muted">Logins, keys and sign-ins Seemo can be given, kept encrypted on this machine. A credential does nothing on its own: you choose which abilities may use it, and which one a channel publishes as.</p>
        </div>
        <button className="btn p" disabled={!status?.unlocked} onClick={() => setHash("add")}>
          <Plus size={15} /> Add a credential
        </button>
      </div>

      {vault.error && <p className="err">{vault.error}</p>}
      {status && !status.unlocked && (
        <div className="vt-locked">
          <Lock size={18} />
          <div>
            <b>The vault is locked</b>
            <p className="muted">{status.problem ?? "Its key was not found."} Give KIQR its key with KIQR_VAULT_KEY_FILE, or put the key back where it was, and restart.</p>
          </div>
        </div>
      )}

      {vault.data && status?.unlocked && all.length === 0 && (
        <div className="pcard none">
          <KeyRound size={22} className="ac2" />
          <p className="t">Nothing in the Vault yet</p>
          <p className="muted">Sign in with Google or GitHub in one click, or add a login, an API key or the server your email goes out through.</p>
          <button className="btn g" onClick={() => setHash("add")}>
            <Plus size={14} /> Add a credential
          </button>
        </div>
      )}

      <div className="acx-groups">
        {here.length > 0 && <Section title="Used in this project" items={here} onOpen={(id) => setHash(`credential=${encodeURIComponent(id)}`)} />}
        {other.length > 0 && <Section title={here.length ? "Not used in this project" : "Your credentials"} items={other} onOpen={(id) => setHash(`credential=${encodeURIComponent(id)}`)} />}
      </div>

      {status?.unlocked && all.length > 0 && (
        <p className="vt-key muted sm">
          <Lock size={12} /> Encrypted with {status.key_text}.
          {status.key === "local_file" && " Anyone who can read this machine's KIQR folder can open it: keep the disk encrypted, or give KIQR its key from elsewhere (KIQR_VAULT_KEY_FILE)."}
        </p>
      )}

      <AnimatePresence>
        {adding && vault.data && (
          <Drawer key="add" title="Add a credential" onClose={() => setHash("")}>
            <CredentialForm templates={vault.data.templates} onSaved={(c) => (vault.reload(), setHash(`credential=${encodeURIComponent(c.id)}`))} onCancel={() => setHash("")} />
          </Drawer>
        )}
        {current && vault.data && (
          <Drawer key={current.id} title={current.label} icon={current.domains[0]} sub={current.identity ?? kindName(current, vault.data.templates)} onClose={() => setHash("")}>
            <CredentialDetails slug={slug} c={current} templates={vault.data.templates} handsFirst={/[#&]hands\b/.test(location.hash)} onChange={vault.reload} onDeleted={() => (setHash(""), vault.reload())} />
          </Drawer>
        )}
        {open && vault.data && !current && (
          <Drawer key="gone" title="Not in the Vault" onClose={() => setHash("")}>
            <p className="muted">There is no credential “{open}” any more.</p>
          </Drawer>
        )}
      </AnimatePresence>
    </div>
  );
}

function kindName(c: Credential, templates: CredentialTemplate[]): string {
  return templates.find((t) => t.id === c.template)?.label ?? c.template;
}

function Section({ title, items, onOpen }: { title: string; items: Credential[]; onOpen: (id: string) => void }) {
  return (
    <section className="acx-group">
      <h2>
        {title} <span className="n">{items.length}</span>
      </h2>
      <div className="vt-list">
        {items.map((c) => (
          <button key={c.id} className="vt-row" onClick={() => onOpen(c.id)}>
            <Favicon domain={c.domains[0]} size={32} />
            <span className="min0 nm">
              <b>{c.label}</b>
              <small>{c.identity && c.identity !== c.label ? c.identity : c.provider ? `Signed in through kiqr.cloud` : templateWord(c.template)}</small>
            </span>
            <span className="vt-doms">
              {c.domains.slice(0, 3).map((d) => (
                <span key={d} className="mono">
                  {d}
                </span>
              ))}
              {c.domains.length > 3 && <span className="mono">+{c.domains.length - 3}</span>}
            </span>
            <CredentialChip c={c} />
          </button>
        ))}
      </div>
    </section>
  );
}

function templateWord(t: string): string {
  return { login: "Login", api_key: "API key", oauth: "Signed-in app", smtp: "Mail server" }[t] ?? "Credential";
}

/** Whether the credential is complete. Sessions and expiry join this once abilities use credentials. */
export function CredentialChip({ c }: { c: Credential }) {
  // Not working here (signed out, needs a hand, a program missing, a token to renew): as the modal says it.
  if (c.state && c.state !== "ready" && c.state !== "missing_fields") {
    const chip = credStateChip(c.state);
    return (
      <span className={`chip ${chip.cls} sm`} title={c.state_text}>
        {chip.spin ? <Loader2 size={11} className="spin" /> : <b />} {chip.text}
      </span>
    );
  }
  if (c.missing.length)
    return (
      <span className="chip warn sm">
        <b /> Missing {c.missing.length === 1 ? c.fields.find((f) => f.name === c.missing[0])?.label.toLowerCase() ?? c.missing[0] : `${c.missing.length} fields`}
      </span>
    );
  return (
    <span className="chip ok sm">
      <b /> Complete
    </span>
  );
}

/** A modal for one thing (a credential, a channel) or an Add panel; `icon` is a domain whose favicon heads it. */
export function Drawer({ title, icon, iconNode, sub, onClose, children }: { title: string; icon?: string; iconNode?: React.ReactNode; sub?: string; onClose: () => void; children: React.ReactNode }) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);
  return createPortal(
    <motion.div className="lib-backdrop" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} transition={{ duration: 0.18 }} onClick={onClose}>
      <motion.div className="ab-modal vt-modal" role="dialog" aria-label={title} initial={{ opacity: 0, y: 14, scale: 0.985 }} animate={{ opacity: 1, y: 0, scale: 1 }} exit={{ opacity: 0, y: 8, scale: 0.99 }} transition={{ duration: 0.22, ease: [0.2, 0.8, 0.2, 1] }} onClick={(e) => e.stopPropagation()}>
        <header className="ab-head">
          {icon !== undefined ? (
            <Favicon domain={icon} size={46} />
          ) : (
            <span className="ic">{iconNode ?? <KeyRound size={22} />}</span>
          )}
          <div className="min0">
            <h2>{title}</h2>
            {sub && <p className="by">{sub}</p>}
          </div>
          <button className="x" onClick={onClose} aria-label="Close">
            <X size={16} />
          </button>
        </header>
        <div className="ab-body">{children}</div>
      </motion.div>
    </motion.div>,
    document.body,
  );
}

/** One credential: its fields (set or not, replaced but never shown), its domains, and delete. */
function CredentialDetails({ slug, c, templates, handsFirst, onChange, onDeleted }: { slug: string; c: Credential; templates: CredentialTemplate[]; handsFirst?: boolean; onChange: () => void; onDeleted: () => void }) {
  const [label, setLabel] = useState(c.label);
  const [editing, setEditing] = useState<string | null>(null);
  const [value, setValue] = useState("");
  const [domainText, setDomainText] = useState("");
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [confirm, setConfirm] = useState(false);
  useEffect(() => setLabel(c.label), [c.label]);

  const run = async (what: string, f: () => Promise<unknown>) => {
    setBusy(what);
    setError(null);
    try {
      await f();
      onChange();
      return true;
    } catch (e) {
      setError((e as Error).message);
      return false;
    } finally {
      setBusy(null);
    }
  };
  const setDomains = (domains: string[]) => run("domains", () => api.updateCredential(c.id, { domains }));
  const addDomain = () => {
    const d = host(domainText);
    if (d && !c.domains.includes(d)) void setDomains([...c.domains, d]).then((ok) => ok && setDomainText(""));
  };
  const saveField = (name: string) => run(name, () => api.updateCredential(c.id, { values: { [name]: value } })).then((ok) => ok && (setEditing(null), setValue("")));
  // A signed-in app's fields are the sign-in's own: shown as what it may do, renewed by signing in again, never typed.
  const visible = c.template === "oauth" ? [] : c.fields;

  return (
    <div className="vt-detail">
      <section>
        <p className="cap">Name</p>
        <div className="vt-inline">
          <input className="input" value={label} onChange={(e) => setLabel(e.target.value)} onKeyDown={(e) => e.key === "Enter" && label.trim() !== c.label && void run("label", () => api.updateCredential(c.id, { label }))} />
          {label.trim() && label.trim() !== c.label && (
            <button className="btn g sm" disabled={busy === "label"} onClick={() => void run("label", () => api.updateCredential(c.id, { label }))}>
              <Check size={13} /> Save
            </button>
          )}
        </div>
      </section>

      <section>
        <p className="cap">Where it is used</p>
        <div className="vt-domlist">
          {c.domains.map((d, i) => (
            <div key={d} className="vt-dom">
              <Favicon domain={d} size={20} />
              <span className="mono grow">{d}</span>
              {i === 0 ? (
                <span className="chip acc sm">
                  <Star size={11} /> Home
                </span>
              ) : (
                <button className="btn t sm" disabled={!!busy} onClick={() => void setDomains([d, ...c.domains.filter((x) => x !== d)])}>
                  Make home
                </button>
              )}
              {c.domains.length > 1 && (
                <button className="btn t sm" aria-label={`Remove ${d}`} disabled={!!busy} onClick={() => void setDomains(c.domains.filter((x) => x !== d))}>
                  <X size={13} />
                </button>
              )}
            </div>
          ))}
          <div className="vt-inline">
            <input className="input mono" placeholder="Add a domain" value={domainText} onChange={(e) => setDomainText(e.target.value)} onKeyDown={(e) => e.key === "Enter" && addDomain()} />
            {domainText.trim() && (
              <button className="btn g sm" disabled={!host(domainText) || !!busy} onClick={addDomain}>
                <Plus size={13} /> Add
              </button>
            )}
          </div>
          <small className="muted">Its values are only ever sent to these domains, whatever an ability asks.</small>
        </div>
      </section>

      <section>
        <p className="cap">{kindName(c, templates)}</p>
        {c.template === "oauth" && <SignedInApp c={c} />}
        <div className="vt-fields">
          {visible.map((f) => (
            <div key={f.name} className="vt-field">
              <span className="lb">
                {f.label} {f.optional && <small>optional</small>}
              </span>
              {editing === f.name ? (
                <div className="vt-inline">
                  <input
                    autoFocus
                    className={`input ${f.kind === "text" ? "" : "mono"}`}
                    type={f.kind === "secret" || f.kind === "totp" ? "password" : "text"}
                    autoComplete="off"
                    placeholder={f.kind === "totp" ? "The setup key (base32)" : f.set ? "The new value" : ""}
                    value={value}
                    onChange={(e) => setValue(e.target.value)}
                    onKeyDown={(e) => e.key === "Enter" && void saveField(f.name)}
                  />
                  <button className="btn p sm" disabled={busy === f.name} onClick={() => void saveField(f.name)}>
                    {busy === f.name ? <Loader2 size={13} className="spin" /> : <Check size={13} />} Save
                  </button>
                  <button className="btn t sm" onClick={() => (setEditing(null), setValue(""))}>
                    Cancel
                  </button>
                </div>
              ) : (
                <div className="vt-inline">
                  <span className={`val ${f.value ? "" : "dim"} ${f.kind === "text" ? "" : "mono"}`}>{f.value ?? (f.set ? (f.kind === "totp" ? "Key set" : "••••••••") : "Not set")}</span>
                  <button className="btn t sm" onClick={() => (setEditing(f.name), setValue(f.value ?? ""))}>
                    {f.set ? (f.value ? "Change" : "Replace") : "Set"}
                  </button>
                  {f.set && f.optional && (
                    <button className="btn t sm" disabled={!!busy} onClick={() => void run(f.name, () => api.updateCredential(c.id, { values: { [f.name]: "" } }))}>
                      Clear
                    </button>
                  )}
                </div>
              )}
            </div>
          ))}
        </div>
      </section>

      <CredentialSession slug={slug} c={c} handsFirst={handsFirst} />

      <section>
        <p className="cap">Abilities that can use it</p>
        <CredentialAbilities slug={slug} credential={c.id} />
      </section>

      <CredentialLog id={c.id} />

      {error && (
        <p className="err">
          <AlertTriangle size={13} /> {error}
        </p>
      )}

      <div className="acx-acts">
        <span className="grow" />
        {confirm ? (
          <>
            <span className="muted sm">Delete it? Every project's abilities lose it.</span>
            <button className="btn t sm" onClick={() => setConfirm(false)}>
              Keep it
            </button>
            <button className="btn g sm bad" disabled={busy === "delete"} onClick={() => void run("delete", () => api.deleteCredential(c.id)).then((ok) => ok && onDeleted())}>
              <Trash2 size={13} /> Delete
            </button>
          </>
        ) : (
          <button className="btn t sm" onClick={() => setConfirm(true)}>
            <Trash2 size={13} /> Delete
          </button>
        )}
      </div>
    </div>
  );
}

/** A login's sign-in on its browser, when an ability here signs it in: how it stands, Sign in now, the live view, Sign out. */
function CredentialSession({ slug, c, handsFirst }: { slug: string; c: Credential; handsFirst?: boolean }) {
  const st = useAsync(() => api.credentialSession(slug, c.id).catch(() => null), [slug, c.id]);
  useEvents(slug, (e) => SESSION_EVENTS.includes(e.type) && e.credential === c.id && st.reload(), { demo: false });
  const [hands, setHands] = useState(!!handsFirst);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const s = st.data;
  if (!s?.has_session) return null;
  const chip = credStateChip(s.session.state === "signed_in" ? "ready" : s.session.state || undefined);
  const run = async (what: string, f: () => Promise<unknown>) => {
    setBusy(what);
    setError(null);
    try {
      await f();
      st.reload();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(null);
    }
  };
  return (
    <section>
      <p className="cap">Signed in on Seemo's browser</p>
      <div className="vt-inline">
        <span className={`chip sm ${chip.cls}`}>
          {chip.spin ? <Loader2 size={11} className="spin" /> : <b />} {chip.text}
        </span>
        {s.session.message && <span className="muted sm">{sentence(s.session.message)}.</span>}
      </div>
      <div className="vt-inline">
        {!s.by_hand && s.has_login && (
          <button className="btn g sm" disabled={!!busy} onClick={() => void run("in", () => api.credentialSignIn(slug, c.id))}>
            {busy === "in" ? <Loader2 size={13} className="spin" /> : <LogIn size={13} />} Sign in now
          </button>
        )}
        {s.live_view && (
          <button className="btn g sm" onClick={() => setHands(true)}>
            <Hand size={13} /> Take the controls
          </button>
        )}
        {s.session.state === "signed_in" && (
          <button className="btn t sm" disabled={!!busy} onClick={() => void run("out", () => api.credentialSignOut(slug, c.id))}>
            <LogOut size={13} /> Sign out
          </button>
        )}
      </div>
      {!s.has_login && !s.by_hand && <p className="muted sm">Add its password above to let Seemo sign in by itself, or sign in once with the controls.</p>}
      {error && <p className="err sm">{error}</p>}
      {hands && <Hands slug={slug} account={c.id} name={c.label} startUrl={s.start_url ?? undefined} onClose={() => (setHands(false), st.reload())} />}
    </section>
  );
}

/** What abilities did as it, newest first, in every project. */
function CredentialLog({ id }: { id: string }) {
  const log = useAsync(() => api.credentialActivity(id, 8).catch(() => null), [id]);
  const items = log.data?.activity ?? [];
  if (!items.length) return null;
  return (
    <section>
      <p className="cap">What it did</p>
      <ul className="vt-log">
        {items.map((a, i) => (
          <li key={i}>
            <span className="mono dim">{new Date(a.at).toLocaleString([], { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" })}</span>
            <span className="grow">
              {a.role} by {a.ability}
              {a.target ? ` on ${a.target}` : ""}
              {a.project ? <span className="dim"> · {a.project}</span> : null}
            </span>
            {a.url && (
              <a className="lnk sm" href={a.url} target="_blank" rel="noreferrer">
                open
              </a>
            )}
          </li>
        ))}
      </ul>
    </section>
  );
}

/** Scopes that only keep a sign-in alive or say who it is: not something to list as access. */
const PLUMBING_SCOPES = ["offline_access", "openid", "profile", "email"];

/** A signed-in app: who it is (when the provider said), what it may do in words, and Sign in again to renew it or give it more. */
function SignedInApp({ c }: { c: Credential }) {
  const providers = useAsync(() => api.signInProviders().catch(() => null), []);
  const p = providers.data?.providers.find((x) => x.id === c.provider);
  const [busy, setBusy] = useState(false);
  const [waiting, setWaiting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEvents(null, (e) => {
    if (e.type !== "credential_connected" || e.credential !== c.id || !waiting) return;
    setWaiting(false);
    if (!e.ok) setError(String(e.error ?? "the sign-in did not finish"));
  }, { demo: false });
  const shown = c.scopes.filter((s) => !PLUMBING_SCOPES.includes(s));
  const label = (s: string) => p?.scopes.find((x) => x.scope === s)?.label ?? s.replace(/^https:\/\/www\.googleapis\.com\/auth\//, "");
  const more = (p?.scopes ?? []).filter((x) => !c.scopes.includes(x.scope));
  const again = async () => {
    if (!c.provider) return;
    setBusy(true);
    setError(null);
    try {
      // Everything it has, and everything the provider offers: signing in again only ever adds.
      const scopes = [...new Set([...c.scopes, ...(p?.scopes ?? []).map((x) => x.scope)])];
      const r = await api.vaultConnect(c.provider, { origin: window.location.origin, scopes, credential: c.id });
      window.open(r.url, "_blank", "noopener");
      setWaiting(true);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="vt-app">
      <p className="sm">
        Signed in with {p?.name ?? c.provider ?? "an app"} through kiqr.cloud{c.identity ? <> as <b>{c.identity}</b></> : ""}.
      </p>
      {shown.length > 0 ? (
        <ul className="vt-scopes">
          {shown.map((s) => (
            <li key={s}>
              <Check size={12} /> {label(s)}
            </li>
          ))}
        </ul>
      ) : (
        <p className="muted sm">It was given no access beyond signing in.</p>
      )}
      {more.length > 0 && <p className="muted sm">It could also: {more.map((x) => x.label).join(", ")}.</p>}
      <div className="vt-inline">
        {c.provider && (
          <button className="btn g sm" disabled={busy || waiting} onClick={() => void again()}>
            {busy || waiting ? <Loader2 size={13} className="spin" /> : <LogIn size={13} />} {more.length ? "Sign in again to give it more" : "Sign in again"}
          </button>
        )}
        {waiting && <span className="muted sm">Waiting for you to approve it in the new tab…</span>}
      </div>
      {error && <p className="err sm">{error}</p>}
    </div>
  );
}
