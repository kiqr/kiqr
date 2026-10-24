// Settings for the whole installation: models, crawl defaults, connections and storage.
// Project-level things (crawl overrides, where a project publishes) live in the project.
import { useEffect, useState } from "react";
import { Link, NavLink, useParams } from "react-router-dom";
import { AnimatePresence, motion } from "motion/react";
import {
  Bot, Check, Cloud, Cpu, Database, ExternalLink, FolderGit2, HardDrive, Image as ImageIcon, KeyRound, Loader2, Plug, Radar, Server, Settings as SettingsIcon, ShieldCheck, Sparkles, Terminal, Trash2, X,
  type LucideIcon,
} from "lucide-react";
import { api, useAsync, useEvents, type Config, type Connection, type Health } from "../api";
import "../settings.css";

type Tab = { id: string; label: string; icon: LucideIcon };
const TABS: Tab[] = [
  { id: "", label: "General", icon: SettingsIcon },
  { id: "models", label: "Models", icon: Cpu },
  { id: "crawling", label: "Crawling", icon: Radar },
  { id: "connections", label: "Connections", icon: Plug },
  { id: "storage", label: "Storage", icon: Database },
];

export function Settings() {
  const { tab = "" } = useParams();
  const health = useAsync(() => api.health(), []);
  const config = useAsync(() => api.config(), []);
  const [draft, setDraft] = useState<Config | null>(null);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    if (config.data) setDraft(structuredClone(config.data));
  }, [config.data]);

  const dirty = !!draft && !!config.data && JSON.stringify(draft) !== JSON.stringify(config.data);
  // Seemo can change settings from a chat; show them here as soon as it does, unless you are mid-edit.
  useEvents(
    null,
    (e) => {
      if (e.type === "config_changed" && !dirty) {
        config.reload();
        health.reload();
      }
    },
    { demo: false },
  );
  const save = async () => {
    if (!draft) return;
    setSaving(true);
    setError(null);
    try {
      await api.saveConfig(draft);
      config.reload();
      health.reload();
      setSaved(true);
      setTimeout(() => setSaved(false), 2200);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setSaving(false);
    }
  };
  const h = health.data;
  const current = TABS.find((t) => t.id === tab) ?? TABS[0];

  return (
    <div className="set">
      <div className="set-head">
        <p className="cap">KIQR</p>
        <h1 className="d2">Settings</h1>
        <p className="muted">For every project on this machine. Saved to {h ? <code>{h.home}/config.yaml</code> : "config.yaml"}; secrets stay in <code>.env</code>.</p>
      </div>
      <div className="set-body">
        <nav className="set-nav" aria-label="Settings">
          {TABS.map((t) => (
            <NavLink key={t.id} to={t.id ? `/settings/${t.id}` : "/settings"} end>
              {({ isActive }) => (
                <>
                  {isActive && <motion.span layoutId="set-active" className="hl" transition={{ type: "spring", stiffness: 500, damping: 40 }} />}
                  <t.icon size={15} />
                  <span>{t.label}</span>
                </>
              )}
            </NavLink>
          ))}
        </nav>
        <motion.div key={current.id} className="set-main" initial={{ opacity: 0, y: 6 }} animate={{ opacity: 1, y: 0 }} transition={{ duration: 0.25 }}>
          {current.id === "" && <General h={h} config={config.data} />}
          {current.id === "models" && draft && <Models h={h} draft={draft} setDraft={setDraft} />}
          {current.id === "crawling" && draft && <Crawling h={h} draft={draft} setDraft={setDraft} />}
          {current.id === "connections" && <Connections h={h} />}
          {current.id === "storage" && <Storage h={h} />}
        </motion.div>
      </div>
      <AnimatePresence>
        {(dirty || saved) && (
          <motion.div className="set-save" initial={{ opacity: 0, y: 16 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: 16 }} transition={{ duration: 0.2 }}>
            {saved && !dirty ? (
              <span className="ok">
                <Check size={15} /> Saved and committed
              </span>
            ) : (
              <>
                <span>{error ?? "You have unsaved changes."}</span>
                <button className="btn t" onClick={() => config.data && setDraft(structuredClone(config.data))}>
                  Discard
                </button>
                <button className="btn p" disabled={saving} onClick={save}>
                  {saving ? "Saving…" : "Save changes"}
                </button>
              </>
            )}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

/* ---------- General ---------- */

function General({ h, config }: { h?: Health; config?: Config }) {
  const projects = useAsync(() => api.projects(), []);
  const ext = useAsync(() => api.extensions(), []);
  const models = h?.providers ?? [];
  const ready = models.filter((p) => p.available).length;
  const connected = (ext.data?.connections ?? []).filter((c) => c.ready).length + (h?.unblockingapi ? 1 : 0);
  const abilitiesOn = (ext.data?.abilities ?? []).filter((a) => a.active).length;
  const tiles: { icon: LucideIcon; label: string; value: string; note: string; to: string; tone: "ok" | "warn" | "" }[] = [
    { icon: Cpu, label: "Models", value: `${ready}/${models.length || 3}`, note: ready === models.length ? "Every task has a model it can use" : "A task has no model it can use", to: "/settings/models", tone: ready === models.length ? "ok" : "warn" },
    { icon: Radar, label: "Crawl default", value: config ? `${config.crawl.max_pages}` : "–", note: config ? `pages, ${config.crawl.concurrency} at a time` : "", to: "/settings/crawling", tone: "" },
    { icon: Plug, label: "Connections", value: `${connected}`, note: abilitiesOn ? `${abilitiesOn} of ${ext.data?.abilities.length ?? 4} abilities on` : "None yet", to: "/settings/connections", tone: connected ? "ok" : "" },
    { icon: FolderGit2, label: "Projects", value: `${projects.data?.length ?? "–"}`, note: "Each in its own folder, every change a commit", to: "/settings/storage", tone: "" },
  ];
  return (
    <>
      <div className="set-tiles">
        {tiles.map((t, i) => (
          <motion.div key={t.label} initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }} transition={{ delay: i * 0.05 }}>
            <Link to={t.to} className={`set-tile ${t.tone}`}>
              <span className="ic">
                <t.icon size={16} />
              </span>
              <span className="lb">{t.label}</span>
              <b>{t.value}</b>
              <small>{t.note}</small>
            </Link>
          </motion.div>
        ))}
      </div>
      <Card icon={Server} title="This installation" text="KIQR runs here, on your machine. Nothing leaves it except the pages it fetches and the model calls you configure.">
        <Rows
          rows={[
            ["Version", <>KIQR {h?.version}</>],
            ["Daemon", <code>http://{h?.address ?? "127.0.0.1:7777"}</code>],
            ["Home", <code>{h?.home}</code>],
            ["Secrets", <code>{h?.env_file ?? ".env"}</code>],
          ]}
        />
      </Card>
    </>
  );
}

/* ---------- Models ---------- */

const TASKS: { id: keyof Config["models"]; name: string; text: string; icon: LucideIcon }[] = [
  { id: "extract", name: "Reading pages", text: "Reads every crawled page and pulls out facts. Runs often: a fast, cheap model is best.", icon: Bot },
  { id: "synthesize", name: "Building the profile", text: "Turns the facts into the business profile, brand voice and content gaps.", icon: Sparkles },
  { id: "chat", name: "Seemo", text: "Talks with you, writes your assets and runs scheduled tasks.", icon: Terminal },
];
const PRESETS = [
  "claude-cli/haiku",
  "claude-cli/sonnet",
  "claude-cli/opus",
  "codex-cli/gpt-5",
  "anthropic/claude-haiku-5-5",
  "anthropic/claude-sonnet-5-5",
  "anthropic/claude-opus-5-5",
  "openai/gpt-5",
  "openai/gpt-5-mini",
];

/** Whether a provider/model string can run here, judged in the browser for unsaved edits. */
function usable(spec: string, h?: Health) {
  const [provider] = spec.split("/");
  const secret = (n: string) => !!h?.secrets?.find((s) => s.name === n)?.set;
  const tool = (n: string) => !!h?.tools?.find((t) => t.name === n)?.installed;
  if (provider === "anthropic") return secret("ANTHROPIC_API_KEY") ? "ok" : "Needs ANTHROPIC_API_KEY in .env";
  if (provider === "openai") return secret("OPENAI_API_KEY") ? "ok" : "Needs OPENAI_API_KEY in .env";
  if (provider === "claude-cli") return tool("claude") || secret("CLAUDE_CODE_OAUTH_TOKEN") ? "ok" : "Needs Claude Code installed";
  if (provider === "codex-cli") return tool("codex") ? "ok" : "Needs the Codex CLI installed";
  return "Use provider/model, like claude-cli/sonnet";
}

function Models({ h, draft, setDraft }: { h?: Health; draft: Config; setDraft: (c: Config) => void }) {
  const tools = h?.tools ?? [];
  const ext = useAsync(() => api.extensions(), []);
  useEvents(null, (e) => ["secret_set", "extension_installed", "extension_removed", "skill_saved", "skill_deleted"].includes(e.type) && ext.reload(), { demo: false });
  const abilities = ext.data?.abilities ?? [];
  const pick = (id: string, conn: string) => setDraft({ ...draft, abilities: { ...(draft.abilities ?? {}), [id]: conn } });
  return (
    <>
      <Card icon={ImageIcon} title="Abilities" text="What Seemo can do beyond writing, and which connection does it. The built-in renderer makes images with nothing installed; connect a generator for photos and illustrations.">
        <div className="set-models">
          {abilities.map((a) => {
            const chosen = draft.abilities?.[a.id] ?? a.configured ?? a.active?.id ?? "";
            const ready = a.connections.filter((c) => c.ready);
            return (
              <div key={a.id} className="set-model">
                <span className="ic">
                  <Sparkles size={15} />
                </span>
                <div className="min0">
                  <p className="nm">{a.label}</p>
                  <p className="ds">{a.active ? `Through ${a.active.name}.` : a.connections.length ? "A connection exists but is not set up yet." : "No connection provides this yet."}</p>
                </div>
                <div className="pick">
                  {ready.length > 1 ? (
                    <select className="input" value={chosen} onChange={(e) => pick(a.id, e.target.value)}>
                      {a.connections.map((c) => (
                        <option key={c.id} value={c.id} disabled={!c.ready}>
                          {c.name}
                          {c.ready ? "" : " (needs a key)"}
                        </option>
                      ))}
                    </select>
                  ) : a.active ? (
                    <span className="chip ok">
                      <b /> {a.active.name}
                    </span>
                  ) : (
                    <Link to="/settings/connections" className="chip warn">
                      <b /> off: add a connection
                    </Link>
                  )}
                </div>
              </div>
            );
          })}
        </div>
      </Card>
      <Card icon={Cpu} title="Which model does what" text="Each task names a provider and a model. Your Claude or ChatGPT subscription works through its command-line tool; API keys work too.">
        <div className="set-models">
          {TASKS.map((t) => {
            const value = draft.models[t.id];
            const state = usable(value, h);
            return (
              <div key={t.id} className="set-model">
                <span className="ic">
                  <t.icon size={15} />
                </span>
                <div className="min0">
                  <p className="nm">{t.name}</p>
                  <p className="ds">{t.text}</p>
                </div>
                <div className="pick">
                  <input className="input mono" list="kiqr-models" value={value} spellCheck={false} onChange={(e) => setDraft({ ...draft, models: { ...draft.models, [t.id]: e.target.value.trim() } })} />
                  <span className={`chip ${state === "ok" ? "ok" : "warn"}`}>
                    <b />
                    {state === "ok" ? "ready" : state}
                  </span>
                </div>
              </div>
            );
          })}
          <datalist id="kiqr-models">
            {PRESETS.map((p) => (
              <option key={p} value={p} />
            ))}
          </datalist>
        </div>
      </Card>
      <Card icon={Terminal} title="On this machine" text="What KIQR found. Command-line tools use your existing subscription; nothing to configure.">
        <Rows
          rows={[
            ...tools.map((t): [string, React.ReactNode] => [t.name === "claude" ? "Claude Code" : "Codex CLI", <Status ok={t.installed} yes="installed" no="not found" />]),
            ...(h?.secrets ?? [])
              .filter((s) => ["ANTHROPIC_API_KEY", "OPENAI_API_KEY", "CLAUDE_CODE_OAUTH_TOKEN"].includes(s.name))
              .map((s): [string, React.ReactNode] => [s.name, <Status ok={s.set} yes="set" no="not set" />]),
          ]}
          mono
        />
      </Card>
    </>
  );
}

/* ---------- Crawling ---------- */

function Crawling({ h, draft, setDraft }: { h?: Health; draft: Config; setDraft: (c: Config) => void }) {
  const set = (patch: Partial<Config["crawl"]>) => setDraft({ ...draft, crawl: { ...draft.crawl, ...patch } });
  const num = (v: string, min: number, max: number) => Math.max(min, Math.min(max, Math.round(Number(v) || min)));
  return (
    <>
      <Card icon={Radar} title="Crawl defaults" text="Used by every project unless its own settings say otherwise. Changes apply from the next crawl.">
        <div className="set-fields">
          <Field label="Pages per crawl" hint="How many pages Seemo reads. Raise it live from the crawl screen." value={draft.crawl.max_pages} min={1} max={5000} onChange={(v) => set({ max_pages: num(v, 1, 5000) })} />
          <Field label="Pages fetched at a time" hint="1–10. KIQR fetches pages itself, never the model." value={draft.crawl.concurrency} min={1} max={10} onChange={(v) => set({ concurrency: num(v, 1, 10) })} />
          <Field label="Pages read at a time" hint="1–10. Reading is queued and never slows fetching." value={draft.crawl.ai_concurrency} min={1} max={10} onChange={(v) => set({ ai_concurrency: num(v, 1, 10) })} />
        </div>
      </Card>
      <Card icon={ShieldCheck} title="Pages that block crawlers" text="Some sites answer with a challenge page, a block or an empty JavaScript shell.">
        <div className="set-toggle-row">
          <div>
            <p className="nm">Read them with UnblockingAPI</p>
            <p className="ds">{h?.unblockingapi ? "A real browser and residential proxies, only for pages KIQR couldn't read. Each success costs one credit." : "Add UNBLOCKINGAPI_KEY to .env to use it. Until then such pages are listed as unreadable."}</p>
          </div>
          <Switch on={draft.crawl.escalate_to_unblockingapi} onChange={(on) => set({ escalate_to_unblockingapi: on })} disabled={!h?.unblockingapi} />
        </div>
      </Card>
    </>
  );
}

function Field({ label, hint, value, min, max, onChange }: { label: string; hint: string; value: number; min: number; max: number; onChange: (v: string) => void }) {
  return (
    <label className="set-field">
      <span className="nm">{label}</span>
      <input className="input mono" type="number" min={min} max={max} value={value} onChange={(e) => onChange(e.target.value)} />
      <span className="ds">{hint}</span>
    </label>
  );
}

function Switch({ on, onChange, disabled }: { on: boolean; onChange: (on: boolean) => void; disabled?: boolean }) {
  return (
    <button type="button" role="switch" aria-checked={on} disabled={disabled} className={`switch ${on ? "on" : ""}`} onClick={() => onChange(!on)}>
      <motion.i layout transition={{ type: "spring", stiffness: 600, damping: 35 }} />
    </button>
  );
}

/* ---------- Connections ---------- */

const RELOAD_ON = ["secret_set", "extension_installed", "extension_removed", "skill_saved", "skill_deleted"];

function Connections({ h }: { h?: Health }) {
  const ext = useAsync(() => api.extensions(), []);
  useEvents(null, (e) => RELOAD_ON.includes(e.type) && ext.reload(), { demo: false });
  const conns = ext.data?.connections ?? [];
  const secret = (n: string) => !!h?.secrets?.find((s) => s.name === n)?.set;
  const providers: { name: string; text: string; env: string; help: string; set: boolean }[] = [
    { name: "UnblockingAPI", text: "Reads pages that block crawlers or need JavaScript, for the crawl and for Seemo's read_url.", env: "UNBLOCKINGAPI_KEY", help: "https://unblockingapi.com", set: !!h?.unblockingapi },
    { name: "Anthropic API", text: "Claude models over the API, for anthropic/… in Models.", env: "ANTHROPIC_API_KEY", help: "https://console.anthropic.com/settings/keys", set: secret("ANTHROPIC_API_KEY") },
    { name: "OpenAI API", text: "GPT models over the API, for openai/… in Models (and OpenAI images).", env: "OPENAI_API_KEY", help: "https://platform.openai.com/api-keys", set: secret("OPENAI_API_KEY") },
  ];
  return (
    <>
      <div className="set-conns">
        {conns.map((c, i) => (
          <ConnectionCard key={c.id} c={c} i={i} onChange={ext.reload} />
        ))}
        <motion.div className="set-conn st-soon" initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }} transition={{ delay: conns.length * 0.05 }}>
          <span className="ic">
            <Cloud size={17} />
          </span>
          <div className="min0">
            <p className="nm">
              kiqr.cloud <span className="soon">Coming soon</span>
            </p>
            <p className="ds">One-click sign-in for Canva, Gmail, X, Reddit, LinkedIn and more, through the kiqr.cloud relay. Optional: the open-source client never needs it.</p>
          </div>
        </motion.div>
      </div>
      <Card icon={KeyRound} title="Model and crawl providers" text="Keys for the models in Models and for reading blocked pages. Saved to .env; never shown again, never sent to the model.">
        <div className="set-models">
          {providers.map((p) => (
            <div key={p.env} className="set-model">
              <span className="ic">
                <KeyRound size={15} />
              </span>
              <div className="min0">
                <p className="nm">
                  {p.name}
                  {p.set && (
                    <span className="chip ok" style={{ marginLeft: 8 }}>
                      <b /> set
                    </span>
                  )}
                </p>
                <p className="ds">{p.text}</p>
              </div>
              <div className="pick">
                <SecretField name={p.env} label={p.env} help={p.help} set={p.set} />
              </div>
            </div>
          ))}
        </div>
      </Card>
      <Card icon={Plug} title="Your own connections" text="Any folder with an extension.yaml: an HTTP call, a program on this computer, or an MCP server. Install it under Seemo → Skills (Advanced) in a project, and it shows up here.">
        {ext.data && ext.data.broken.length > 0 && (
          <ul className="set-broken">
            {ext.data.broken.map((b) => (
              <li key={b.folder}>
                <code>{b.folder}</code> <span>{b.error}</span>
              </li>
            ))}
          </ul>
        )}
        <p className="set-note">Values live in <code>{h?.env_file ?? ".env"}</code>. A connection that runs a program says so on its card and is never installed without you.</p>
      </Card>
    </>
  );
}

