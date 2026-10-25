// The marketplace: what Seemo could do, from the index kiqr.cloud hosts plus
// what ships with KIQR. Installing is for the machine; the keys typed on
// install are this project's. Advanced: a folder or git URL, broken folders.
import { useMemo, useState } from "react";
import { AnimatePresence, motion } from "motion/react";
import { AlertTriangle, BookOpen, ChevronDown, ChevronRight, Download, FolderOpen, Loader2, Search, Terminal, X } from "lucide-react";
import { api, type Ability, type Scope } from "../api";
import { AbilityIcon, Author } from "./abilityBits";

const SHELVES: { id: string; label: string }[] = [
  { id: "all", label: "All" },
  { id: "web", label: "Web" },
  { id: "writing", label: "Writing" },
  { id: "images", label: "Images" },
  { id: "video", label: "Video" },
  { id: "music", label: "Music" },
  { id: "email", label: "Email" },
  { id: "publishing", label: "Publishing" },
  { id: "browser", label: "Browser" },
  { id: "tools", label: "Tools" },
];

export function Marketplace({ slug, items, index, indexFetched, loaded, onOpen, onChange }: { slug: string; items: Ability[]; index: string; indexFetched?: boolean; loaded: boolean; onOpen: (a: Ability) => void; onChange: () => void }) {
  const [q, setQ] = useState("");
  const [shelf, setShelf] = useState("all");
  const [author, setAuthor] = useState("all");
  const shown = useMemo(() => {
    const needle = q.trim().toLowerCase();
    return items
      .filter((it) => (shelf === "all" ? true : it.categories.includes(shelf)))
      .filter((it) => author === "all" || it.author === author)
      .filter((it) => !needle || [it.name, it.summary, ...it.provides.map((p) => p.label), ...it.categories, it.author].join(" ").toLowerCase().includes(needle))
      .sort((a, b) => a.name.localeCompare(b.name));
  }, [items, q, shelf, author]);
  const authors = useMemo(() => {
    const n = new Map<string, number>();
    for (const it of items) n.set(it.author, (n.get(it.author) ?? 0) + 1);
    return [...n.entries()].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]));
  }, [items]);
  return (
    <section className="mk-sec" id="marketplace">
      <h2>
        Marketplace <span className="n">{items.length}</span>
      </h2>
      <p className="cm-sec-ds">More abilities, by KIQR and by others. Installed once for this machine, then switched on and connected per project.</p>
      <div className="mk-bar">
        <label className="mk-search">
          <Search size={15} />
          <input className="input" placeholder="Search abilities" value={q} onChange={(e) => setQ(e.target.value)} />
          {q && (
            <button className="x" onClick={() => setQ("")} aria-label="Clear">
              <X size={13} />
            </button>
          )}
        </label>
        <div className="mk-shelves">
          {SHELVES.map((s) => (
            <button key={s.id} className={`chip ${shelf === s.id ? "acc" : ""}`} onClick={() => setShelf(s.id)}>
              {s.label}
              {s.id === "all" ? ` ${items.length}` : ""}
            </button>
          ))}
        </div>
      </div>
      {authors.length > 1 && (
        <div className="mk-authors">
          <span className="lb">By</span>
          <button className={`chip ${author === "all" ? "acc" : ""}`} onClick={() => setAuthor("all")}>
            Anyone
          </button>
          {authors.map(([a, n]) => (
            <button key={a} className={`chip ${author === a ? "acc" : ""}`} onClick={() => setAuthor(author === a ? "all" : a)}>
              {a} <small>{n}</small>
            </button>
          ))}
        </div>
      )}
      {loaded && shown.length === 0 && (
        <p className="muted set-note">
          {q || shelf !== "all" || author !== "all" ? "Nothing here matches." : indexFetched === false ? `The marketplace index at ${index || "the configured URL"} could not be reached, so only what ships with KIQR is listed.` : "Everything in the index is installed."} {indexFetched === false ? "Check the URL under Settings → General, or install from a folder or git URL under Advanced." : `The index at ${index || "kiqr.cloud"} grows over time; your own folders install under Advanced.`}
        </p>
      )}
      <div className="mk-grid">
        {shown.map((it, i) => (
          <MarketCard key={it.id} slug={slug} a={it} i={i} onOpen={() => onOpen(it)} onChange={onChange} />
        ))}
      </div>
    </section>
  );
}

