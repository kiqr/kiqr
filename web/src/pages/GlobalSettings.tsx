// Settings for the whole installation: models, crawl defaults, connections and storage.
// Project-level things (crawl overrides, where a project publishes) live in the project.
import { useEffect, useState } from "react";
import { Link, NavLink, useParams } from "react-router-dom";
import { AnimatePresence, motion } from "motion/react";
import {
  Bot, Check, Cloud, Cpu, Database, FolderGit2, HardDrive, KeyRound, Plug, Radar, Server, Settings as SettingsIcon, ShieldCheck, Sparkles, Terminal, X,
  type LucideIcon,
} from "lucide-react";
import { api, useAsync, type Config, type Health } from "../api";
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
  const models = h?.providers ?? [];
  const ready = models.filter((p) => p.available).length;
  const connected = (h?.unblockingapi ? 1 : 0) + (h?.secrets?.find((s) => s.name === "KIQR_CLOUD_TOKEN")?.set ? 1 : 0);
  const tiles: { icon: LucideIcon; label: string; value: string; note: string; to: string; tone: "ok" | "warn" | "" }[] = [
    { icon: Cpu, label: "Models", value: `${ready}/${models.length || 3}`, note: ready === models.length ? "Every task has a model it can use" : "A task has no model it can use", to: "/settings/models", tone: ready === models.length ? "ok" : "warn" },
    { icon: Radar, label: "Crawl default", value: config ? `${config.crawl.max_pages}` : "–", note: config ? `pages, ${config.crawl.concurrency} at a time` : "", to: "/settings/crawling", tone: "" },
    { icon: Plug, label: "Connections", value: `${connected}`, note: h?.unblockingapi ? "UnblockingAPI reads blocked pages" : "None yet", to: "/settings/connections", tone: connected ? "ok" : "" },
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
  return (
    <>
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

function Connections({ h }: { h?: Health }) {
  const secret = (n: string) => !!h?.secrets?.find((s) => s.name === n)?.set;
  const items: { icon: LucideIcon; name: string; text: string; state: "on" | "off" | "soon"; how?: React.ReactNode }[] = [
    {
      icon: ShieldCheck,
      name: "UnblockingAPI",
      text: "Reads pages that block crawlers or need JavaScript, with a real browser and residential proxies.",
      state: h?.unblockingapi ? "on" : "off",
      how: (
        <>
          Add <code>UNBLOCKINGAPI_KEY</code> to <code>.env</code> and restart the daemon.
        </>
      ),
    },
    {
      icon: KeyRound,
      name: "Anthropic API",
      text: "Claude models over the API, for anthropic/… in Models.",
      state: secret("ANTHROPIC_API_KEY") ? "on" : "off",
      how: (
        <>
          Add <code>ANTHROPIC_API_KEY</code> to <code>.env</code>.
        </>
      ),
    },
    {
      icon: KeyRound,
      name: "OpenAI API",
      text: "GPT models over the API, for openai/… in Models.",
      state: secret("OPENAI_API_KEY") ? "on" : "off",
      how: (
        <>
          Add <code>OPENAI_API_KEY</code> to <code>.env</code>.
        </>
      ),
    },
    { icon: Cloud, name: "kiqr.cloud", text: "One-click sign-in for X, Reddit, LinkedIn, Google Ads, Gmail and more, through the kiqr.cloud relay. Optional: the open-source client never needs it.", state: "soon" },
    { icon: Plug, name: "Your own MCP servers", text: "Add any MCP server and Seemo learns its tools. Credentials go in .env, for one project or all of them.", state: "soon" },
  ];
  return (
    <>
      <div className="set-conns">
        {items.map((c, i) => (
          <motion.div key={c.name} className={`set-conn st-${c.state}`} initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }} transition={{ delay: i * 0.05 }}>
            <span className="ic">
              <c.icon size={17} />
            </span>
            <div className="min0">
              <p className="nm">
                {c.name}
                {c.state === "on" && (
                  <span className="chip ok">
                    <b /> connected
                  </span>
                )}
                {c.state === "soon" && <span className="soon">Coming soon</span>}
              </p>
              <p className="ds">{c.text}</p>
              {c.state === "off" && c.how && <p className="how">{c.how}</p>}
            </div>
          </motion.div>
        ))}
      </div>
      <p className="set-note">
        Where each project publishes (its blog, X account, subreddit, ad account) is set per project under <b>Seemo → Connectors</b>. Secrets never reach the model.
      </p>
    </>
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