/** One connection: what it provides, what it needs, and the fields to give it. */
function ConnectionCard({ c, i, onChange }: { c: Connection; i: number; onChange: () => void }) {
  const [confirm, setConfirm] = useState(false);
  const [busy, setBusy] = useState(false);
  const state: "on" | "off" | "warn" = c.ready ? "on" : c.transport === "mcp" && c.program_found === false ? "warn" : "off";
  const remove = async () => {
    setBusy(true);
    try {
      await api.removeConnection(c.id, c.scope.scope === "project" ? c.scope.project : undefined);
      onChange();
    } finally {
      setBusy(false);
      setConfirm(false);
    }
  };
  const provides = c.provides.map((p) => p.label).filter(Boolean);
  const cost = c.provides.find((p) => p.cost)?.cost ?? c.tools.find((t) => t.cost)?.cost ?? null;
  return (
    <motion.div className={`set-conn st-${state}`} initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }} transition={{ delay: i * 0.05 }}>
      <span className="ic">{c.transport === "builtin" ? <Sparkles size={17} /> : c.transport === "mcp" || c.transport === "command" ? <Terminal size={17} /> : <Plug size={17} />}</span>
      <div className="min0 grow">
        <p className="nm">
          {c.name}
          {c.ready ? (
            <span className="chip ok">
              <b /> connected
            </span>
          ) : state === "warn" ? (
            <span className="chip warn">
              <b /> program not found
            </span>
          ) : (
            <span className="chip warn">
              <b /> needs a key
            </span>
          )}
          {c.scope.scope === "project" && <span className="chip">{c.scope.project}</span>}
        </p>
        <p className="ds">
          {c.summary}
          {c.homepage && (
            <>
              {" "}
              <a href={c.homepage} target="_blank" rel="noreferrer" className="lnk">
                {new URL(c.homepage).hostname} <ExternalLink size={11} />
              </a>
            </>
          )}
        </p>
        {(provides.length > 0 || c.tools.length > 0 || c.channels.length > 0) && (
          <p className="how">
            {provides.length > 0 && <>Provides: {provides.join(", ")}. </>}
            {c.channels.length > 0 && <>Publishes to: {c.channels.map((ch) => ch.label).join(", ")}. </>}
            {c.tools.length > 0 && <>{c.tools.length} tool{c.tools.length === 1 ? "" : "s"} of its own. </>}
            {cost && <>Cost: {cost}.</>}
          </p>
        )}
        {(c.transport === "mcp" || c.transport === "command") && (
          <p className="how warn">
            Runs a program on this computer{c.command ? <>: <code>{c.command}</code></> : null}.
          </p>
        )}
        {c.auth.length > 0 && (
          <div className="set-keys">
            {c.auth.map((a) => (
              <SecretField key={a.env} name={a.env} label={a.label || a.env} help={a.help} set={a.set} optional={a.optional} />
            ))}
          </div>
        )}
        {!c.builtin && (
          <div className="set-conn-acts">
            {confirm ? (
              <>
                <span className="muted sm">Remove this connection and its skill?</span>
                <button className="btn t bad" disabled={busy} onClick={() => void remove()}>
                  {busy ? <Loader2 size={14} className="spin" /> : <Check size={14} />} Remove
                </button>
                <button className="btn t" onClick={() => setConfirm(false)}>
                  Keep
                </button>
              </>
            ) : (
              <button className="btn t sm" onClick={() => setConfirm(true)}>
                <Trash2 size={13} /> Remove
              </button>
            )}
          </div>
        )}
      </div>
    </motion.div>
  );
}

