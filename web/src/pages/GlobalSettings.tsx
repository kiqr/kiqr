// Settings for the whole installation: models, crawl defaults, the abilities marketplace and storage.
// Project-level things (crawl overrides, where a project publishes) live in the project.
import { useEffect, useState } from "react";
import { Link, NavLink, useParams } from "react-router-dom";
import { AnimatePresence, motion } from "motion/react";
import { ArrowUpCircle, Bot, Check, Cloud, Cpu, Database, ExternalLink, FolderGit2, HardDrive, KeyRound, Loader2, Network, Radar, Server, Settings as SettingsIcon, Sparkles, Terminal, X, type LucideIcon } from "lucide-react";
import { api, useAsync, useEvents, type Config, type Health } from "../api";
import { SecretField } from "./SecretField";
import { Switch } from "./Switch";
import { SignInById } from "./SignIns";
import { FactoryReset } from "./FactoryReset";
import "../settings.css";

type Tab = { id: string; label: string; icon: LucideIcon };
const TABS: Tab[] = [
  { id: "", label: "General", icon: SettingsIcon },
  { id: "models", label: "Models", icon: Cpu },
  { id: "crawling", label: "Crawling", icon: Radar },
  { id: "network", label: "Network", icon: Network },
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
      // A new address restarts the daemon; the shell follows it there (config_changed with restart).
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
          {current.id === "network" && draft && <NetworkTab h={h} draft={draft} setDraft={setDraft} />}
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
  const mk = useAsync(() => api.abilities(), []);
  const models = h?.providers ?? [];
  const ready = models.filter((p) => p.available).length;
  const installed = (mk.data?.abilities ?? []).length;
  const abilitiesOn = (mk.data?.contracts ?? []).filter((c) => c.active).length;
  const tiles: { icon: LucideIcon; label: string; value: string; note: string; to: string; tone: "ok" | "warn" | "" }[] = [
    { icon: Cpu, label: "Models", value: `${ready}/${models.length || 3}`, note: ready === models.length ? "Every task has a model it can use" : "A task has no model it can use", to: "/settings/models", tone: ready === models.length ? "ok" : "warn" },
    { icon: Radar, label: "Crawl default", value: config ? `${config.crawl.max_pages}` : "–", note: config ? `pages, ${config.crawl.concurrency} at a time` : "", to: "/settings/crawling", tone: "" },
    { icon: Sparkles, label: "Abilities", value: `${installed}`, note: abilitiesOn ? `on this machine; set up per project under Setup → Abilities` : "on this machine; nothing beyond writing and images yet", to: projects.data?.[0] ? `/p/${projects.data[0].slug}/abilities` : "/projects", tone: abilitiesOn ? "ok" : "" },
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
      <Card icon={Cloud} title="kiqr.cloud" text="Connects this KIQR agent with your third-party apps: WordPress, UnblockingAPI and more, signed in with one click instead of keys you copy. Optional; it keeps your account and nothing else.">
        <SignInById id="kiqr-cloud" />
      </Card>
      <Card icon={Server} title="This installation" text="KIQR runs here, on your machine. Nothing leaves it except the pages it fetches, the model calls you configure and an anonymous usage ping you can switch off.">
        <Rows
          rows={[
            ["Version", <Version h={h} />],
            ["Updates", <AutoUpdate />],
            ["Usage stats", <UsagePing />],
            ["Daemon", <code>http://{h?.address ?? "127.0.0.1:4040"}</code>],
            ["Home", <code>{h?.home}</code>],
            ["Secrets", <code>{h?.env_file ?? ".env"}</code>],
          ]}
        />
      </Card>
    </>
  );
}

/** Whether new versions install on their own. Saved straight away, like a switch elsewhere. */
function AutoUpdate() {
  const config = useAsync(() => api.config(), []);
  const [busy, setBusy] = useState(false);
  const on = !!config.data?.updates?.auto;
  const toggle = async (next: boolean) => {
    if (!config.data) return;
    setBusy(true);
    try {
      await api.saveConfig({ ...config.data, updates: { auto: next } });
      config.reload();
    } finally {
      setBusy(false);
    }
  };
  return (
    <label className="set-auto">
      <Switch on={on} disabled={busy || !config.data} onChange={toggle} />
      <span>{on ? "Installed on their own, when Seemo is not working" : "KIQR says when one is out; you install it"}</span>
    </label>
  );
}

/** The anonymous usage ping: a switch, and exactly what it sends. */
function UsagePing() {
  const t = useAsync(() => api.telemetry(), []);
  const [busy, setBusy] = useState(false);
  const [open, setOpen] = useState(false);
  const d = t.data;
  const env = d?.off_by && d.off_by !== "config" ? d.off_by : null;
  const toggle = async (next: boolean) => {
    setBusy(true);
    try {
      await api.setTelemetry(next);
      t.reload();
    } finally {
      setBusy(false);
    }
  };
  return (
    <span className="set-ping">
      <label className="set-auto">
        <Switch on={!!d?.enabled} disabled={busy || !d || !!env} onChange={toggle} />
        <span>
          {env === "no-cloud" ? "Off: no kiqr.cloud address is set" : env ? `Off: ${env} is set` : d?.enabled ? `An anonymous ping to kiqr.cloud every ${d.every_minutes} minutes` : "Off"}
        </span>
      </label>
      {d && (
        <button className="btn t sm" onClick={() => setOpen(!open)}>
          {open ? "Hide" : "What's sent"}
        </button>
      )}
      {open && d && (
        <pre className="set-ping-body">
          {JSON.stringify(d.payload, null, 2)}
        </pre>
      )}
    </span>
  );
}

/** The running version, and the newer one with an Update button when the daemon found it. */
function Version({ h }: { h?: Health }) {
  const status = useAsync(() => api.updateStatus(), []);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEvents(null, (e) => {
    if (e.type === "update_available") status.reload();
    if (e.type === "update_failed") setError(String(e.error ?? "The update failed."));
  });
  const u = status.data;
  const update = async () => {
    if (!u?.latest) return;
    setBusy(true);
    setError(null);
    try {
      await api.applyUpdate();
      // The daemon restarts as the new version; reload once it answers as that version.
      for (let i = 0; i < 90; i++) {
        await new Promise((r) => setTimeout(r, 1000));
        const v = await api.health().then((x) => x.version).catch(() => null);
        if (v === u.latest) return window.location.reload();
      }
      setError("The new version is installed but the daemon has not come back yet. Check that it is running.");
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
    setBusy(false);
  };
  return (
    <span className="set-version">
      <span>KIQR {h?.version}</span>
      {u?.available && u.latest && (
        <>
          <span className="warn">v{u.latest} available</span>
          {u.notes_url && (
            <a href={u.notes_url} target="_blank" rel="noreferrer" className="muted">
              What's new <ExternalLink size={11} />
            </a>
          )}
          {u.can_self_update ? (
            <button className="btn p sm" disabled={busy} onClick={update}>
              {busy ? <Loader2 size={13} className="spin" /> : <ArrowUpCircle size={13} />} {busy ? "Updating…" : `Update to ${u.latest}`}
            </button>
          ) : (
            <small className="muted">{u.reason}</small>
          )}
        </>
      )}
      {error && <small className="bad">{error}</small>}
    </span>
  );
}

/* ---------- Models ---------- */

const TASKS: { id: keyof Config["models"]; name: string; text: string; icon: LucideIcon }[] = [
  { id: "extract", name: "Reading pages", text: "Reads every crawled page and pulls out facts. Runs often: a fast, cheap model is best.", icon: Bot },
  { id: "synthesize", name: "Building the profile", text: "Turns the facts into the business profile, brand voice and content gaps.", icon: Sparkles },
  { id: "chat", name: "Seemo", text: "Talks with you, writes your assets and runs scheduled tasks.", icon: Terminal },
];
/** A model: one of the known ones from a list, or any other `provider/model` typed after "Other model…". */
function ModelPick({ value, onChange }: { value: string; onChange: (v: string) => void }) {
  const [other, setOther] = useState(!!value && !PRESETS.includes(value));
  return (
    <span className="model-pick">
      <select className="input mono" value={other ? "__other" : value} onChange={(e) => (e.target.value === "__other" ? setOther(true) : (setOther(false), onChange(e.target.value)))} aria-label="Model">
        {!value && (
          <option value="" disabled>
            Choose…
          </option>
        )}
        {PRESETS.map((p) => (
          <option key={p} value={p}>
            {p}
          </option>
        ))}
        <option value="__other">Other model…</option>
      </select>
      {other && <input className="input mono" value={value} spellCheck={false} placeholder="provider/model" onChange={(e) => onChange(e.target.value.trim())} aria-label="Other model" />}
    </span>
  );
}

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
  return (
    <>
      <Card icon={KeyRound} title="Model keys" text="For models over the API. Your Claude or ChatGPT subscription needs none: it signs in through the command-line tool. Saved to .env, never shown again, never sent to the model.">
        <div className="set-models">
          {[
            { name: "Anthropic API", text: "Claude models over the API, for anthropic/… below.", env: "ANTHROPIC_API_KEY", help: "https://console.anthropic.com/settings/keys" },
            { name: "OpenAI API", text: "GPT models over the API, for openai/… below (and OpenAI images).", env: "OPENAI_API_KEY", help: "https://platform.openai.com/api-keys" },
          ].map((p) => (
            <div key={p.env} className="set-model">
              <span className="ic">
                <KeyRound size={15} />
              </span>
              <div className="min0">
                <p className="nm">{p.name}</p>
                <p className="ds">{p.text}</p>
              </div>
              <div className="pick">
                <SecretField name={p.env} label={p.env} help={p.help} set={!!h?.secrets?.find((x) => x.name === p.env)?.set} />
              </div>
            </div>
          ))}
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
                  <ModelPick value={value} onChange={(v) => setDraft({ ...draft, models: { ...draft.models, [t.id]: v } })} />
                  <span className={`chip ${state === "ok" ? "ok" : "warn"}`}>
                    <b />
                    {state === "ok" ? "ready" : state}
                  </span>
                </div>
              </div>
            );
          })}
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

