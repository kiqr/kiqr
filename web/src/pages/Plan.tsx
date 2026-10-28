// Your business → Plan: the marketing plan Seemo owns. Objectives for the
// period, each with its KPI and target; the campaigns under them with what
// was made for each. Seemo proposes the plan in a chat; here the
// user reads it, asks for a new one, and keeps campaigns by hand if they like.
import { useState } from "react";
import { Link, useParams } from "react-router-dom";
import { AlertTriangle, Check, ListChecks, Loader2, MessagesSquare, Plus, Send, Sparkles, Target, Trash2, X } from "lucide-react";
import { api, useAsync, useEvents, type Campaign, type Objective, type PerfKpi } from "../api";
import { fmt } from "./Performance";
import { openSeemo } from "../SeemoWidget";
import { networkName, useNetworks } from "../networks";
import { whenAt } from "./AssetActions";
import "../profile.css";
import "../chat.css";

export const PLAN_EVENTS = ["plan_saved", "campaign_saved", "campaign_deleted", "asset_saved", "asset_deleted", "task_saved", "task_deleted"];

const ASK_FOR_PLAN = "Propose a marketing plan for this period from the business brief: two to four objectives with a KPI each, what this period is for, and the channels in play. Read the Plan the marketing instructions first.";

export function PlanPage() {
  const { slug = "" } = useParams();
  const data = useAsync(() => api.plan(slug), [slug]);
  const perf = useAsync(() => api.performance(slug).catch(() => null), [slug]);
  useEvents(slug, (e) => {
    if (PLAN_EVENTS.includes(e.type)) data.reload();
    if (e.type === "metrics_snapshot" || e.type === "plan_saved") perf.reload();
  }, { demo: false });
  const [asking, setAsking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);
  const plan = data.data?.plan ?? null;
  const campaigns = data.data?.campaigns ?? [];
  const kpis = data.data?.kpis ?? [];
  const kpiLabel = (id: string) => kpis.find((k) => k.id === id)?.label ?? id;

  /** A new chat with the ask; Seemo answers with a plan card there and in the corner. */
  const ask = async (text: string) => {
    setAsking(true);
    setError(null);
    try {
      const c = await api.createChat(slug, plan ? "Revise the marketing plan" : "A marketing plan");
      openSeemo(c.id);
      await api.sendMessage(slug, c.id, text);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setAsking(false);
    }
  };

  return (
    <div className="prof plan">
      <div className="cm-head">
        <div>
          <p className="cap">Marketing</p>
          <h1 className="d2">Plan</h1>
          <p className="muted">The marketing plan Seemo works from: a few objectives for the period, each with a number to reach, and the campaigns under them. Seemo proposes it from what it knows about the business; you approve it.</p>
        </div>
        <div className="cm-acts">
          <button className="btn p" disabled={asking} onClick={() => void ask(plan ? `Review the marketing plan against what has happened since v${plan.version} and propose a revised one with propose_plan if the facts changed; otherwise tell me it still stands and why.` : ASK_FOR_PLAN)}>
            {asking ? <Loader2 size={15} className="spin" /> : <Sparkles size={15} />} {plan ? "Ask Seemo to review it" : "Ask Seemo for a plan"}
          </button>
        </div>
      </div>
      {error && (
        <p className="err">
          <AlertTriangle size={13} /> {error}
        </p>
      )}

      {data.data && !plan && (
        <div className="pcard none">
          <Target size={22} className="ac2" />
          <p className="t">No plan yet</p>
          <p className="muted">Seemo reads the brief and proposes objectives for the period, each with a KPI. You approve it in the chat, and from then on everything it makes is tied to one of them.</p>
          <button className="btn g" disabled={asking} onClick={() => void ask(ASK_FOR_PLAN)}>
            <MessagesSquare size={14} /> Ask Seemo for a plan
          </button>
        </div>
      )}

      {plan && (
        <>
          <PlanHero slug={slug} period={plan.period} version={plan.version} summary={plan.summary} channels={plan.channels} progress={overall(plan.objectives, perf.data?.kpis ?? [])} />
          <section className="gl plan-sec">
            <div className="gl-top">
              <span className="gl-ic">
                <Target size={14} />
              </span>
              <b>Objectives</b>
              <span className="gl-meta">{plan.objectives.length}</span>
            </div>
            <div className="plan-goals">
              {plan.objectives.map((o) => (
                <ObjectiveCard key={o.id} o={o} kpi={kpiLabel(o.kpi)} campaigns={campaigns.filter((c) => c.objective === o.id)} measured={perf.data?.kpis.find((k) => k.objective === o.id) ?? null} slug={slug} />
              ))}
            </div>
          </section>
        </>
      )}

      <section className="gl plan-sec">
        <div className="gl-top">
          <span className="gl-ic">
            <Sparkles size={14} />
          </span>
          <b>Campaigns</b>
          <span className="gl-meta">
            {campaigns.length}
            {!adding && (
              <button className="btn t sm" onClick={() => setAdding(true)}>
                <Plus size={13} /> New campaign
              </button>
            )}
          </span>
        </div>
        {adding && <CampaignForm slug={slug} objectives={plan?.objectives ?? []} onDone={() => (setAdding(false), data.reload())} />}
        {data.data && campaigns.length === 0 && !adding && <p className="muted sm gl-empty">None yet. A campaign is one initiative towards an objective, a launch or a content series, with the assets and tasks made for it. Seemo proposes them; you can add one here too.</p>}
        <div className="plan-camps">
          {campaigns.map((c) => (
            <CampaignCard key={c.id} slug={slug} c={c} onChange={data.reload} />
          ))}
        </div>
      </section>
    </div>
  );
}