/** A help line: a bare URL becomes "where to get it"; a sentence keeps its words, with any URL in it linked. */
function Hint({ text }: { text: string }) {
  if (/^https?:\/\/\S+$/.test(text.trim()))
    return (
      <a href={text.trim()} target="_blank" rel="noreferrer" className="lnk">
        where to get it <ExternalLink size={10} />
      </a>
    );
  const parts = text.split(/(https?:\/\/\S+)/);
  return (
    <span className="hint">
      {parts.map((p, i) =>
        /^https?:\/\//.test(p) ? (
          <a key={i} href={p} target="_blank" rel="noreferrer" className="lnk">
            {p.replace(/^https?:\/\//, "")} <ExternalLink size={10} />
          </a>
        ) : (
          <span key={i}>{p}</span>
        ),
      )}
    </span>
  );
}

/** One secret by name: typed once, saved to .env, never shown again. */
function SecretField({ name, label, help, set, optional }: { name: string; label: string; help?: string; set: boolean; optional?: boolean }) {
  const [value, setValue] = useState("");
  const [busy, setBusy] = useState(false);
  const [done, setDone] = useState(set);
  const [error, setError] = useState<string | null>(null);
  const [editing, setEditing] = useState(!set);
  useEffect(() => {
    setDone(set);
    setEditing(!set);
  }, [set]);
  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      const r = await api.setSecret(name, value.trim());
      setDone(r.set);
      setEditing(!r.set);
      setValue("");
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="set-key">
      <span className="lb">
        {label}
        {optional && <small> optional</small>}
        {help && <Hint text={help} />}
      </span>
      {editing ? (
        <span className="in">
          <input className="input mono" type="password" autoComplete="off" spellCheck={false} placeholder={done ? "new value" : "paste it here"} value={value} onChange={(e) => setValue(e.target.value)} onKeyDown={(e) => e.key === "Enter" && value.trim() && void save()} />
          <button className="btn g sm" disabled={busy || !value.trim()} onClick={() => void save()}>
            {busy ? <Loader2 size={13} className="spin" /> : <Check size={13} />} Save
          </button>
          {done && (
            <button className="btn t sm" onClick={() => setEditing(false)}>
              Cancel
            </button>
          )}
        </span>
      ) : (
        <span className="in">
          <span className="chip ok">
            <b /> set
          </span>
          <button className="btn t sm" onClick={() => setEditing(true)}>
            Change
          </button>
        </span>
      )}
      {error && <span className="err">{error}</span>}
    </div>
  );
}