function Crawling({ draft, setDraft }: { h?: Health; draft: Config; setDraft: (c: Config) => void }) {
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
    </>
  );
}

/* ---------- Network ---------- */

const HOSTS: { value: string; label: string; hint: string }[] = [
  { value: "127.0.0.1", label: "This computer only", hint: "Only a browser on this computer can open KIQR. The safe choice." },
  { value: "0.0.0.0", label: "Every device on your network", hint: "Phones and other computers on your network can open KIQR too. It has no password, so use this only on a network you trust." },
];

/** Where the browser finds the daemon after a move: this page's host name when it listens everywhere. */
export function addressUrl(d: Config["daemon"]) {
  const host = d.host === "0.0.0.0" || d.host === "::" ? location.hostname : d.host;
  return `${location.protocol}//${host}:${d.port}${location.pathname}`;
}

function NetworkTab({ h, draft, setDraft }: { h?: Health; draft: Config; setDraft: (c: Config) => void }) {
  const set = (patch: Partial<Config["daemon"]>) => setDraft({ ...draft, daemon: { ...draft.daemon, ...patch } });
  const locked = h?.address_env ?? [];
  const host = HOSTS.find((x) => x.value === draft.daemon.host);
  return (
    <Card icon={Network} title="Address" text="Where KIQR's web page and API listen. Saving a new address restarts KIQR there (after Seemo finishes what it is doing) and this page moves with it.">
      {locked.length > 0 ? (
        <p className="set-note">
          Set by {locked.map((k) => <code key={k}>{k}</code>)} in the environment, which overrides config.yaml. KIQR listens on <code>{h?.address}</code>.
        </p>
      ) : (
        <div className="set-fields">
          <label className="set-field">
            <span className="nm">Who can open KIQR</span>
            <select className="input" value={host?.value ?? draft.daemon.host} onChange={(e) => set({ host: e.target.value })}>
              {HOSTS.map((x) => (
                <option key={x.value} value={x.value}>
                  {x.label}
                </option>
              ))}
              {!host && <option value={draft.daemon.host}>{draft.daemon.host}</option>}
            </select>
            <span className="ds">{host?.hint ?? "A custom address from config.yaml."}</span>
          </label>
          <Field label="Port" hint="1024–65535. KIQR checks it is free before saving." value={draft.daemon.port} min={1024} max={65535} onChange={(v) => set({ port: Math.round(Number(v) || 0) })} />
        </div>
      )}
    </Card>
  );
}

