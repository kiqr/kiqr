// One ability, in full: what it is and what it includes, its instructions
// (read, edit, edit a copy, restore), how it connects for this project, and
// the switches and actions. Opened from any row or card on the Abilities page.
import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { Link } from "react-router-dom";
import { AnimatePresence, motion } from "motion/react";
import { AlertTriangle, Check, Download, FolderOpen, Globe, Loader2, Pencil, RefreshCw, RotateCcw, Trash2, X } from "lucide-react";
import { api, type Ability, type Contract } from "../api";
import { Markdown } from "../markdown";
import { SecretField } from "./SecretField";
import { Switch } from "./Switch";
import { InstallPanel } from "./Marketplace";
import { parseInstructions, TeachForm, type TeachDraft } from "./TeachForm";
import { Author, iconFor, needsOf, partsOf, stateOf } from "./abilityBits";

type Tab = "overview" | "instructions" | "connect";

export function AbilityModal({ slug, a, contracts, initialTab, onClose, onChange, onPrefer }: { slug: string; a: Ability; contracts: Contract[]; initialTab?: Tab; onClose: () => void; onChange: () => void; onPrefer: (contract: string, id: string) => void }) {
  const Icon = iconFor(a);
  const state = stateOf(a);
  const canConnect = a.installed && a.auth.length > 0;
  const [tab, setTab] = useState<Tab>(initialTab ?? "overview");
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [confirm, setConfirm] = useState(false);
  const [installing, setInstalling] = useState(false);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);
  useEffect(() => {
    if (tab === "connect" && !canConnect) setTab("overview");
  }, [tab, canConnect]);
  const run = async (what: string, f: () => Promise<unknown>, close = false) => {
    setBusy(what);
    setError(null);
    try {
      await f();
      onChange();
      if (close) onClose();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(null);
      setConfirm(false);
    }
  };
  const rivals = contracts.filter((c) => a.provides.some((p) => p.ability === c.id) && c.candidates.length > 1);
  const tabs: { id: Tab; label: string }[] = [{ id: "overview", label: "Overview" }, ...(a.instructions.length ? [{ id: "instructions" as Tab, label: "Instructions" }] : []), ...(canConnect ? [{ id: "connect" as Tab, label: "Connect" }] : [])];

  return createPortal(
    <motion.div className="lib-backdrop" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} transition={{ duration: 0.18 }} onClick={onClose}>
      <motion.div className="ab-modal" role="dialog" aria-label={a.name} initial={{ opacity: 0, y: 14, scale: 0.985 }} animate={{ opacity: 1, y: 0, scale: 1 }} exit={{ opacity: 0, y: 8, scale: 0.99 }} transition={{ duration: 0.22, ease: [0.2, 0.8, 0.2, 1] }} onClick={(e) => e.stopPropagation()}>
        <header className="ab-head">
          <span className={`ic ${state.tone}`}>
            <Icon size={22} />
          </span>
          <div className="min0">
            <h2>{a.name}</h2>
            <p className="by">
              by <Author a={a} />
              {a.version && <span> · v{a.version}</span>}
              {a.latest && a.version && a.latest !== a.version && <span className="warn"> · v{a.latest} available</span>}
              {a.cost && <span> · {a.cost}</span>}
              {a.core && <span> · part of KIQR</span>}
            </p>
          </div>
          <span className={`chip ${state.tone} sm`}>
            <b /> {state.text}
          </span>
          <button className="x" onClick={onClose} aria-label="Close">
            <X size={16} />
          </button>
        </header>
        {tabs.length > 1 && (
          <nav className="ab-tabs" aria-label="Sections">
            {tabs.map((t) => (
              <button key={t.id} className={tab === t.id ? "on" : ""} onClick={() => setTab(t.id)}>
                {tab === t.id && <motion.span layoutId="ab-tab" className="hl" transition={{ type: "spring", stiffness: 500, damping: 40 }} />}
                <span>{t.label}</span>
              </button>
            ))}
          </nav>
        )}
        <div className="ab-body">
          {tab === "overview" && <Overview slug={slug} a={a} onConnect={canConnect ? () => setTab("connect") : undefined} />}
          {tab === "instructions" && <Instructions slug={slug} a={a} onChange={onChange} />}
          {tab === "connect" && <Connect slug={slug} a={a} rivals={rivals} onChange={onChange} onPrefer={onPrefer} />}
          <AnimatePresence>
            {installing && (
              <InstallPanel
                a={a}
                busy={busy === "install"}
                error={error}
                onCancel={() => setInstalling(false)}
                onInstall={(secrets) =>
                  run("install", async () => {
                    await api.installAbility(a.id, { scope: "project", project: slug, secrets });
                    setInstalling(false);
                  })
                }
              />
            )}
          </AnimatePresence>
          {error && !installing && (
            <p className="err">
              <AlertTriangle size={13} /> {error}
            </p>
          )}
        </div>
        <footer className="ab-foot">
          {!a.installed ? (
            <button className="mk-install-btn" onClick={() => setInstalling((v) => !v)}>
              <Download size={14} /> {a.repo ? "Download and install" : "Install"}
            </button>
          ) : (
            <>
              <label className="ab-sw" title={a.machine_off ? "Switched off for the whole machine" : !a.ready ? "Connect it first" : undefined}>
                <Switch on={!a.paused && !a.machine_off && a.ready} disabled={!!busy || a.machine_off || !a.ready} onChange={(on) => run("project", () => (on ? api.resumeProjectAbility(slug, a.id) : api.pauseProjectAbility(slug, a.id)))} />
                <span>On for this project</span>
              </label>
              <label className="ab-sw" title={!a.ready ? "Needs its keys before it can be on" : undefined}>
                <Switch on={!a.machine_off && a.ready} disabled={!!busy || (!a.ready && (a.auth.length > 0 || a.program_found === false))} onChange={(on) => run("machine", () => (on ? api.resumeAbility(a.id) : api.pauseAbility(a.id)))} />
                <span>On for every project</span>
              </label>
              <span className="grow" />
              {(a.git || (a.source === "shipped" && !a.core)) && (
                <button className="btn t sm" disabled={!!busy} onClick={() => run("update", () => api.updateAbility(a.id))} title={a.git ? "Fetch the repository again" : "Refresh from this release"}>
                  {busy === "update" ? <Loader2 size={13} className="spin" /> : <RefreshCw size={13} />} Update
                </button>
              )}
              {!a.core &&
                (confirm ? (
                  <span className="mk-remove">
                    <span className="muted sm">Remove it from this machine?</span>
                    <button className="btn t sm bad" disabled={!!busy} onClick={() => run("remove", () => api.removeAbility(a.id, a.scope?.scope === "project" ? slug : undefined), true)}>
                      {busy === "remove" ? <Loader2 size={13} className="spin" /> : <Check size={13} />} Yes
                    </button>
                    <button className="btn t sm" onClick={() => setConfirm(false)}>
                      Keep
                    </button>
                  </span>
                ) : (
                  <button className="btn t sm" onClick={() => setConfirm(true)}>
                    <Trash2 size={13} /> Remove
                  </button>
                ))}
            </>
          )}
        </footer>
      </motion.div>
    </motion.div>,
    document.body,
  );
}

