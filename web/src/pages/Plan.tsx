// Your business → Plan: the marketing plan Seemo owns. Objectives for the
// period, each with its KPI and target; the campaigns under them with what
// was made for each. Seemo proposes the plan on a card from a chat; here the
// user reads it, asks for a new one, and keeps campaigns by hand if they like.
import { useState } from "react";
import { Link, useParams } from "react-router-dom";
import { AlertTriangle, Calendar, Check, ListChecks, Loader2, MessagesSquare, Plus, Sparkles, Target, Trash2, X } from "lucide-react";
import { api, useAsync, useEvents, type Campaign, type Objective, type PerfKpi } from "../api";
import { fmt } from "./Performance";
import { openSeemo } from "../SeemoWidget";
import { ago } from "./Project";
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
          <p className="muted">The marketing plan Seemo works from: a few objectives for the period, each with a number to reach, and the campaigns under them. Seemo proposes it from the brief; you approve it on a card.</p>
        </div>
        <div className="cm-acts">
          {plan && (
            <span className="chip" title={`Version ${plan.version}, ${plan.updated_by}`}>
              {plan.period} · v{plan.version}
            </span>
          )}
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
        <section className="plan-sec">
          {plan.summary && <p className="plan-sum">{plan.summary}</p>}
          <h2>
            Objectives <span className="n">{plan.objectives.length}</span>
          </h2>
          <ul className="plan-objs">
            {plan.objectives.map((o) => (
              <ObjectiveRow key={o.id} o={o} kpi={kpiLabel(o.kpi)} campaigns={campaigns.filter((c) => c.objective === o.id)} measured={perf.data?.kpis.find((k) => k.objective === o.id) ?? null} slug={slug} />
            ))}
          </ul>
          {plan.channels.length > 0 && (
            <p className="muted sm">
              Channels in play: {plan.channels.join(", ")}. <Link to={`/p/${slug}/channels`}>Channels</Link>
            </p>
          )}
        </section>
      )}

      <section className="plan-sec">
        <h2>
          Campaigns <span className="n">{campaigns.length}</span>
          {!adding && (
            <button className="btn t sm" onClick={() => setAdding(true)}>
              <Plus size={13} /> New campaign
            </button>
          )}
        </h2>
        {adding && <CampaignForm slug={slug} objectives={plan?.objectives ?? []} onDone={() => (setAdding(false), data.reload())} />}
        {data.data && campaigns.length === 0 && !adding && <p className="muted sm">None yet. A campaign is one initiative towards an objective, a launch or a content series, with the assets and tasks made for it. Seemo proposes them; you can add one here too.</p>}
        <div className="plan-camps">
          {campaigns.map((c) => (
            <CampaignCard key={c.id} slug={slug} c={c} onChange={data.reload} />
          ))}
        </div>
      </section>
    </div>
  );
}

function ObjectiveRow({ o, kpi, campaigns, measured, slug }: { o: Objective; kpi: string; campaigns: Campaign[]; measured: PerfKpi | null; slug: string }) {
  const target = o.target != null ? `${o.target}${o.unit ? ` ${o.unit}` : ""}` : null;
  return (
    <li className={`plan-obj ${o.status}`}>
      <span className="ic">
        <Target size={15} />
      </span>
      <div className="min0">
        <p className="nm">
          {o.title}
          {o.status !== "active" && <span className="chip">{o.status}</span>}
        </p>
        {o.why && <p className="ds">{o.why}</p>}
        <p className="meta">
          <span className="chip acc">{kpi}</span>
          {measured?.measured && measured.value != null && (
            <Link className="now" to={`/p/${slug}/performance`}>
              now {fmt(measured.value, o.unit)}
              {o.target != null ? ` of ${target}` : ""}
            </Link>
          )}
          {target && !(measured?.measured && measured.value != null) && <span>target {target}</span>}
          {o.by && (
            <span>
              <Calendar size={11} /> by {new Date(o.by).toLocaleDateString([], { day: "numeric", month: "short", year: "numeric" })}
            </span>
          )}
          {campaigns.length > 0 && <span>{campaigns.length === 1 ? "1 campaign" : `${campaigns.length} campaigns`}</span>}
        </p>
      </div>
    </li>
  );
}

function CampaignCard({ slug, c, onChange }: { slug: string; c: Campaign; onChange: () => void }) {
  const [busy, setBusy] = useState<string | null>(null);
  const [confirm, setConfirm] = useState(false);
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
  const dates = [c.start, c.end].filter(Boolean).map((d) => new Date(d!).toLocaleDateString([], { day: "numeric", month: "short" }));
  return (
    <div className={`pcard plan-camp ${c.status}`}>
      <div className="row">
        <h3>
          {c.title}
          <span className={`chip ${c.status === "active" ? "ok" : ""}`}>{c.status}</span>
        </h3>
        <div className="acts">
          {c.status === "active" ? (
            <button className="btn t sm" disabled={!!busy} title="Mark it done" onClick={() => void run("done", () => api.updateCampaign(slug, c.id, { status: "done" }))}>
              {busy === "done" ? <Loader2 size={13} className="spin" /> : <Check size={13} />} Done
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
              <button className="btn t sm" onClick={() => setConfirm(false)}>
                <X size={13} />
              </button>
            </>
          ) : (
            <button className="btn t sm" aria-label="Remove" title="Remove the campaign; its assets and tasks stay" onClick={() => setConfirm(true)}>
              <Trash2 size={13} />
            </button>
          )}
        </div>
      </div>
      {c.goal && <p className="ds">{c.goal}</p>}
      <p className="meta">
        {c.objective_title && (
          <span>
            <Target size={11} /> {c.objective_title}
          </span>
        )}
        {c.channels.length > 0 && <span>{c.channels.join(", ")}</span>}
        {dates.length > 0 && (
          <span>
            <Calendar size={11} /> {dates.join(" to ")}
          </span>
        )}
        <span className="muted">{c.created_by.startsWith("chat:") ? "proposed by Seemo" : "yours"} · {c.updated_at ? ago(c.updated_at) : ""}</span>
      </p>
      {(c.assets.length > 0 || c.tasks.length > 0) && (
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
    </div>
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