function MarketCard({ slug, a, i, onOpen, onChange }: { slug: string; a: Ability; i: number; onOpen: () => void; onChange: () => void }) {
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const install = async (secrets: Record<string, string>) => {
    setBusy(true);
    setError(null);
    try {
      await api.installAbility(a.id, { scope: "project", project: slug, secrets });
      setOpen(false);
      onChange();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  return (
    <motion.article className="mk-card st-available" initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }} transition={{ delay: Math.min(i, 12) * 0.03 }} onClick={onOpen}>
      <header>
        <span className={`ic ${a.avatar ? "has-avatar" : ""}`}>
          <AbilityIcon a={a} size={a.avatar ? 40 : 18} />
        </span>
        <div className="min0">
          <h3>{a.name}</h3>
          <p className="by">
            by <Author a={a} />
            {a.trust === "verified" && <span className="chip ok sm"> verified</span>}
            {a.version && <span> · v{a.version}</span>}
            {a.cost && <span> · {a.cost}</span>}
          </p>
        </div>
      </header>
      <p className="ds">{a.summary}</p>
      <p className="tags">
        {a.instructions.length > 0 || a.transport === "instructions" ? (
          <span className="chip">
            <BookOpen size={11} /> instructions
          </span>
        ) : null}
        {a.provides.map((p) => (
          <span key={p.ability} className="chip">
            {p.label}
          </span>
        ))}
        {a.channels.map((c) => (
          <span key={c} className="chip">
            Publishes to {c}
          </span>
        ))}
        {a.auth.length > 0 && <span className="chip dimmed">{a.channels.length ? "your account" : "needs a key"}</span>}
        {a.tools > 0 && <span className="chip">{a.tools} tools</span>}
        {a.runs_program && (
          <span className="chip warn" title={a.command ?? ""}>
            <Terminal size={11} /> runs a program here
          </span>
        )}
        {a.hosts.length > 0 && <span className="chip dimmed">calls {a.hosts.join(", ")}</span>}
      </p>
      <AnimatePresence>{open && <InstallPanel a={a} busy={busy} error={error} onCancel={() => setOpen(false)} onInstall={(s) => void install(s)} />}</AnimatePresence>
      <footer onClick={(e) => e.stopPropagation()}>
        <button className={`mk-install-btn ${open ? "open" : ""}`} onClick={() => setOpen((o) => !o)}>
          <Download size={14} /> {open ? "Cancel" : "Install"}
        </button>
        <button className="btn t sm" onClick={onOpen}>
          Details
        </button>
        {a.homepage && (
          <a className="lnk sm" href={a.homepage} target="_blank" rel="noreferrer">
            {new URL(a.homepage).hostname}
          </a>
        )}
      </footer>
    </motion.article>
  );
}

/** Install is for the machine; the keys typed here are this project's. Abilities that publish bring an account per project instead, when a channel is added. */
export function InstallPanel({ a, busy, error, onCancel, onInstall }: { a: Ability; busy: boolean; error: string | null; onCancel: () => void; onInstall: (secrets: Record<string, string>) => void }) {
  const perProject = a.channels.length > 0;
  const [secrets, setSecrets] = useState<Record<string, string>>({});
  const required = perProject ? [] : a.auth.filter((k) => !k.optional);
  // One that signs in with a button is connected after it is installed, from its Connect tab.
  const ok = !!a.connect || required.every((k) => (secrets[k.env] ?? "").trim() || k.set);
  return (
    <motion.div className="mk-install" initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: "auto" }} exit={{ opacity: 0, height: 0 }} transition={{ duration: 0.2 }} onClick={(e) => e.stopPropagation()}>
      {perProject ? (
        <p className="muted sm">Installed once for this machine. Each project connects its own account when it adds a channel for it.</p>
      ) : a.connect ? (
        <p className="muted sm">Installed once for this machine. Then sign in from its Connect tab with “{a.connect.label}”, or type the keys below.</p>
      ) : a.auth.length > 0 ? (
        <p className="muted sm">Installed once for this machine. The keys below are this project's; other projects add theirs, or a key for every project, from the ability's Connect tab.</p>
      ) : a.instructions.length > 0 || a.transport === "instructions" ? (
        <p className="muted sm">Instructions Seemo reads before this kind of work. Nothing to connect.</p>
      ) : null}
      {!perProject && a.auth.length > 0 && (
        <div className="keys">
          {a.auth.map((k) => (
            <label key={k.env}>
              <span className="lb">
                {k.label || k.env}
                {k.optional && <small> optional</small>}
                {k.set && <small> · already set ({k.set === "project" ? "this project" : "the machine"})</small>}
              </span>
              <input className="input mono" type="password" autoComplete="off" spellCheck={false} placeholder={k.help && !/^https?:/.test(k.help) ? k.help : "paste it here"} value={secrets[k.env] ?? ""} onChange={(e) => setSecrets({ ...secrets, [k.env]: e.target.value })} />
              {k.help && /^https?:/.test(k.help) && (
                <a className="lnk sm" href={k.help} target="_blank" rel="noreferrer">
                  where to get it
                </a>
              )}
            </label>
          ))}
        </div>
      )}
      {a.runs_program && (
        <p className="warn sm">
          <Terminal size={12} /> This one runs a program on this computer{a.command ? <code> {a.command}</code> : null}.
        </p>
      )}
      {error && (
        <p className="err">
          <AlertTriangle size={13} /> {error}
        </p>
      )}
      <div className="acts">
        <button className="btn p sm" disabled={busy || !ok} onClick={() => onInstall(secrets)}>
          {busy ? <Loader2 size={13} className="spin" /> : <Download size={13} />} {a.repo ? "Download and install" : "Install"}
        </button>
        <button className="btn t sm" onClick={onCancel}>
          Cancel
        </button>
      </div>
    </motion.div>
  );
}

