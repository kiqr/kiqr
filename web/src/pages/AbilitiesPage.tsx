// Setup → Abilities: the one page for everything Seemo can do in this
// project. Core abilities (code, switch off but never leave), what is
// installed from the marketplace, what you taught it; each opens in its own
// modal. "Teach Seemo something" adds one in plain words. The marketplace
// sits below. Installing is for the machine; switches, keys and copies of
// the instructions are per project.
import { useCallback, useEffect, useMemo, useState } from "react";
import { useLocation, useNavigate, useParams } from "react-router-dom";
import { AnimatePresence } from "motion/react";
import { AlertTriangle, BookOpen, KeyRound, Loader2, Plug, Plus, RefreshCw, Send, Sparkles, Terminal } from "lucide-react";
import { api, useAsync, useEvents, type Ability } from "../api";
import { Switch } from "./Switch";
import { AbilityModal } from "./AbilityModal";
import { Advanced, Marketplace } from "./Marketplace";
import { TeachForm, type TeachDraft } from "./TeachForm";
import { AbilityIcon, Author, isOn, RELOAD_ON, stateOf } from "./abilityBits";
import "../profile.css";
import "../settings.css";
import "../chat.css";

export function AbilitiesPage() {
  const { slug = "" } = useParams();
  const location = useLocation();
  const navigate = useNavigate();
  const data = useAsync(() => api.abilities(slug), [slug]);
  useEvents(null, (e) => RELOAD_ON.includes(e.type) && data.reload(), { demo: false });
  const [draft, setDraft] = useState<TeachDraft | null>(null);
  const [openId, setOpenId] = useState<string | null>(() => /#ability=([^&]+)/.exec(location.hash)?.[1] ?? null);
  const [openTab, setOpenTab] = useState<"overview" | "instructions" | "connect" | undefined>(undefined);
  const [error, setError] = useState<string | null>(null);

  const abilities = data.data?.abilities ?? [];
  const market = data.data?.market ?? [];
  const contracts = data.data?.contracts ?? [];
  const guide = data.data?.guide ?? null;
  const yours = useMemo(() => abilities.filter((a) => a.source === "yours" || a.source === "local").sort(byName), [abilities]);
  const fromMarket = useMemo(() => abilities.filter((a) => !a.core && (a.source === "marketplace" || a.source === "shipped")).sort(byName), [abilities]);
  const builtin = useMemo(() => abilities.filter((a) => a.core), [abilities]);
  const on = abilities.filter(isOn).length;
  const opened = useMemo(() => (openId ? [...abilities, ...market, ...(guide ? [guide] : [])].find((a) => a.id === openId) ?? null : null), [openId, abilities, market, guide]);

  const open = useCallback(
    (a: Ability, tab?: "overview" | "instructions" | "connect") => {
      setOpenId(a.id);
      setOpenTab(tab);
      navigate({ hash: `ability=${encodeURIComponent(a.id)}` }, { replace: true });
    },
    [navigate],
  );
  const close = useCallback(() => {
    setOpenId(null);
    setOpenTab(undefined);
    if (location.hash) navigate({ hash: "" }, { replace: true });
  }, [navigate, location.hash]);
  useEffect(() => {
    // The modal was opened by link but the ability is not here: drop the hash quietly.
    if (openId && data.data && !opened) close();
  }, [openId, data.data, opened, close]);

  const saveConfig = async (patch: (c: import("../api").Config) => import("../api").Config) => {
    setError(null);
    try {
      const c = await api.config();
      await api.saveConfig(patch(c));
      data.reload();
    } catch (e) {
      setError((e as Error).message);
    }
  };
  const prefer = (contract: string, id: string) => void saveConfig((c) => ({ ...c, abilities: { ...(c.abilities ?? {}), [contract]: id } }));
  const pickers = contracts.filter((c) => c.candidates.filter((x) => x.ready).length > 1);

  return (
    <div className="prof">
      <div className="cm-head">
        <div>
          <p className="cap">Setup</p>
          <h1 className="d2">Abilities</h1>
          <p className="muted">Everything Seemo can do here. Installed once for this machine; switched on, connected and taught per project. Click one to see what it includes and to set it up.</p>
        </div>
        <div className="cm-acts">
          <span className="chip">
            {on} of {abilities.length} on
          </span>
          {!draft && (
            <button className="btn p" onClick={() => setDraft({ name: "", description: "", text: "", everywhere: false, editing: null })}>
              <Plus size={15} /> Teach Seemo something
            </button>
          )}
        </div>
      </div>

      {draft && <TeachForm slug={slug} draft={draft} setDraft={setDraft} onDone={() => (setDraft(null), data.reload())} />}

      {data.data && pickers.length > 0 && (
        <div className="mk-top">
          {pickers.map((c) => (
              <label key={c.id} className="mk-pick-one">
                <span>{c.label} with</span>
                <select className="input sm" value={c.active?.id ?? c.configured ?? ""} onChange={(e) => prefer(c.id, e.target.value)}>
                  {c.candidates.map((x) => (
                    <option key={x.id} value={x.id} disabled={!x.ready}>
                      {x.name}
                      {x.ready ? "" : " (needs setup)"}
                    </option>
                  ))}
                </select>
              </label>
            ))}
        </div>
      )}
      {error && (
        <p className="err">
          <AlertTriangle size={13} /> {error}
        </p>
      )}

      <section className="mk-sec">
        <h2>
          What Seemo can do here <span className="n">{abilities.length}</span>
        </h2>
        {data.data && yours.length === 0 && !draft && (
          <div className="pcard none ab-none">
            <BookOpen size={22} className="ac2" />
            <p className="t">Nothing of your own yet</p>
            <p className="muted">Tell Seemo how you write a product update, how to publish to your blog, or what a weekly report should contain. It becomes an ability here, and Seemo follows it every time.</p>
            <button className="btn g" onClick={() => setDraft({ name: "", description: "", text: "", everywhere: false, editing: null })}>
              <Plus size={14} /> Teach Seemo something
            </button>
          </div>
        )}
        {yours.length > 0 && <Group label="Yours" items={yours} slug={slug} onOpen={open} onChange={data.reload} />}
        {fromMarket.length > 0 && <Group label="From the marketplace" items={fromMarket} slug={slug} onOpen={open} onChange={data.reload} />}
        {builtin.length > 0 && <Group label="Built in" items={builtin} slug={slug} cols={2} onOpen={open} onChange={data.reload} />}
      </section>

      <Marketplace slug={slug} items={[...market, ...fromMarket]} index={data.data?.index ?? ""} indexFetched={data.data?.index_fetched} loaded={!!data.data} onOpen={(a, tab) => open(a, tab)} onChange={data.reload} />

      <Advanced slug={slug} broken={data.data?.broken ?? []} envFile={data.data?.env_file ?? "~/.kiqr/.env"} projectEnvFile={data.data?.project_env_file ?? null} index={data.data?.index ?? ""} machineFolder={(data.data?.env_file ?? "~/.kiqr/.env").replace(/\/\.env$/, "/abilities")} onDone={data.reload} onGuide={() => guide && open(guide, "instructions")} />

      <AnimatePresence>{opened && <AbilityModal key={opened.id} slug={slug} a={opened} contracts={contracts} initialTab={openTab} onClose={close} onChange={data.reload} onPrefer={prefer} />}</AnimatePresence>
    </div>
  );
}

function byName(a: Ability, b: Ability) {
  return a.name.localeCompare(b.name);
}

function Group({ label, items, slug, cols, onOpen, onChange }: { label: string; items: Ability[]; slug: string; cols?: 2; onOpen: (a: Ability, tab?: "overview" | "instructions" | "connect") => void; onChange: () => void }) {
  return (
    <>
      <p className="cap ab-cap">{label}</p>
      <ul className={`mk-list ab-list ${cols === 2 ? "cols2" : ""}`}>
        {items.map((a) => (
          <Row key={a.id} slug={slug} a={a} onOpen={onOpen} onChange={onChange} />
        ))}
      </ul>
    </>
  );
}

/** One ability as a row: icon, name, vendor, state, what it includes, the project switch. */
function Row({ slug, a, onOpen, onChange }: { slug: string; a: Ability; onOpen: (a: Ability, tab?: "overview" | "instructions" | "connect") => void; onChange: () => void }) {
  const state = stateOf(a);
  const [busy, setBusy] = useState(false);
  const [updating, setUpdating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const toggle = async (want: boolean) => {
    setBusy(true);
    setError(null);
    try {
      if (want && a.machine_off) await api.resumeAbility(a.id);
      if (want) await api.resumeProjectAbility(slug, a.id);
      else await api.pauseProjectAbility(slug, a.id);
      onChange();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  const update = async () => {
    setUpdating(true);
    setError(null);
    try {
      await api.updateAbility(a.id, a.scope?.scope === "project" ? slug : undefined);
      onChange();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setUpdating(false);
    }
  };
  const needs = !a.ready && a.installed;
  const newer = a.installed && a.updatable && !!a.latest && a.latest !== a.version;
  return (
    <li className={`mk-row ab-row ${isOn(a) ? "on" : "off"}`}>
      <div className="main" role="button" tabIndex={0} onClick={() => onOpen(a)} onKeyDown={(e) => (e.key === "Enter" || e.key === " ") && (e.preventDefault(), onOpen(a))}>
        <span className={`ic ${a.avatar ? "has-avatar" : ""}`}>
          <AbilityIcon a={a} size={a.avatar ? 34 : 16} />
        </span>
        <div className="min0">
          <p className="nm">
            {a.name}
            <span className={`chip ${state.tone} sm`}>
              <b /> {state.text}
            </span>
          </p>
          <p className="ds">
            by <Author a={a} />
            {a.version && a.latest && a.latest !== a.version && !newer && <span className="warn"> · update available</span>}
            <span className="ab-parts">
              {a.instructions.length > 0 && (
                <button className="part" title="Read or edit how Seemo does this" onClick={(e) => (e.stopPropagation(), onOpen(a, "instructions"))}>
                  <BookOpen size={11} /> instructions
                </button>
              )}
              {a.auth.length > 0 && (
                <button className={`part ${a.missing.length ? "warn" : ""}`} title={a.core ? "KIQR's own key" : a.channels.length ? "This project's channels, each with its own account" : "This project's accounts"} onClick={(e) => (e.stopPropagation(), onOpen(a, "connect"))}>
                  <KeyRound size={11} /> {a.core ? "key" : a.channels.length ? "channels" : (a.accounts?.length ?? 0) > 1 ? `${a.accounts?.length} accounts` : "account"}
                </button>
              )}
              {a.transport === "mcp" ? (
                <span className={`part ${a.program_found === false ? "warn" : ""}`}>
                  <Plug size={11} /> MCP server
                </span>
              ) : a.runs_program ? (
                <span className="part">
                  <Terminal size={11} /> program
                </span>
              ) : null}
              {a.channels.length > 0 && (
                <span className="part">
                  <Send size={11} /> publishes
                </span>
              )}
              {a.active_for.length > 0 && (
                <span className="part acc">
                  <Sparkles size={11} /> preferred
                </span>
              )}
            </span>
            {needs && (a.core || !a.missing.length) && <span className="warn"> · needs {a.missing.length ? a.auth.filter((k) => a.missing.includes(k.env)).map((k) => k.label || k.env).join(", ") : a.program_found === false ? "its program" : "setup"}</span>}
          </p>
        </div>
        <div className="acts" onClick={(e) => e.stopPropagation()}>
          {newer && (
            <button className="btn p sm" disabled={updating} onClick={() => void update()} title={`Version ${a.version} is installed`}>
              {updating ? <Loader2 size={13} className="spin" /> : <RefreshCw size={13} />} Update to {a.latest}
            </button>
          )}
          <span title={a.paused || a.machine_off ? "Off for this project" : needs ? (a.channels.length ? "No channel yet; opens it so you can add one" : "Not connected yet; opens it so you can") : "On for this project"}>
            {/* A switch that cannot flip yet still does something: it opens the ability where the missing step is. */}
            <Switch on={!a.paused && !a.machine_off && a.ready} disabled={busy} onChange={(want) => (!a.ready ? onOpen(a, a.auth.length ? "connect" : "overview") : void toggle(want))} />
          </span>
        </div>
      </div>
      {error && (
        <p className="err">
          <AlertTriangle size={13} /> {error}
        </p>
      )}
    </li>
  );
}