/** KPIs where a smaller number is the better one (a search position). */
const LOWER_IS_BETTER = new Set(["rank_positions"]);

/** How far an objective is towards its target, 0 to 1, when something measures it. */
function progressOf(o: Objective, m: PerfKpi | null): number | null {
  if (!m?.measured || m.value == null || o.target == null || o.target === 0) return null;
  const r = LOWER_IS_BETTER.has(o.kpi) ? o.target / Math.max(m.value, 1e-9) : m.value / o.target;
  return Math.max(0, Math.min(1, r));
}

/** The measured objectives' progress, averaged; null when none is measured yet. */
function overall(objectives: Objective[], kpis: PerfKpi[]): { pct: number; measured: number; total: number } | null {
  const active = objectives.filter((o) => o.status === "active");
  const done = objectives.filter((o) => o.status === "done").length;
  const parts = active.map((o) => progressOf(o, kpis.find((k) => k.objective === o.id) ?? null)).filter((p): p is number => p != null);
  if (parts.length + done === 0) return null;
  const pct = (parts.reduce((a, b) => a + b, 0) + done) / (parts.length + done);
  return { pct, measured: parts.length + done, total: active.length + done };
}

function PlanHero({ slug, period, version, summary, channels, progress }: { slug: string; period: string; version: number; summary: string; channels: string[]; progress: { pct: number; measured: number; total: number } | null }) {
  const R = 34;
  const C = 2 * Math.PI * R;
  const networks = useNetworks(slug);
  const [all, setAll] = useState(false);
  const long = summary.length > 280;
  return (
    <section className="gl plan-hero">
      <div className="gl-top">
        <span className="gl-ic">
          <Target size={14} />
        </span>
        <b>Marketing plan</b>
        <span className="gl-meta">
          <span className="chip acc">{period}</span> v{version}
        </span>
      </div>
      <div className="plan-hero-body">
        <div className="plan-hero-main">
          {summary && (
            <p className={`plan-sum${long && !all ? " clamp" : ""}`}>
              {summary}
            </p>
          )}
          {long && (
            <button className="btn t sm plan-more" onClick={() => setAll(!all)}>
              {all ? "Show less" : "Read all"}
            </button>
          )}
          {channels.length > 0 && (
            <p className="plan-chans">
              <span className="cap">Channels</span>
              {channels.map((c) => (
                <Link key={c} className="tk" to={`/p/${slug}/channels`}>
                  <Send size={11} /> {networkName(c, networks)}
                </Link>
              ))}
            </p>
          )}
        </div>
        <div className="plan-ring" title={progress ? `${progress.measured} of ${progress.total} objectives measured` : "Nothing measured yet"}>
          <svg viewBox="0 0 80 80" aria-hidden="true">
            <defs>
              <linearGradient id="plan-ring-g" x1="0" x2="1" y1="0" y2="1">
                <stop offset="0" stopColor="var(--accent)" />
                <stop offset="1" stopColor="var(--accent-2)" />
              </linearGradient>
            </defs>
            <circle className="track" cx="40" cy="40" r={R} />
            {progress && progress.pct > 0 && <circle className="fill" cx="40" cy="40" r={R} strokeDasharray={`${C * progress.pct} ${C}`} />}
          </svg>
          <div className="plan-ring-c">
            <b>{progress ? `${Math.round(progress.pct * 100)}%` : "–"}</b>
            <small>{progress ? `${progress.measured} of ${progress.total} measured` : "not measured yet"}</small>
          </div>
        </div>
      </div>
    </section>
  );
}