/** Folders, a git URL, what broke, Seemo's own guide. */
export function Advanced({ slug, broken, envFile, projectEnvFile, index, machineFolder, onDone, onGuide }: { slug: string; broken: { folder: string; scope: Scope; error: string }[]; envFile: string; projectEnvFile: string | null; index: string; machineFolder: string; onDone: () => void; onGuide: () => void }) {
  const [open, setOpen] = useState(false);
  const [source, setSource] = useState("");
  const [here, setHere] = useState(false);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);
  const install = async () => {
    setBusy(true);
    setMsg(null);
    try {
      const r = await api.installAbilitySource({ source: source.trim(), scope: here ? "project" : "global", project: here ? slug : undefined });
      setMsg({ ok: true, text: `Installed ${r?.name ?? source}.` });
      setSource("");
      onDone();
    } catch (e) {
      setMsg({ ok: false, text: (e as Error).message });
    } finally {
      setBusy(false);
    }
  };
  const projectFolder = `${machineFolder.replace(/\/abilities$/, "")}/projects/${slug}/abilities`;
  return (
    <div className="mk-adv">
      <button className="adv-toggle" onClick={() => setOpen((a) => !a)} aria-expanded={open}>
        {open ? <ChevronDown size={14} /> : <ChevronRight size={14} />} Advanced: folders, git, Seemo's guide
      </button>
      {open && (
        <div className="mk-adv-body">
          <p className="muted sm">
            An ability is a folder with a <code>SKILL.md</code> (instructions), an <code>extension.yaml</code> (an HTTP call, a program on this computer, or an MCP server), or both. Format in <code>docs/abilities.md</code>. Install one that is not in the index from a git URL or a folder on this machine.
          </p>
          <dl className="set-rows mono">
            <div>
              <dt>Every project</dt>
              <dd>
                <code>
                  <FolderOpen size={12} /> {machineFolder}
                </code>
              </dd>
            </div>
            <div>
              <dt>This project</dt>
              <dd>
                <code>
                  <FolderOpen size={12} /> {projectFolder}
                </code>
              </dd>
            </div>
          </dl>
          <div className="row">
            <input className="input mono" placeholder="https://github.com/you/kiqr-wordpress.git or /path/to/folder" value={source} onChange={(e) => setSource(e.target.value)} />
            <button className={`chip ${!here ? "acc" : ""}`} onClick={() => setHere(false)}>
              Every project
            </button>
            <button className={`chip ${here ? "acc" : ""}`} onClick={() => setHere(true)}>
              This project
            </button>
            <button className="btn g sm" disabled={busy || !source.trim()} onClick={() => void install()}>
              {busy ? <Loader2 size={13} className="spin" /> : <Download size={13} />} Install
            </button>
          </div>
          {msg && <p className={`sm ${msg.ok ? "ok" : "err"}`}>{msg.text}</p>}
          {broken.length > 0 && (
            <ul className="set-broken">
              {broken.map((b) => (
                <li key={`${b.scope.scope}-${b.folder}`}>
                  <code>{b.folder}</code> <span>{b.error}</span>
                </li>
              ))}
            </ul>
          )}
          <p className="muted sm">
            <button className="lnk" onClick={onGuide}>
              <BookOpen size={12} /> Seemo's own guide
            </button>{" "}
            is how it works inside KIQR: the tools, when to ask, how to report. Editable, with care.
          </p>
          <p className="muted sm">
            Index: <code>{index || "none"}</code>. Keys live in <code>{projectEnvFile ?? "the project's .env"}</code> for this project and <code>{envFile}</code> for the machine; values are never shown again.
          </p>
        </div>
      )}
    </div>
  );
}