function Overview({ slug, a, onConnect }: { slug: string; a: Ability; onConnect?: () => void }) {
  const parts = partsOf(a, true);
  const needs = needsOf(a);
  return (
    <div className="ab-ov">
      <p className="ab-sum">{a.summary}</p>
      {a.description && (
        <p className="ab-when">
          <b>When Seemo uses it:</b> {a.description}
        </p>
      )}
      {a.categories.length > 0 && (
        <p className="tags">
          {a.categories.map((c) => (
            <span key={c} className="chip">
              {c}
            </span>
          ))}
          {a.trust === "verified" && <span className="chip ok">verified</span>}
          {a.trust === "community" && <span className="chip">community</span>}
        </p>
      )}
      <p className="cap">What's included</p>
      <ul className="ab-incl">
        {parts.map((p) => (
          <li key={p.label} className={p.tone ?? ""}>
            <p.icon size={15} />
            <div>
              <span>{p.label}</span>
              {p.detail && <small>{p.detail}</small>}
            </div>
          </li>
        ))}
      </ul>
      {needs.length > 0 && (
        <>
          <p className="cap">
            What it needs
            {a.auth.length > 0 && onConnect && (
              <button className="lnk ab-cap-act" onClick={onConnect}>
                Set up under Connect
              </button>
            )}
          </p>
          <ul className="ab-incl">
            {needs.map((p) => (
              <li key={p.label} className={p.tone ?? ""}>
                <p.icon size={15} />
                <div>
                  <span>{p.label}</span>
                  {p.detail && <small>{p.detail}</small>}
                </div>
              </li>
            ))}
          </ul>
        </>
      )}
      <dl className="ab-meta">
        <div>
          <dt>From</dt>
          <dd>{a.source === "core" ? "KIQR itself" : a.source === "shipped" ? "shipped with KIQR" : a.source === "marketplace" ? "the marketplace" : a.source === "yours" ? "you, taught here" : "a folder on this machine"}</dd>
        </div>
        {a.repo && (
          <div>
            <dt>Repository</dt>
            <dd>
              <a className="lnk" href={a.repo} target="_blank" rel="noreferrer">
                {a.repo.replace(/^https?:\/\//, "").replace(/\.git$/, "")}
              </a>
            </dd>
          </div>
        )}
        {a.homepage && (
          <div>
            <dt>Website</dt>
            <dd>
              <a className="lnk" href={a.homepage} target="_blank" rel="noreferrer">
                {new URL(a.homepage).hostname}
              </a>
            </dd>
          </div>
        )}
        {a.folder && (
          <div>
            <dt>Folder</dt>
            <dd className="mono">
              <FolderOpen size={12} /> {a.folder}
            </dd>
          </div>
        )}
        {a.scope?.scope === "project" && (
          <div>
            <dt>Installed for</dt>
            <dd>this project only</dd>
          </div>
        )}
      </dl>
      {a.channels.length > 0 && (
        <p className="muted sm">
          Where it publishes is set up per project under <Link to={`/p/${slug}/channels`}>Channels</Link>.
        </p>
      )}
    </div>
  );
}

/** Read, edit a copy, restore. Several files for a core ability. */
function Instructions({ slug, a, onChange }: { slug: string; a: Ability; onChange: () => void }) {
  const [file, setFile] = useState(a.instructions[0]?.file ?? "");
  const current = a.instructions.find((f) => f.file === file) ?? a.instructions[0];
  const [text, setText] = useState<string | null>(null);
  const [edit, setEdit] = useState<{ scope: "project" | "global"; body: string; front: string } | null>(null);
  const [teach, setTeach] = useState<TeachDraft | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const yours = a.source === "yours";
  useEffect(() => {
    setText(null);
    setEdit(null);
    api.instructions(slug, a.id, current?.file).then(setText, (e) => setError((e as Error).message));
  }, [slug, a.id, current?.file, a.instructions]);
  if (!current) return null;
  const startEdit = async (scope: "project" | "global") => {
    setError(null);
    try {
      const t = await api.instructions(scope === "project" ? slug : null, a.id, current.file);
      const p = parseInstructions(t);
      if (yours) setTeach({ name: p.name || a.name, description: p.description || a.description, text: p.body, everywhere: scope === "global", editing: a.id });
      else setEdit({ scope, body: p.body, front: p.front });
    } catch (e) {
      setError((e as Error).message);
    }
  };
  const save = async () => {
    if (!edit) return;
    setBusy("save");
    setError(null);
    try {
      const body = edit.front ? `---\n${edit.front}\n---\n\n${edit.body.trim()}\n` : edit.body;
      await api.saveInstructions(edit.scope === "project" ? slug : null, a.id, body, current.file);
      setEdit(null);
      onChange();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(null);
    }
  };
  const restore = async (scope: "project" | "global") => {
    setBusy("restore");
    setError(null);
    try {
      await api.deleteInstructions(scope === "project" ? slug : null, a.id, current.file);
      onChange();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(null);
    }
  };
  const parsed = text !== null ? parseInstructions(text) : null;
  const ownScope: "project" | "global" = a.scope?.scope === "project" ? "project" : "global";
  return (
    <div className="ab-ins">
      {a.instructions.length > 1 && (
        <div className="ab-files">
          {a.instructions.map((f) => (
            <button key={f.file} className={`chip ${f.file === current.file ? "acc" : ""}`} onClick={() => setFile(f.file)}>
              {f.label}
              {(f.project_copy || f.machine_copy) && <small> · edited</small>}
            </button>
          ))}
        </div>
      )}
      <p className="muted sm ab-where">
        {yours ? (
          <>Your own, kept for {ownScope === "project" ? "this project" : "every project"}. Seemo reads it before matching requests.</>
        ) : current.project_copy ? (
          <>This project has its own version. {current.machine_copy ? "The machine's is edited too." : ""}</>
        ) : current.machine_copy ? (
          <>Edited for every project; the shipped text is not in use.</>
        ) : (
          <>The text as shipped. Edit a copy and Seemo follows yours instead.</>
        )}
      </p>
      {teach ? (
        <TeachForm slug={slug} draft={teach} setDraft={setTeach} onDone={() => (setTeach(null), onChange())} />
      ) : edit ? (
        <div className="ab-edit">
          <textarea className="input" rows={16} value={edit.body} onChange={(e) => setEdit({ ...edit, body: e.target.value })} />
          <div className="acts">
            <button className="btn p sm" disabled={busy === "save" || edit.body.trim().length < 3} onClick={() => void save()}>
              {busy === "save" ? <Loader2 size={13} className="spin" /> : <Check size={13} />} Save for {edit.scope === "project" ? "this project" : "every project"}
            </button>
            <button className="btn t sm" onClick={() => setEdit(null)}>
              Cancel
            </button>
          </div>
        </div>
      ) : (
        <>
          <div className="ab-text">{parsed === null ? <Loader2 size={14} className="spin" /> : <Markdown text={parsed.body} />}</div>
          <div className="acts">
            {yours ? (
              <button className="btn g sm" onClick={() => void startEdit(ownScope)}>
                <Pencil size={13} /> Edit
              </button>
            ) : (
              <>
                <button className="btn g sm" onClick={() => void startEdit("project")}>
                  <Pencil size={13} /> {current.project_copy ? "Edit this project's version" : "Edit a copy for this project"}
                </button>
                <button className="btn t sm" onClick={() => void startEdit("global")}>
                  <Globe size={13} /> {current.machine_copy ? "Edit for every project" : "Edit a copy for every project"}
                </button>
                {current.project_copy && (
                  <button className="btn t sm" disabled={busy === "restore"} onClick={() => void restore("project")}>
                    <RotateCcw size={13} /> Drop this project's version
                  </button>
                )}
                {current.machine_copy && a.builtin && (
                  <button className="btn t sm" disabled={busy === "restore"} onClick={() => void restore("global")}>
                    <RotateCcw size={13} /> Back to the shipped text
                  </button>
                )}
              </>
            )}
          </div>
        </>
      )}
      {error && (
        <p className="err">
          <AlertTriangle size={13} /> {error}
        </p>
      )}
    </div>
  );
}

/** Keys for this project, or for every project; which one serves a contract. */
function Connect({ slug, a, rivals, onChange, onPrefer }: { slug: string; a: Ability; rivals: Contract[]; onChange: () => void; onPrefer: (contract: string, id: string) => void }) {
  const [scope, setScope] = useState<"project" | "global">(a.channels.length ? "project" : a.auth.some((k) => k.set === "project") ? "project" : "global");
  const machineHas = a.auth.filter((k) => !k.optional).every((k) => !!k.set);
  return (
    <div className="ab-con">
      <div className="scope">
        <span className="nm">Keys for</span>
        <button className={`chip ${scope === "project" ? "acc" : ""}`} onClick={() => setScope("project")}>
          This project
        </button>
        <button className={`chip ${scope === "global" ? "acc" : ""}`} onClick={() => setScope("global")}>
          <Globe size={12} /> Every project
        </button>
      </div>
      <div className="set-keys">
        {a.auth.map((k) => (
          <SecretField key={`${scope}-${k.env}`} name={k.env} label={k.label || k.env} help={k.help} set={scope === "project" ? (k.set === "project" ? "project" : null) : k.set === "global" ? "global" : null} optional={k.optional || (scope === "project" && !!k.set)} slug={scope === "project" ? slug : undefined} onSaved={onChange} />
        ))}
      </div>
      <p className="muted sm">
        {scope === "project"
          ? machineHas
            ? "The machine's keys are in use here; a value saved for this project replaces them for this project only."
            : "Saved for this project only, never shown again."
          : "The machine's keys: every project without its own uses them."}
      </p>
      {rivals.map((c) => (
        <div key={c.id} className="ab-prefer">
          <span className="nm">{c.label} with</span>
          {c.candidates.map((x) => (
            <button key={x.id} className={`chip ${(c.active?.id ?? "") === x.id ? "acc" : ""}`} disabled={!x.ready} title={x.ready ? undefined : "Needs setup"} onClick={() => onPrefer(c.id, x.id)}>
              {x.name}
            </button>
          ))}
        </div>
      ))}
    </div>
  );
}
