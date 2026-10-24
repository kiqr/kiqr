// The transcript of one chat, live: messages, the reply as it streams, the
// tools Seemo is using, and the cards (an approval waiting for a yes, a task
// running). Used full-size on the Chats page and compact in the widget.
import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { Link } from "react-router-dom";
import { AlertCircle, ArrowUp, BookOpen, Check, ChevronDown, ChevronRight, ExternalLink, ListChecks, Loader2, Send, Settings2, Square, X } from "lucide-react";
import { api, useAsync, useEvents, type Card, type Message, type Step, type Streaming, type Task } from "../api";
import { Markdown } from "../markdown";
import { ago } from "../pages/Project";

export function Conversation({ slug, messages, streaming, compact, trailing }: { slug: string; messages: Message[]; streaming: Streaming | null; compact?: boolean; trailing?: React.ReactNode }) {
  const box = useRef<HTMLDivElement>(null);
  const stick = useRef(true);
  // Follow the bottom while the reader is there; leave them alone when they scrolled up.
  useEffect(() => {
    const el = box.current;
    if (!el) return;
    const onScroll = () => {
      stick.current = el.scrollHeight - el.scrollTop - el.clientHeight < 80;
    };
    el.addEventListener("scroll", onScroll);
    return () => el.removeEventListener("scroll", onScroll);
  }, []);
  useEffect(() => {
    const el = box.current;
    if (el && stick.current) el.scrollTop = el.scrollHeight;
  }, [messages, streaming?.text, streaming?.steps.length, trailing]);

  return (
    <div className={`conv ${compact ? "compact" : ""}`} ref={box}>
      {messages.map((m) => (
        <MessageView key={m.id} slug={slug} m={m} compact={compact} />
      ))}
      {streaming && (
        <div className="cm ai live">
          <span className="av" />
          <div className="body">
            <Steps steps={streaming.steps} live />
            {streaming.text ? (
              <Markdown text={streaming.text} className="chat-md" />
            ) : (
              <p className="thinking">
                <i />
                <i />
                <i />
              </p>
            )}
          </div>
        </div>
      )}
      {trailing}
    </div>
  );
}

function MessageView({ slug, m, compact }: { slug: string; m: Message; compact?: boolean }) {
  if (m.role === "user")
    return (
      <div className="cm you">
        <div className="ms y">
          {m.text}
          <span className="t">{time(m.at)}</span>
        </div>
      </div>
    );
  if (m.role === "card" && m.card) return <CardView slug={slug} card={m.card} id={m.id} compact={compact} />;
  if (m.role === "note")
    return (
      <div className="cm note" title={m.text}>
        <ListChecks size={13} />
        <span>{m.text.split("\n")[0]}</span>
      </div>
    );
  return (
    <div className="cm ai">
      <span className="av" />
      <div className="body">
        <Steps steps={m.steps ?? []} />
        <Markdown text={m.text} className="chat-md" />
        {m.error && (
          <p className="err">
            <AlertCircle size={13} /> {m.error}
          </p>
        )}
        <span className="t">{time(m.at)}</span>
      </div>
    </div>
  );
}

/** The tools behind a message: one line while running, a folded list afterwards. */
function Steps({ steps, live }: { steps: Step[]; live?: boolean }) {
  const [open, setOpen] = useState(false);
  if (!steps.length) return null;
  const running = steps.find((s) => s.ok === null);
  const failed = steps.filter((s) => s.ok === false).length;
  const images = steps.filter((s) => s.image).map((s) => s.image!);
  const gallery = images.length > 0 && (
    <div className="step-images">
      {images.map((u) => (
        <a key={u} href={u} target="_blank" rel="noreferrer">
          <img src={u} alt="" loading="lazy" />
        </a>
      ))}
    </div>
  );
  if (live && running)
    return (
      <>
        {gallery}
        <p className="steps-live">
          <Loader2 size={13} className="spin" /> {running.label}…
        </p>
      </>
    );
  const summary = steps.length === 1 ? steps[0].label : `${steps.length} steps${failed ? `, ${failed} failed` : ""}`;
  return (
    <div className={`steps ${open ? "open" : ""}`}>
      <button className="steps-sum" onClick={() => setOpen((o) => !o)} aria-expanded={open}>
        {open ? <ChevronDown size={13} /> : <ChevronRight size={13} />}
        {summary}
      </button>
      {open && (
        <ul>
          {steps.map((s, i) => (
            <li key={i} className={s.ok === false ? "bad" : ""}>
              {s.ok === null ? <Loader2 size={12} className="spin" /> : s.ok ? <Check size={12} /> : <X size={12} />}
              <span>{s.label}</span>
              {s.note && <small>{s.note}</small>}
            </li>
          ))}
        </ul>
      )}
      {gallery}
    </div>
  );
}