/** While the daemon restarts on its new address: wait until it answers there, then go. `pid` is the old daemon's. */
export function Moving({ to, pid }: { to: string; pid?: number }) {
  const [late, setLate] = useState(false);
  useEffect(() => {
    let done = false;
    const origin = new URL(to).origin;
    const tick = async () => {
      while (!done) {
        await new Promise((r) => setTimeout(r, 1000));
        // Same origin (only the host changed): the old daemon still answers until it restarts, so wait for a new one.
        // Another origin: an opaque answer is enough to know it is up.
        const up =
          origin === location.origin
            ? await api.health().then((x) => x.pid !== pid, () => false)
            : await fetch(`${origin}/api/health`, { mode: "no-cors", cache: "no-store" }).then(() => true, () => false);
        if (up && !done) return void (window.location.href = to);
      }
    };
    tick();
    const t = setTimeout(() => setLate(true), 20000);
    return () => {
      done = true;
      clearTimeout(t);
    };
  }, [to]);
  return (
    <motion.div className="set-moving" initial={{ opacity: 0 }} animate={{ opacity: 1 }}>
      <div className="pcard">
        <Loader2 size={18} className="spin" />
        <div>
          <h2>KIQR is moving to {new URL(to).host}</h2>
          <p className="muted">It restarts there as soon as Seemo has finished what it is doing, and this page follows.</p>
          {late && (
            <p className="muted">
              Taking a while. Check <code>kiqr status</code>, or open <a href={to}>{new URL(to).host}</a> yourself.
            </p>
          )}
        </div>
      </div>
    </motion.div>
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


/* ---------- Connections ---------- */

/* ---------- Storage ---------- */

function Storage({ h }: { h?: Health }) {
  const tracked = ["config.yaml", "Business profiles and briefs", "Facts from every page", "Asset types", "Abilities and Seemo's persona", "Tasks and their runs"];
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
      <FactoryReset home={h?.home} boot={h?.boot} />
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
