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
import { AlertTriangle, Check, KeyRound, Loader2, Lock, Plus, Star, Trash2, X } from "lucide-react";
import { api, useAsync, useEvents, VAULT_EVENTS, type Credential, type CredentialTemplate } from "../api";
import { CredentialForm, host } from "./CredentialForm";
import { Favicon } from "./Favicon";
import "../profile.css";
import "../accounts.css";
import "../vault.css";

export function VaultPage() {
  const { slug = "" } = useParams();
  const location = useLocation();
  const navigate = useNavigate();
  const vault = useAsync(() => api.vault(), []);
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
            <CredentialDetails c={current} templates={vault.data.templates} onChange={vault.reload} onDeleted={() => (setHash(""), vault.reload())} />
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

function Drawer({ title, icon, sub, onClose, children }: { title: string; icon?: string; sub?: string; onClose: () => void; children: React.ReactNode }) {
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
            <span className="ic">
              <KeyRound size={22} />
            </span>
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
function CredentialDetails({ c, templates, onChange, onDeleted }: { c: Credential; templates: CredentialTemplate[]; onChange: () => void; onDeleted: () => void }) {
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
  const visible = c.fields.filter((f) => !(c.template === "oauth" && ["token", "refresh", "expires_at"].includes(f.name)));

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
        {c.template === "oauth" && (
          <p className="muted sm">
            Signed in through kiqr.cloud{c.scopes.length ? ` with access to ${c.scopes.length} ${c.scopes.length === 1 ? "thing" : "things"}` : ""}. Sign in again from Add a credential to give it more.
          </p>
        )}
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