// ------------------------------------------------------------------ cards

function CardView({ slug, card, id, compact }: { slug: string; card: Card; id: string; compact?: boolean }) {
  if (card.kind === "approval" && card.status === "pending") return <ApprovalCard slug={slug} card={card} compact={compact} />;
  if (card.approval_kind === "task" && card.task) return <TaskCard slug={slug} taskId={card.task} compact={compact} />;
  if (card.approval_kind === "task") return <Resolved kind="task" status={card.status} title={String(card.payload.title ?? "Task")} />;
  if (card.approval_kind === "skill") return <Resolved kind="skill" status={card.status} title={String(card.payload.name ?? "Skill")} />;
  if (card.approval_kind === "channel") return <Resolved kind="channel" status={card.status} title={String(card.payload.label ?? "Channel")} />;
  if (card.approval_kind === "publish" || card.approval_kind === "email") {
    const p = card.payload as Record<string, unknown>;
    const title = card.approval_kind === "email" ? `“${String(p.subject ?? "")}” to ${String(p.to ?? "")}` : `${String((p.asset as { title?: string })?.title ?? "Asset")} → ${String(p.channel_label ?? p.destination_text ?? "")}`;
    const url = card.result?.url as string | undefined;
    return (
      <div className={`ccard ${card.status}`}>
        <p className="cap">
          <Send size={13} /> {card.status === "approved" ? (card.approval_kind === "email" ? "Sent" : "Published") : `${card.approval_kind === "email" ? "Email" : "Publish"} ${card.status}`}
        </p>
        <p className="ttl">{title}</p>
        {url && (
          <p className="sm">
            <a href={url} target="_blank" rel="noreferrer">
              {url} <ExternalLink size={11} />
            </a>
          </p>
        )}
      </div>
    );
  }
  if (card.approval_kind === "config") {
    const changes = (card.payload.changes as { key: string; from: unknown; to: unknown }[] | undefined) ?? [];
    return (
      <div className={`ccard ${card.status}`} data-id={id}>
        <p className="cap">
          <Settings2 size={13} /> Settings {card.status === "approved" ? "changed" : card.status}
        </p>
        <ul className="kvs">
          {changes.map((c) => (
            <li key={c.key}>
              <code>{c.key}</code> <span className="from">{String(c.from)}</span> <span className="to">{String(c.to)}</span>
            </li>
          ))}
        </ul>
        {card.status === "approved" && (card.result?.restart as boolean) && <p className="warn">Host or port changed: restart the daemon to apply it.</p>}
      </div>
    );
  }
  return null;
}

function Resolved({ kind, status, title }: { kind: string; status: string; title: string }) {
  return (
    <div className={`ccard ${status}`}>
      <p className="cap">
        {kind === "skill" ? <BookOpen size={13} /> : kind === "channel" ? <Send size={13} /> : <ListChecks size={13} />} {kind === "task" ? "Task" : kind === "skill" ? "Skill" : kind === "channel" ? "Channel" : "Change"} {status === "approved" && (kind === "skill" || kind === "channel") ? "added" : status}
      </p>
      <p className="ttl">{title}</p>
    </div>
  );
}