/* ---------- Storage ---------- */

function Storage({ h }: { h?: Health }) {
  const tracked = ["config.yaml", "Business profiles and briefs", "Facts from every page", "Asset types", "Skills and Seemo's persona", "Tasks and their runs"];
  const ignored = [".env (secrets)", "Crawl cache", "Generated assets: articles, images, videos…"];
  return (
    <>
      <Card icon={HardDrive} title="Plain files, on this machine" text="No database. Everything is YAML and Markdown in one folder you can read, back up or move.">
        <Rows rows={[["Home", <code>{h?.home}</code>]]} />
      </Card>
      <Card icon={FolderGit2} title="History" text="The home folder is a local git repository. Every change is a commit with a readable message, so anything can be undone. KIQR never pushes it anywhere.">
        <div className="set-git">
          <div>
            <p className="cap">In history</p>
            <ul>
              {tracked.map((t) => (
                <li key={t}>
                  <Check size={13} /> {t}
                </li>
              ))}
            </ul>
          </div>
          <div>
            <p className="cap">Kept out of history</p>
            <ul className="out">
              {ignored.map((t) => (
                <li key={t}>
                  <X size={13} /> {t}
                </li>
              ))}
            </ul>
          </div>
        </div>
      </Card>
      <Card icon={Cloud} title="Backups" text="Generated assets aren't in git, so they need a backup of their own." soon>
        <p className="muted sm">S3-compatible backups (AWS, Cloudflare R2, Backblaze, MinIO) for assets and the whole home folder.</p>
      </Card>
    </>
  );
}

/* ---------- pieces ---------- */

function Card({ icon: Icon, title, text, soon, children }: { icon: LucideIcon; title: string; text: string; soon?: boolean; children: React.ReactNode }) {
  return (
    <section className="pcard set-card">
      <div className="set-card-head">
        <span className="ic">
          <Icon size={16} />
        </span>
        <div>
          <h2>
            {title}
            {soon && <span className="soon">Coming soon</span>}
          </h2>
          <p>{text}</p>
        </div>
      </div>
      {children}
    </section>
  );
}

function Rows({ rows, mono }: { rows: [string, React.ReactNode][]; mono?: boolean }) {
  return (
    <dl className={`set-rows ${mono ? "mono" : ""}`}>
      {rows.map(([k, v]) => (
        <div key={k}>
          <dt>{k}</dt>
          <dd>{v}</dd>
        </div>
      ))}
    </dl>
  );
}

function Status({ ok, yes, no }: { ok: boolean; yes: string; no: string }) {
  return (
    <span className={`chip ${ok ? "ok" : ""}`}>
      <b /> {ok ? yes : no}
    </span>
  );
}