function ObjectiveCard({ o, kpi, campaigns, measured, slug }: { o: Objective; kpi: string; campaigns: Campaign[]; measured: PerfKpi | null; slug: string }) {
  const target = o.target != null ? fmt(o.target, o.unit) : null;
  const p = o.status === "done" ? 1 : progressOf(o, measured);
  const has = measured?.measured && measured.value != null;
  const lower = LOWER_IS_BETTER.has(o.kpi);
  const change = has && measured!.prev ? (measured!.value! - measured!.prev) / measured!.prev : null;
  const good = change == null ? null : lower ? change < 0 : change > 0;
  const by = o.by ? new Date(o.by).toLocaleDateString([], { day: "numeric", month: "short" }) : null;
  const tip = [o.why, by && `By ${by}`, campaigns.length > 0 && (campaigns.length === 1 ? "1 campaign" : `${campaigns.length} campaigns`)].filter(Boolean).join(" · ");
  return (
    <article className={`gbox plan-row ${o.status}`} title={tip}>
      <div className="plan-row-top">
        <h3>{o.title}</h3>
        {o.status !== "active" ? (
          <span className="fig dim">{o.status}</span>
        ) : change != null && Math.abs(change) >= 0.005 ? (
          <span className={`fig ${good ? "" : "bad"}`}>
            {change > 0 ? "+" : "−"}
            {Math.round(Math.abs(change) * 100)}%
          </span>
        ) : p != null ? (
          <span className="fig dim">{Math.round(p * 100)}%</span>
        ) : null}
      </div>
      <div className="plan-bar" role="progressbar" aria-label={`${o.title}: progress`} aria-valuemin={0} aria-valuemax={100} aria-valuenow={p == null ? undefined : Math.round(p * 100)}>
        <span style={{ width: `${Math.round((p ?? 0) * 100)}%` }} />
      </div>
      <div className="plan-row-num">
        <span>{kpi}</span>
        {has ? (
          <Link to={`/p/${slug}/performance`}>
            <b>{fmt(measured!.value)}</b>
            {target ? ` of ${target}` : ""}
          </Link>
        ) : o.status === "active" ? (
          <Link className="connect" to={`/p/${slug}/performance`}>
            Connect a source
          </Link>
        ) : target ? (
          <span>
            of <b>{target}</b>
          </span>
        ) : null}
      </div>
    </article>
  );
}

/** Where today is between a campaign's start and end, 0 to 1. */
function through(start?: string | null, end?: string | null): number | null {
  if (!start || !end) return null;
  const a = Date.parse(start);
  const b = Date.parse(end);
  if (!(b > a)) return null;
  return Math.max(0, Math.min(1, (Date.now() - a) / (b - a)));
}