/** Seemo wants a yes: a task to create, or settings to change. */
function ApprovalCard({ slug, card, compact }: { slug: string; card: Card; compact?: boolean }) {
  const [busy, setBusy] = useState<"approve" | "decline" | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [more, setMore] = useState(false);
  const p = card.payload as Record<string, unknown>;
  const decide = async (d: "approve" | "decline") => {
    setBusy(d);
    setError(null);
    try {
      await api.resolveApproval(slug, card.approval!, d);
    } catch (e) {
      setError((e as Error).message);
      setBusy(null);
    }
  };
  const isTask = card.approval_kind === "task";
  const isSkill = card.approval_kind === "skill";
  const isPublish = card.approval_kind === "publish";
  const isEmail = card.approval_kind === "email";
  const isChannel = card.approval_kind === "channel";
  const next = ((p.next as string[]) ?? []).slice(0, 3);
  const instruction = String(p.instruction ?? "");
  const skillText = String(p.text ?? "");
  const files = (p.files as { name: string; bytes: number }[] | undefined) ?? [];
  const caption = isTask ? "Seemo proposes a task" : isSkill ? "Seemo wants to remember a skill" : isPublish ? "Seemo wants to publish" : isEmail ? "Seemo wants to send an email" : isChannel ? "Seemo proposes a channel" : "Seemo proposes a settings change";
  const yes = isTask ? "Approve and run" : isSkill ? "Save the skill" : isPublish ? "Publish it" : isEmail ? "Send it" : isChannel ? "Add the channel" : "Apply";
  return (
    <div className="ccard pending">
      <p className="cap">
        {isTask ? <ListChecks size={13} /> : isSkill ? <BookOpen size={13} /> : isPublish || isEmail || isChannel ? <Send size={13} /> : <Settings2 size={13} />} {caption}
      </p>
      {isPublish || isEmail ? (
        <>
          <p className="ttl">{isEmail ? String(p.subject ?? "") : String((p.asset as { title?: string })?.title ?? "")}</p>
          <p className="sched">
            {isEmail ? (
              <>
                to <b>{String(p.to ?? "")}</b>
              </>
            ) : (
              <>
                to <b>{String(p.channel_label ?? "")}</b> <span className="muted">· {String(p.destination_text ?? "")}</span>
              </>
            )}
            <span className="muted"> · via {String(p.connection_name ?? "")}</span>
          </p>
          {!!p.excerpt && <p className={`instr ${more ? "open" : ""}`}>{String(p.excerpt)}</p>}
          {String(p.excerpt ?? "").length > 220 && !compact && (
            <button className="btn t sm" onClick={() => setMore((m) => !m)}>
              {more ? "Less" : "Read it all"}
            </button>
          )}
          {files.length > 0 && <p className="muted sm">Attached: {files.map((f) => f.name).join(", ")}</p>}
          {!!p.note && <p className="muted sm">{String(p.note)}</p>}
        </>
      ) : isChannel ? (
        <>
          <p className="ttl">{String(p.label ?? "")}</p>
          <p className="sched">
            {String(p.kind_label ?? p.kind ?? "")} via {String(p.connection_name ?? "")}
          </p>
          <ul className="kvs">
            {Object.entries((p.destination as Record<string, unknown>) ?? {}).map(([k, v]) => (
              <li key={k}>
                <code>{k}</code> <span className="to">{String(v)}</span>
              </li>
            ))}
          </ul>
          <p className="muted sm">Publishing to it will still ask you each time.</p>
        </>
      ) : isSkill ? (
        <>
          <p className="ttl">{String(p.name)}</p>
          <p className="sched">{String(p.description ?? "")}</p>
          <p className={`instr ${more ? "open" : ""}`}>{skillText}</p>
          {skillText.length > 220 && !compact && (
            <button className="btn t sm" onClick={() => setMore((m) => !m)}>
              {more ? "Less" : "Read the whole skill"}
            </button>
          )}
          <p className="muted sm">Kept for {String(p.scope_text ?? "this project")}; Seemo reads it before matching requests.</p>
        </>
      ) : isTask ? (
        <>
          <p className="ttl">{String(p.title)}</p>
          <p className="sched">
            <b>{String(p.schedule_text)}</b>
            {next.length > 0 && <span className="muted"> · {next.map((n) => when(n)).join(", ")}</span>}
          </p>
          <p className={`instr ${more ? "open" : ""}`}>{instruction}</p>
          {instruction.length > 220 && !compact && (
            <button className="btn t sm" onClick={() => setMore((m) => !m)}>
              {more ? "Less" : "Read the whole instruction"}
            </button>
          )}
          <ul className="allow">
            {((p.allowed_labels as string[]) ?? []).map((l) => (
              <li key={l}>
                <Check size={12} /> {l}
              </li>
            ))}
          </ul>
          {(p.follow_up as boolean) && <p className="muted sm">Seemo will report here when it has run.</p>}
        </>
      ) : (
        <>
          <ul className="kvs">
            {((p.changes as { key: string; from: unknown; to: unknown }[]) ?? []).map((c) => (
              <li key={c.key}>
                <code>{c.key}</code> <span className="from">{String(c.from)}</span> <span className="to">{String(c.to)}</span>
              </li>
            ))}
          </ul>
          {!!p.reason && <p className="muted sm">{String(p.reason)}</p>}
          {(p.restart as boolean) && <p className="warn">Host or port change: applies after the daemon restarts.</p>}
        </>
      )}
      {error && (
        <p className="err">
          <AlertCircle size={13} /> {error}
        </p>
      )}
      <div className="acts">
        <button className="btn p" disabled={!!busy} onClick={() => decide("approve")}>
          {busy === "approve" ? <Loader2 size={14} className="spin" /> : <Check size={14} />} {yes}
        </button>
        <button className="btn g" disabled={!!busy} onClick={() => decide("decline")}>
          Decline
        </button>
      </div>
    </div>
  );
}

