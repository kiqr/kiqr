// Seemo's tasks: what runs on its own, when, with which tools, and every run.
// The calendar is the same tasks as an agenda; month and week views come later.
import { useEffect, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { CalendarDays, Check, ChevronDown, ChevronRight, ListChecks, Loader2, MessagesSquare, Pause, Play, Send, Trash2, X } from "lucide-react";
import { api, useAsync, useDemoSeemo, useEvents, type CalendarItem, type Run, type Task } from "../api";
import { when } from "../chat/Conversation";
import { ago } from "./Project";
import "../profile.css";
import "../chat.css";

export function TasksPage() {
  const { slug = "" } = useParams();
  const tasks = useAsync(() => api.tasks(slug), [slug]);
  const onEvent = (e: { type: string }) => {
    if (e.type.startsWith("task_")) tasks.reload();
  };
  useEvents(slug, onEvent, { demo: false });
  useDemoSeemo(onEvent);
  const list = tasks.data ?? [];
  const active = list.filter((t) => t.status !== "done");
  const done = list.filter((t) => t.status === "done");
  return (
    <div className="prof">
      <div className="cm-head">
        <div>
          <p className="cap">Seemo</p>
          <h1 className="d2">Tasks</h1>
          <p className="muted">What Seemo does on its own. You confirm what a task may do once, when it is created; after that it runs on its schedule and every run is kept here.</p>
        </div>
      </div>
      {tasks.data && list.length === 0 && (
        <div className="pcard none">
          <ListChecks size={22} className="ac2" />
          <p className="t">No tasks yet</p>
          <p className="muted">Ask Seemo in a chat: “every weekday at 10:00, write a LinkedIn post” or “write three posts now”. It proposes a task, you approve it, and it appears here.</p>
          <Link className="btn g" to={`/p/${slug}/chats`}>
            <MessagesSquare size={14} /> Open chats
          </Link>
        </div>
      )}
      <div className="tasks">
        {active.map((t) => (
          <TaskRow key={t.id} slug={slug} t={t} onChange={tasks.reload} />
        ))}
      </div>
      {done.length > 0 && (
        <>
          <p className="cap" style={{ margin: "24px 0 10px" }}>
            Done
          </p>
          <div className="tasks">
            {done.map((t) => (
              <TaskRow key={t.id} slug={slug} t={t} onChange={tasks.reload} />
            ))}
          </div>
        </>
      )}
    </div>
  );
}

function TaskRow({ slug, t, onChange }: { slug: string; t: Task; onChange: () => void }) {
  const { hash } = window.location;
  const [open, setOpen] = useState(hash === `#${t.id}`);
  const [confirm, setConfirm] = useState(false);
  const [busy, setBusy] = useState(false);
  const runs = useAsync(() => (open ? api.runs(slug, t.id) : Promise.resolve([] as Run[])), [slug, t.id, open, t.last_run, t.running]);
  useEffect(() => {
    if (hash === `#${t.id}`) document.getElementById(t.id)?.scrollIntoView({ block: "center" });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);
  const act = async (fn: () => Promise<unknown>) => {
    setBusy(true);
    try {
      await fn();
      onChange();
    } finally {
      setBusy(false);
      setConfirm(false);
    }
  };
  const state = t.running ? "running" : t.last_status ?? "scheduled";
  const chip = t.running ? (
    <span className="chip acc live">
      <b /> Running
    </span>
  ) : t.status === "paused" ? (
    <span className="chip">Paused</span>
  ) : t.status === "done" ? (
    <span className={`chip ${t.last_status === "ok" ? "ok" : "bad"}`}>{t.last_status === "ok" ? "Done" : "Failed"}</span>
  ) : (
    <span className="chip ai">Scheduled</span>
  );
  return (
    <div className={`pcard task ${state}`} id={t.id}>
      <div className="row">
        <span className="ic">{t.running ? <Loader2 size={16} className="spin" /> : <ListChecks size={16} />}</span>
        <div className="min0">
          <h3>
            {t.title} {chip}
          </h3>
          <p className="meta">
            <span>{t.schedule_text}</span>
            {t.next_run && t.status === "active" && <span>next {when(t.next_run)}</span>}
            {t.last_run && (
              <span>
                last {ago(t.last_run)}
                {t.last_status && t.last_status !== "running" ? `, ${t.last_status}` : ""}
              </span>
            )}
            <span>
              {t.runs} run{t.runs === 1 ? "" : "s"}
            </span>
            {t.created_by.startsWith("chat:") && <Link to={`/p/${slug}/chats/${t.created_by.slice(5)}`}>from a chat</Link>}
          </p>
          <p className="instr">{t.instruction}</p>
          <p className="meta" style={{ marginTop: 6 }}>
            {t.allowed_labels.map((l) => (
              <span key={l}>
                <Check size={11} style={{ verticalAlign: -1, color: "var(--ok)" }} /> {l}
              </span>
            ))}
          </p>
        </div>
        <div className="acts">
          {!t.running && t.status !== "done" && (
            <button className="btn t" disabled={busy} title="Run now" aria-label="Run now" onClick={() => void act(() => api.runTask(slug, t.id))}>
              <Play size={15} />
            </button>
          )}
          {t.status === "active" && !t.running && (
            <button className="btn t" disabled={busy} title="Pause" aria-label="Pause" onClick={() => void act(() => api.pauseTask(slug, t.id))}>
              <Pause size={15} />
            </button>
          )}
          {t.status === "paused" && (
            <button className="btn t" disabled={busy} title="Resume" aria-label="Resume" onClick={() => void act(() => api.resumeTask(slug, t.id))}>
              <Play size={15} />
            </button>
          )}
          {confirm ? (
            <>
              <button className="btn t bad" disabled={busy} title="Yes, delete" aria-label="Yes, delete" onClick={() => void act(() => api.deleteTask(slug, t.id))}>
                <Check size={15} />
              </button>
              <button className="btn t" title="Keep it" aria-label="Keep it" onClick={() => setConfirm(false)}>
                <X size={15} />
              </button>
            </>
          ) : (
            <button className="btn t" disabled={busy || t.running} title="Delete" aria-label="Delete" onClick={() => setConfirm(true)}>
              <Trash2 size={15} />
            </button>
          )}
          <button className="btn t" title={open ? "Hide runs" : "Show runs"} aria-label={open ? "Hide runs" : "Show runs"} aria-expanded={open} onClick={() => setOpen((o) => !o)}>
            {open ? <ChevronDown size={15} /> : <ChevronRight size={15} />}
          </button>
        </div>
      </div>
      {open && (
        <div className="runs">
          {runs.data?.length === 0 && <p className="muted" style={{ fontSize: 13 }}>No runs yet.</p>}
          {runs.data?.map((r) => (
            <RunRow key={r.id} slug={slug} r={r} />
          ))}
        </div>
      )}
    </div>
  );
}

function RunRow({ slug, r }: { slug: string; r: Run }) {
  const [more, setMore] = useState(false);
  const cls = r.status === "ok" ? "ok" : r.status === "running" ? "acc live" : r.status === "missed" ? "" : "bad";
  return (
    <div className="run">
      <div className="hd">
        <span className={`chip ${cls}`}>
          <b /> {r.status === "ok" ? "Done" : r.status === "running" ? "Running" : r.status === "missed" ? "Missed" : "Failed"}
        </span>
        <time>{when(r.started)}</time>
        <span className="muted">{r.trigger === "schedule" ? "on schedule" : r.trigger === "approval" ? "right after approval" : r.trigger === "startup" ? "caught up after a restart" : "on request"}</span>
        {r.cost_usd != null && <span className="cost">${r.cost_usd.toFixed(3)}</span>}
      </div>
      {r.summary && <p className="sum">{more ? r.summary : r.summary.split("\n")[0]}</p>}
      {r.summary.includes("\n") && (
        <button className="btn t sm" onClick={() => setMore((m) => !m)}>
          {more ? "Less" : "Whole report"}
        </button>
      )}
      {r.assets.length > 0 && (
        <div className="assets">
          {r.assets.map((a) => (
            <Link key={a} to={`/p/${slug}/library/${a}`}>
              {a}
            </Link>
          ))}
        </div>
      )}
      {r.steps.length > 0 && (
        <div className="steps open">
          <ul>
            {r.steps.map((s, i) => (
              <li key={i} className={s.ok === false ? "bad" : ""}>
                {s.ok === false ? <X size={12} /> : <Check size={12} />}
                <span>{s.label}</span>
                {s.note && <small>{s.note}</small>}
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------- calendar

export function CalendarPage() {
  const { slug = "" } = useParams();
  const items = useAsync(() => api.calendar(slug), [slug]);
  const tasks = useAsync(() => api.tasks(slug), [slug]);
  const onEvent = (e: { type: string }) => {
    if (e.type.startsWith("task_")) {
      items.reload();
      tasks.reload();
    }
    // A publish was scheduled, moved, done or dropped.
    if (e.type === "asset_approved" || e.type === "asset_saved" || e.type === "asset_published") items.reload();
  };
  useEvents(slug, onEvent, { demo: false });
  useDemoSeemo(onEvent);
  const weeks = groupByWeek(items.data ?? []);
  const paused = (tasks.data ?? []).filter((t) => t.status === "paused");
  const today = new Date().toDateString();
  return (
    <div className="prof">
      <div className="cm-head">
        <div>
          <p className="cap">Seemo</p>
          <h1 className="d2">Calendar</h1>
          <p className="muted">The next 30 days: task runs and the publishes you scheduled. Month and week views follow in a later release.</p>
        </div>
      </div>
      {items.data && items.data.length === 0 && (
        <div className="pcard none">
          <CalendarDays size={22} className="ac2" />
          <p className="t">Nothing scheduled</p>
          <p className="muted">Tell Seemo when: “every weekday at 10:00, reply to relevant Reddit posts” or “every day in December until the 24th, post the calendar meme”.</p>
          <Link className="btn g" to={`/p/${slug}/chats`}>
            <MessagesSquare size={14} /> Open chats
          </Link>
        </div>
      )}
      {items.data && items.data.length > 0 && (
        <div className="agenda">
          <div className="weeks">
            {weeks.map((w) => (
              <div className="week pcard" key={w.label}>
                <p className="cap">{w.label}</p>
                {w.days.map((d) => (
                  <div className={`day ${d.date.toDateString() === today ? "today" : ""}`} key={d.date.toISOString()}>
                    <div className="d">
                      <small>{d.date.toLocaleDateString([], { weekday: "short" })}</small>
                      <b>{d.date.getDate()}</b>
                    </div>
                    <div className="items">
                      {d.items.map((it, i) =>
                        it.kind === "publish" && it.asset ? (
                          <Link className="item publish" key={i} to={`/p/${slug}/library/${it.asset.type}/${it.asset.id}`}>
                            <time>{new Date(it.at).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}</time>
                            <Send size={12} />
                            <span>{it.title}</span>
                            <small>publish {it.schedule_text}</small>
                          </Link>
                        ) : (
                          <Link className="item" key={i} to={`/p/${slug}/tasks#${it.task}`}>
                            <time>{new Date(it.at).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}</time>
                            <span>{it.title}</span>
                            <small>{it.schedule_text}</small>
                          </Link>
                        ),
                      )}
                    </div>
                  </div>
                ))}
              </div>
            ))}
          </div>
          <aside className="side pcard">
            <p className="cap">
              <ListChecks size={13} /> Repeating tasks
            </p>
            <ul>
              {(tasks.data ?? [])
                .filter((t) => t.schedule.kind === "rrule" && t.status === "active")
                .map((t) => (
                  <li key={t.id}>
                    <b>{t.title}</b>
                    <span>{t.schedule_text}</span>
                  </li>
                ))}
              {paused.map((t) => (
                <li key={t.id}>
                  <b>{t.title}</b>
                  <span>paused</span>
                </li>
              ))}
            </ul>
            <Link className="btn t sm" to={`/p/${slug}/tasks`} style={{ marginTop: 10 }}>
              All tasks
            </Link>
          </aside>
        </div>
      )}
    </div>
  );
}

function groupByWeek(items: CalendarItem[]) {
  const weeks: { label: string; days: { date: Date; items: CalendarItem[] }[] }[] = [];
  for (const it of items) {
    const d = new Date(it.at);
    const monday = new Date(d);
    monday.setHours(0, 0, 0, 0);
    monday.setDate(d.getDate() - ((d.getDay() + 6) % 7));
    const label = `Week of ${monday.toLocaleDateString([], { day: "numeric", month: "long" })}`;
    let w = weeks.find((x) => x.label === label);
    if (!w) {
      w = { label, days: [] };
      weeks.push(w);
    }
    let day = w.days.find((x) => x.date.toDateString() === d.toDateString());
    if (!day) {
      day = { date: new Date(d.getFullYear(), d.getMonth(), d.getDate()), items: [] };
      w.days.push(day);
    }
    day.items.push(it);
  }
  return weeks;
}