function CampaignCard({ slug, c, onChange }: { slug: string; c: Campaign; onChange: () => void }) {
  const [busy, setBusy] = useState<string | null>(null);
  const [confirm, setConfirm] = useState(false);
  const [open, setOpen] = useState(false);
  const run = async (what: string, fn: () => Promise<unknown>) => {
    setBusy(what);
    try {
      await fn();
      onChange();
    } finally {
      setBusy(null);
      setConfirm(false);
    }
  };
  const day = (d: string) => new Date(d).toLocaleDateString([], { day: "numeric", month: "short" });
  const at = through(c.start, c.end);
  const published = c.assets.filter((a) => a.status === "published").length;
  // The bar: how far through its dates, else how much of what was made went out.
  const p = c.status === "done" ? 1 : at ?? (c.assets.length ? published / c.assets.length : null);
  const when = c.start && c.end ? `${day(c.start)} – ${day(c.end)}` : c.start ? `from ${day(c.start)}` : c.end ? `until ${day(c.end)}` : null;
  const items = c.assets.length + c.tasks.length;
  const tip = [c.goal, c.objective_title && `For: ${c.objective_title}`, c.channels.length > 0 && c.channels.join(", "), c.created_by.startsWith("chat:") ? "Proposed by Seemo" : null].filter(Boolean).join(" · ");
  return (
    <article className={`gbox plan-row camp ${c.status}`} title={tip}>
      <div className="plan-row-top">
        <h3>{c.title}</h3>
        <span className="acts">
          {c.status === "active" ? (
            <button className="btn t sm" disabled={!!busy} title="Mark it done" onClick={() => void run("done", () => api.updateCampaign(slug, c.id, { status: "done" }))}>
              {busy === "done" ? <Loader2 size={12} className="spin" /> : <Check size={12} />} Done
            </button>
          ) : (
            <button className="btn t sm" disabled={!!busy} onClick={() => void run("active", () => api.updateCampaign(slug, c.id, { status: "active" }))}>
              Reopen
            </button>
          )}
          {confirm ? (
            <>
              <button className="btn t sm bad" disabled={!!busy} onClick={() => void run("rm", () => api.deleteCampaign(slug, c.id))}>
                Remove
              </button>
              <button className="btn t sm" aria-label="Keep it" onClick={() => setConfirm(false)}>
                <X size={12} />
              </button>
            </>
          ) : (
            <button className="btn t sm" aria-label="Remove" title="Remove the campaign; its assets and tasks stay" onClick={() => setConfirm(true)}>
              <Trash2 size={12} />
            </button>
          )}
        </span>
        <span className={`fig ${c.status === "active" ? "" : "dim"}`}>{c.status !== "active" ? c.status : at === 0 ? "upcoming" : at != null && at < 1 ? `${Math.round(at * 100)}%` : "active"}</span>
      </div>
      <div className="plan-bar">
        <span style={{ width: `${Math.round((p ?? 0) * 100)}%` }} />
      </div>
      <div className="plan-row-num">
        <span>{when ?? (c.objective_title || "no dates")}</span>
        {items > 0 ? (
          <button className="lnk" onClick={() => setOpen(!open)} aria-expanded={open}>
            <b>{published}</b> of {c.assets.length} out{c.tasks.length ? ` · ${c.tasks.length} task${c.tasks.length > 1 ? "s" : ""}` : ""}
          </button>
        ) : (
          <span>nothing made yet</span>
        )}
      </div>
      {open && (
        <ul className="plan-items">
          {c.tasks.map((t) => (
            <li key={`t-${t.id}`}>
              <ListChecks size={12} />
              <Link to={`/p/${slug}/tasks#${t.id}`}>{t.title}</Link>
              <small>
                task · {t.status}
                {t.next_run ? ` · next ${whenAt(t.next_run)}` : ""}
              </small>
            </li>
          ))}
          {c.assets.map((a) => (
            <li key={`a-${a.type}/${a.id}`}>
              <Sparkles size={12} />
              <Link to={`/p/${slug}/library/${a.type}/${a.id}`}>{a.title}</Link>
              <small>
                {a.type} · {a.status}
              </small>
            </li>
          ))}
        </ul>
      )}
    </article>
  );
}

function CampaignForm({ slug, objectives, onDone }: { slug: string; objectives: Objective[]; onDone: () => void }) {
  const [title, setTitle] = useState("");
  const [goal, setGoal] = useState("");
  const [objective, setObjective] = useState(objectives[0]?.id ?? "");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      await api.createCampaign(slug, { title, goal, objective: objective || undefined });
      onDone();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="pcard teach-form plan-form">
      <div className="grid">
        <label>
          <span className="nm">Campaign</span>
          <input className="input" value={title} placeholder="Launch: Visual Editor" onChange={(e) => setTitle(e.target.value)} />
        </label>
        <label>
          <span className="nm">Objective</span>
          <select className="input" value={objective} onChange={(e) => setObjective(e.target.value)}>
            <option value="">None</option>
            {objectives.map((o) => (
              <option key={o.id} value={o.id}>
                {o.title}
              </option>
            ))}
          </select>
        </label>
      </div>
      <label className="full">
        <span className="nm">Goal</span>
        <input className="input" value={goal} placeholder="Fifty sign-ups from the launch week" onChange={(e) => setGoal(e.target.value)} />
      </label>
      {error && <p className="err">{error}</p>}
      <div className="acts">
        <button className="btn p" disabled={busy || title.trim().length < 2} onClick={() => void save()}>
          {busy ? <Loader2 size={14} className="spin" /> : <Check size={14} />} Add campaign
        </button>
        <button className="btn t" onClick={onDone}>
          Cancel
        </button>
      </div>
    </div>
  );
}