/** A task this chat created, kept live from events. */
export function TaskCard({ slug, taskId, compact }: { slug: string; taskId: string; compact?: boolean }) {
  const task = useAsync(() => api.task(slug, taskId), [slug, taskId]);
  const [step, setStep] = useState<string | null>(null);
  const [assets, setAssets] = useState(0);
  useEvents(
    slug,
    (e) => {
      if (e.task !== taskId) return;
      if (e.type === "task_run_started") {
        setStep("Starting…");
        setAssets(0);
        task.reload();
      }
      if (e.type === "task_run_step") {
        if (e.status === "started") setStep(e.label as string);
        if (e.status === "done" && e.label === "Adding to the library") setAssets((n) => n + 1);
      }
      if (e.type === "task_run_finished" || e.type === "task_saved") {
        setStep(null);
        task.reload();
      }
    },
    { demo: false },
  );
  const t: Task | undefined = task.data;
  if (!t) return <div className="ccard task" />;
  const running = t.running || t.last_status === "running";
  const state = running ? "running" : t.last_status ?? (t.status === "paused" ? "paused" : "scheduled");
  return (
    <div className={`ccard task ${state}`}>
      <p className="cap">
        <ListChecks size={13} /> Task
        <span className={`chip ${running ? "acc live" : state === "ok" ? "ok" : state === "failed" ? "bad" : ""}`}>
          <b /> {running ? "Running" : state === "ok" ? "Done" : state === "failed" ? "Failed" : state === "missed" ? "Missed" : t.status === "paused" ? "Paused" : "Scheduled"}
        </span>
      </p>
      <p className="ttl">{t.title}</p>
      <p className="sched">
        {t.schedule_text}
        {t.next_run && t.status === "active" && <span className="muted"> · next {when(t.next_run)}</span>}
      </p>
      {running && (
        <p className="steps-live">
          <Loader2 size={13} className="spin" /> {step ?? "Working…"}
          {assets > 0 && <span className="muted"> · {assets} added to the library</span>}
        </p>
      )}
      {!compact && (
        <div className="acts">
          <Link className="btn t sm" to={`/p/${slug}/tasks#${t.id}`}>
            Open in Tasks <ExternalLink size={12} />
          </Link>
          {t.last_status === "ok" && (
            <Link className="btn t sm" to={`/p/${slug}/library`}>
              See the library <ExternalLink size={12} />
            </Link>
          )}
        </div>
      )}
    </div>
  );
}

// --------------------------------------------------------------- composer

export function Composer({ onSend, onStop, busy, placeholder, compact, autoFocus }: { onSend: (text: string) => Promise<void> | void; onStop?: () => void; busy?: boolean; placeholder?: string; compact?: boolean; autoFocus?: boolean }) {
  const [text, setText] = useState("");
  const [sending, setSending] = useState(false);
  const ref = useRef<HTMLTextAreaElement>(null);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, compact ? 120 : 200)}px`;
  }, [text, compact]);
  const submit = async () => {
    const t = text.trim();
    if (!t || sending || busy) return;
    setSending(true);
    try {
      await onSend(t);
      setText("");
    } catch {
      /* the caller shows the error */
    } finally {
      setSending(false);
      ref.current?.focus();
    }
  };
  const onKey = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      void submit();
    }
  };
  return (
    <div className={`composer ${compact ? "compact" : ""}`}>
      <textarea ref={ref} rows={1} value={text} onChange={(e) => setText(e.target.value)} onKeyDown={onKey} placeholder={placeholder ?? "Ask Seemo, or tell it what to do"} autoFocus={autoFocus} />
      {busy && onStop ? (
        <button className="send stop" onClick={onStop} aria-label="Stop" title="Stop">
          <Square size={14} />
        </button>
      ) : (
        <button className="send" onClick={() => void submit()} disabled={!text.trim() || sending || busy} aria-label="Send" title="Send (Enter)">
          {sending ? <Loader2 size={15} className="spin" /> : <ArrowUp size={16} />}
        </button>
      )}
    </div>
  );
}

// ---------------------------------------------------------------- helpers

export function time(iso: string) {
  return new Date(iso).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

/** "Mon 26 Oct 09:00", or just the time today. */
export function when(iso: string) {
  const d = new Date(iso);
  const now = new Date();
  const sameDay = d.toDateString() === now.toDateString();
  const t = d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  if (sameDay) return `today ${t}`;
  return `${d.toLocaleDateString([], { weekday: "short", day: "numeric", month: "short" })} ${t}`;
}

export { ago };
