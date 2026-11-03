// The transcript of one chat, live: messages, the reply as it streams, the
// tools Seemo is using, and the cards (an approval waiting for a yes, a task
// running). Used full-size on the Chats page and compact in the widget.
import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { Link } from "react-router-dom";
import { AlertCircle, ArrowUp, BookOpen, Brain, Check, ChevronDown, ChevronRight, ExternalLink, ListChecks, Loader2, Send, Settings2, Square, Target, Users, X, Zap, Radio } from "lucide-react";
import { api, useAsync, useEvents, type Card, type Message, type Step, type Streaming, type Task } from "../api";
import { Markdown } from "../markdown";
import { ago } from "../pages/Project";
import { AssetEmbed } from "../pages/AssetEmbed";

/** A line from outside the chat (the crawl's narration), placed among the messages by its time. */
export type Note = { text: string; at: Date };

export function Conversation({ slug, messages, streaming, compact, notes = [], bare }: { slug: string; messages: Message[]; streaming: Streaming | null; compact?: boolean; notes?: Note[]; /** Just the rows, inside someone else's scrolling list (Seemo's first conversation). */ bare?: boolean }) {
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
  }, [messages, streaming?.text, streaming?.steps.length, notes.length]);

  // A card the daemon put in mid-turn comes after the words of that turn: below Seemo's message, or below the
  // live bubble while it still works, never above what led to it.
  const ordered: Message[] = [];
  let pendingCards: Message[] = [];
  for (const m of messages) {
    if (m.role === "card") {
      pendingCards.push(m);
      continue;
    }
    if (m.role !== "user" && m.role !== "note") {
      ordered.push(m, ...pendingCards);
      pendingCards = [];
      continue;
    }
    ordered.push(...pendingCards);
    pendingCards = [];
    ordered.push(m);
  }
  const trailing = pendingCards;
  // A publish proposal is shown as the asset's own card, so the asset is not shown again among the steps;
  // of several proposals for one asset, the last carries the card and earlier ones stay one line.
  const proposed = new Map<string, string>();
  for (const m of messages) {
    const k = m.role === "card" && m.card ? publishedAsset(m.card) : null;
    if (k) proposed.set(k, m.id);
  }
  const skip = new Set(proposed.keys());
  const lastFor = new Set(proposed.values());
  // Messages and notes in time order; notes in a row share one box.
  const rows: ({ m: Message } | { notes: Note[] })[] = [];
  let ni = 0;
  const flush = (until: number) => {
    const run: Note[] = [];
    while (ni < notes.length && notes[ni].at.getTime() <= until) run.push(notes[ni++]);
    if (run.length) rows.push({ notes: run });
  };
  for (const m of ordered) {
    flush(new Date(m.at).getTime());
    rows.push({ m });
  }
  flush(Infinity);

  const content = (
    <>
      {rows.map((r) =>
        "m" in r ? (
          <MessageView key={r.m.id} slug={slug} m={r.m} compact={compact} skip={skip} last={lastFor.has(r.m.id)} />
        ) : (
          <div key={`n-${r.notes[0].at.getTime()}`} className="narr" aria-label="From the crawl">
            {r.notes.map((n, i) => (
              <p key={i}>
                <span className="t">{n.at.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}</span>
                {n.text}
              </p>
            ))}
          </div>
        ),
      )}
      {streaming && (
        <div className="cm ai live">
          <span className="av" />
          <div className="body">
            {streaming.text ? (
              <Markdown text={streaming.text} className="chat-md" />
            ) : (
              <p className="thinking">
                <i />
                <i />
                <i />
              </p>
            )}
            <Steps slug={slug} id={streaming.message} steps={streaming.steps} skip={skip} live />
          </div>
        </div>
      )}
      {trailing.map((m) => (
        <MessageView key={m.id} slug={slug} m={m} compact={compact} skip={skip} last={lastFor.has(m.id)} />
      ))}
    </>
  );
  if (bare) return content;
  return (
    <div className={`conv ${compact ? "compact" : ""}`} ref={box}>
      {content}
    </div>
  );
}

/** The asset a publish card is about, as `type/id`, when it names one. */
function publishedAsset(card: Card): string | null {
  if (card.approval_kind !== "publish") return null;
  const a = card.payload.asset as { type?: string; id?: string } | undefined;
  return a?.type && a?.id ? `${a.type}/${a.id}` : null;
}

function MessageView({ slug, m, compact, skip, last }: { slug: string; m: Message; compact?: boolean; skip?: Set<string>; last?: boolean }) {
  if (m.role === "user")
    return (
      <div className="cm you">
        <div className="ms y">
          {m.text}
          <span className="t">{time(m.at)}</span>
        </div>
      </div>
    );
  if (m.role === "card" && m.card) return <CardView slug={slug} card={m.card} id={m.id} compact={compact} last={last} />;
  if (m.role === "note" && m.kind === "memory")
    return (
      <div className="cm note memory" title={m.text}>
        <Brain size={13} />
        <span>
          {m.text}
          <Link to={`/p/${slug}/memory`}>Memory</Link>
        </span>
      </div>
    );
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
        <Markdown text={m.text} className="chat-md" />
        {m.error && (
          <p className="err">
            <AlertCircle size={13} /> {m.error}
          </p>
        )}
        <Steps slug={slug} id={m.id} steps={m.steps ?? []} skip={skip} />
        <span className="t">{time(m.at)}</span>
      </div>
    </div>
  );
}

/** Which messages' steps the reader unfolded: kept across the live bubble becoming the message, so nothing folds or flickers under them. */
const OPEN_STEPS = new Set<string>();

/** The tools behind a message, folded under "n steps", last in the bubble; the same box while running (a spinner in place of the chevron) and afterwards. */
function Steps({ slug, id, steps, live, skip }: { slug: string; id: string; steps: Step[]; live?: boolean; /** Assets a publish card shows instead. */ skip?: Set<string> }) {
  const [open, setOpenState] = useState(() => OPEN_STEPS.has(id));
  const setOpen = (o: boolean) => {
    if (o) OPEN_STEPS.add(id);
    else OPEN_STEPS.delete(id);
    setOpenState(o);
  };
  if (!steps.length) return null;
  const running = steps.find((s) => s.ok === null);
  const failed = steps.filter((s) => s.ok === false).length;
  // Each asset once, however many steps touched it (made, changed, an image landed on it).
  const touched = steps.filter((s) => s.asset).map((s) => s.asset!).filter((a, i, arr) => arr.findIndex((b) => b.type === a.type && b.id === a.id) === i);
  // An image that landed on an asset shown below (or on its publish card) is shown there, not twice.
  const images = steps.filter((s) => s.image && !touched.some((a) => s.image!.includes(`/assets/${a.type}/${a.id}/`))).map((s) => s.image!);
  const made = touched.filter((a) => !skip?.has(`${a.type}/${a.id}`));
  const gallery = (images.length > 0 || made.length > 0) && (
    <>
      {made.map((a) => (
        <AssetEmbed key={`${a.type}/${a.id}`} slug={slug} type={a.type} id={a.id} />
      ))}
      {images.length > 0 && (
        <div className="step-images">
          {images.map((u) => (
            <a key={u} href={u} target="_blank" rel="noreferrer">
              <img src={u} alt="" loading="lazy" />
            </a>
          ))}
        </div>
      )}
    </>
  );
  const working = !!live && !!running;
  // The count, not the running step's name: a label that changes under an unfolded list makes it jump.
  const summary = steps.length === 1 && !working ? steps[0].label : `${steps.length} ${steps.length === 1 ? "step" : "steps"}${failed ? `, ${failed} failed` : ""}`;
  return (
    <>
      {gallery}
      <div className={`steps ${open ? "open" : ""} ${working ? "working" : ""}`}>
        <button className="steps-sum" onClick={() => setOpen(!open)} aria-expanded={open} title={working ? `${running.label}…` : undefined}>
          {working ? <Loader2 size={13} className="spin" /> : open ? <ChevronDown size={13} /> : <ChevronRight size={13} />}
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
      </div>
    </>
  );
}

// ------------------------------------------------------------------ cards

/** Where a publish goes and through whom, as the card carries it (`target`). */
type PublishTarget = { channel?: string | null; label?: string; kind?: string; ownership?: string; account?: string; account_label?: string; new?: boolean };
/** Where something happens through an action, when it names a spot (`spot`). */
type ActionSpot = { kind?: string; kind_label?: string; label?: string; channel?: string | null; new?: boolean };

/** Where a publish goes, in words: "To r/homelab as u/acme_dev", "Send to Newsletter (412) from hello@kiqr.dev". */
export function publishWhere(p: Record<string, unknown>): string {
  const t = (p.target ?? {}) as PublishTarget;
  const via = t.account_label && t.account_label !== t.label ? ` as ${t.account_label}` : "";
  if (p.recipients != null) return `Send to ${t.label ?? "the list"} (${String(p.recipients)})${t.account_label ? ` from ${t.account_label}` : ""}`;
  return `To ${t.label ?? ""}${via}`;
}

/** The note on a card whose yes makes somewhere new a channel. */
function NewSpotNote() {
  return <p className="new-spot sm">Not one of your channels yet; approving adds it.</p>;
}

/** Who an email goes to, in words: "Send to Newsletter (412) from hello@kiqr.dev", or to an address. */
export function emailWhere(p: Record<string, unknown>): string {
  const from = p.from_label ? ` from ${String(p.from_label)}` : "";
  if (p.list_label) return `Send to ${String(p.list_label)}${p.recipients != null ? ` (${String(p.recipients)})` : ""}${from}`;
  return `Send to ${String(p.to ?? "")}${from}`;
}

function CardView({ slug, card, id, compact, last }: { slug: string; card: Card; id: string; compact?: boolean; last?: boolean }) {
  if (card.kind === "approval" && card.status === "pending") return <ApprovalCard slug={slug} card={card} compact={compact} />;
  if (card.approval_kind === "task" && card.task) return <TaskCard slug={slug} taskId={card.task} compact={compact} />;
  if (card.approval_kind === "task") return <Resolved kind="task" status={card.status} title={String(card.payload.title ?? "Task")} />;
  if (card.approval_kind === "ability") return <Resolved kind="ability" status={card.status} title={String(card.payload.name ?? "Ability")} />;
  if (card.approval_kind === "list") return <Resolved kind="list" status={card.status} title={String(card.payload.label ?? "List")} />;
  if (card.approval_kind === "channel") return <Resolved kind="channel" status={card.status} title={String(card.payload.label || card.payload.domain || "Channel")} />;
  if (card.approval_kind === "plan") return <Resolved kind="plan" status={card.status} title={`${String(card.payload.period ?? "Plan")}${card.status === "approved" && card.result?.version ? ` · v${String(card.result.version)}` : ""}`} />;
  if (card.approval_kind === "campaign") return <Resolved kind="campaign" status={card.status} title={String(card.payload.title ?? "Campaign")} />;
  if (last && publishedAsset(card)) return <PublishCard slug={slug} card={card} compact={compact} />;
  if (card.approval_kind === "publish" || card.approval_kind === "email") {
    const p = card.payload as Record<string, unknown>;
    const title = card.approval_kind === "email" ? `“${String(p.subject ?? "")}” · ${emailWhere(p)}` : `${String((p.asset as { title?: string })?.title ?? "Asset")} · ${publishWhere(p)}`;
    const url = card.result?.url as string | undefined;
    return (
      <div className={`ccard ${card.status}`}>
        <p className="cap">
          <Send size={13} /> {publishStatus(card)}
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
  if (card.approval_kind === "action") {
    const p = card.payload as Record<string, unknown>;
    const url = card.result?.url as string | undefined;
    return (
      <div className={`ccard ${card.status}`} data-id={id}>
        <p className="cap">
          <Zap size={13} /> {card.status === "approved" ? (p.auto ? `Done by Seemo with ${String(p.ability_name ?? "")}` : `Done with ${String(p.ability_name ?? "")}`) : card.status === "failed" ? `Not done: ${String(card.result?.error ?? "")}` : `${String(p.ability_name ?? "Action")} ${card.status}`}
        </p>
        <p className="ttl">
          {String(p.label ?? p.tool ?? "")}
          {(p.spot as ActionSpot | null)?.label ? <span className="muted"> · in {String((p.spot as ActionSpot).label)}</span> : !!p.target && <span className="muted"> · {String(p.target)}</span>}
        </p>
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
  if (card.approval_kind === "api")
    return (
      <div className={`ccard ${card.status}`} data-id={id}>
        <p className="cap">
          <Settings2 size={13} /> {card.status === "approved" ? "Changed" : card.status === "failed" ? `Did not work: ${String(card.result?.error ?? "")}` : `Change ${card.status}`}
        </p>
        <p className="ttl">{String(card.payload.summary ?? "")}</p>
      </div>
    );
  if (card.approval_kind === "channels")
    return (
      <div className={`ccard ${card.status}`} data-id={id}>
        <p className="cap">
          <Radio size={13} /> Channels {card.status === "approved" ? "changed" : card.status}
        </p>
        <ChannelChanges p={card.payload} />
      </div>
    );
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
        {card.status === "approved" && (card.result?.restart as boolean) && <p className="warn">The address changed: KIQR restarts there when this turn ends.</p>}
      </div>
    );
  }
  return null;
}

function Resolved({ kind, status, title }: { kind: string; status: string; title: string }) {
  const noun = kind === "task" ? "Task" : kind === "ability" ? "Ability" : kind === "list" ? "List" : kind === "channel" ? "Channel" : kind === "plan" ? "Plan" : kind === "campaign" ? "Campaign" : "Change";
  return (
    <div className={`ccard ${status}`}>
      <p className="cap">
        {kind === "ability" ? <BookOpen size={13} /> : kind === "list" ? <Users size={13} /> : kind === "plan" || kind === "campaign" ? <Target size={13} /> : <ListChecks size={13} />} {noun} {status === "approved" && (kind === "ability" || kind === "list" || kind === "channel" || kind === "campaign") ? "added" : status}
      </p>
      <p className="ttl">{title}</p>
    </div>
  );
}

type ChannelChange = { channel: string; label: string; diff: { key: string; from: string; to: string }[] };
const CHANNEL_KEYS: Record<string, string> = { publisher: "Posts as", publish: "Publishing", reply: "Replying", read: "Reading", learn: "Learning", label: "Name", description: "Description", url: "Link", ownership: "Ownership", ability: "Published through", fields: "Where", added: "Status" };
const MODE_WORDS: Record<string, string> = { off: "off", ask: "asks first", auto: "on its own", true: "on", false: "off" };

/** Seemo's channel changes: per channel, what changes, before → after. */
function ChannelChanges({ p }: { p: Record<string, unknown> }) {
  const list = (p.changes as ChannelChange[] | undefined) ?? [];
  const say = (key: string, v: string) => (["publish", "reply", "read", "learn"].includes(key) ? MODE_WORDS[v] ?? v : v || "none");
  return (
    <>
      {list.map((c) => (
        <div key={c.channel} className="ch-change">
          <p className="ttl">{c.label}</p>
          <ul className="kvs">
            {c.diff.map((d) => (
              <li key={d.key}>
                <code>{CHANNEL_KEYS[d.key] ?? d.key}</code> <span className="from">{say(d.key, d.from)}</span> <span className="to">{say(d.key, d.to)}</span>
              </li>
            ))}
          </ul>
        </div>
      ))}
      {!!p.reason && <p className="muted sm">{String(p.reason)}</p>}
    </>
  );
}

/** How a publish or email card ended, in words. */
function publishStatus(card: Card): string {
  const p = card.payload as Record<string, unknown>;
  if (card.status === "approved") return card.result?.sending ? `Sending to ${String(card.result?.recipients ?? "")} people` : card.approval_kind === "email" ? "Sent" : card.result?.scheduled ? "Scheduled" : card.result?.publishing ? "Publishing in the browser; Seemo says here when it is up" : p.auto ? "Published by Seemo" : "Published";
  if (card.status === "failed") return `Publish failed: ${String(card.result?.error ?? "")}`;
  return `${card.approval_kind === "email" ? "Email" : "Publish"} ${card.status}`;
}

/** A publish of an asset: one card, the asset as it will look, with the
 *  proposal (where, as whom, when) and the yes in its footer while it waits,
 *  and how it went once answered. */
function PublishCard({ slug, card, compact }: { slug: string; card: Card; compact?: boolean }) {
  const [busy, setBusy] = useState<"approve" | "decline" | null>(null);
  const [error, setError] = useState<string | null>(null);
  const p = card.payload as Record<string, unknown>;
  const a = p.asset as { type: string; id: string };
  const pending = card.kind === "approval" && card.status === "pending";
  const scheduled = typeof p.at === "string" && p.at ? String(p.at) : "";
  const url = card.result?.url as string | undefined;
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
  const options = Object.entries((p.options as Record<string, unknown>) ?? {});
  const ask = (
    <div className="pub-ask">
      <p className="cap">
        <Send size={13} /> {scheduled ? "Seemo wants to schedule this" : "Seemo wants to publish this"}
      </p>
      <p className="sched">
        <b>{publishWhere(p)}</b>
        {scheduled && <span className="muted"> · at {when(scheduled)}</span>}
      </p>
      {(p.target as PublishTarget | undefined)?.new && <NewSpotNote />}
      {options.length > 0 && (
        <p className="muted sm">
          {options.map(([k, v]) => `${k.replace(/_/g, " ")}: ${Array.isArray(v) ? v.map((x) => (x && typeof x === "object" ? String((x as { path?: unknown }).path ?? JSON.stringify(x)) : String(x))).join(", ") : String(v)}`).join(" · ")}
        </p>
      )}
      {!!p.note && <p className="muted sm">{String(p.note)}</p>}
      {error && (
        <p className="err">
          <AlertCircle size={13} /> {error}
        </p>
      )}
      <div className="row">
        <button className="btn p sm" disabled={!!busy} onClick={() => void decide("approve")}>
          {busy === "approve" ? <Loader2 size={14} className="spin" /> : <Check size={14} />} {scheduled ? "Schedule it" : "Publish it"}
        </button>
        <button className="btn g sm" disabled={!!busy} onClick={() => void decide("decline")}>
          {busy === "decline" ? <Loader2 size={14} className="spin" /> : <X size={14} />} Decline
        </button>
      </div>
    </div>
  );
  return (
    <div className={`pub-card ${card.status}`}>
      {!pending && (
        <p className="pub-status">
          <Send size={13} /> {publishStatus(card)}
          {url && (
            <a href={url} target="_blank" rel="noreferrer">
              open <ExternalLink size={11} />
            </a>
          )}
        </p>
      )}
      <AssetEmbed slug={slug} type={a.type} id={a.id} compact={compact ?? true} approval={pending ? ask : undefined} />
    </div>
  );
}

/** Seemo wants a yes: a task to create, or settings to change. */
export function ApprovalCard({ slug, card, compact }: { slug: string; card: Card; compact?: boolean }) {
  if (publishedAsset(card)) return <PublishCard slug={slug} card={card} compact={compact} />;
  return <ApprovalBox slug={slug} card={card} compact={compact} />;
}

function ApprovalBox({ slug, card, compact }: { slug: string; card: Card; compact?: boolean }) {
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
  const isAbility = card.approval_kind === "ability";
  const isPublish = card.approval_kind === "publish";
  const isEmail = card.approval_kind === "email";
  const isPlan = card.approval_kind === "plan";
  const isCampaign = card.approval_kind === "campaign";
  const isAction = card.approval_kind === "action";
  const isChannel = card.approval_kind === "channel";
  const isChannels = card.approval_kind === "channels";
  const isApi = card.approval_kind === "api";
  const isList = card.approval_kind === "list" || (isChannel && card.payload.kind === "list");
  const objectives = (p.objectives as { id: string; title: string; why?: string; kpi_label?: string; kpi?: string; target?: number | null; unit?: string; by?: string | null }[] | undefined) ?? [];
  const replaces = p.replaces as { version: number; period: string } | null | undefined;
  const next = ((p.next as string[]) ?? []).slice(0, 3);
  const instruction = String(p.instruction ?? "");
  const abilityText = String(p.text ?? "");
  const files = (p.files as { name: string; bytes: number }[] | undefined) ?? [];
  const scheduled = isPublish && typeof p.at === "string" && p.at;
  const caption = isAction ? `Seemo wants to use ${String(p.ability_name ?? "an ability")}` : isTask ? "Seemo proposes a task" : isAbility ? "Seemo wants to learn something" : isPublish ? (scheduled ? "Seemo wants to schedule a publish" : "Seemo wants to publish") : isEmail ? "Seemo wants to send an email" : isList ? `Seemo proposes a list: ${String(p.label ?? "")}` : isChannel ? `Seemo proposes a channel: ${String(p.label || p.domain || "")}` : isPlan ? (replaces ? "Seemo proposes a revised plan" : "Seemo proposes a marketing plan") : isCampaign ? (p.update ? "Seemo proposes a campaign change" : "Seemo proposes a campaign") : isChannels ? "Seemo proposes channel changes" : isApi ? "Seemo wants to change KIQR" : "Seemo proposes a settings change";
  const yes = isAction ? "Do it" : isTask ? "Approve and run" : isAbility ? "Teach it" : isPublish ? (scheduled ? "Schedule it" : "Publish it") : isEmail ? "Send it" : isList ? "Make the list" : isChannel ? "Add the channel" : isPlan ? "Approve the plan" : isCampaign ? (p.update ? "Apply" : "Add the campaign") : "Apply";
  const no = "Decline";
  return (
    <div className="ccard pending">
      <p className="cap">
        {isAction ? <Zap size={13} /> : isTask ? <ListChecks size={13} /> : isAbility ? <BookOpen size={13} /> : isList ? <Users size={13} /> : isChannel || isChannels ? <Radio size={13} /> : isPublish || isEmail ? <Send size={13} /> : isPlan || isCampaign ? <Target size={13} /> : <Settings2 size={13} />} {caption}
      </p>
      {isAction ? (
        <>
          <p className="ttl">{String(p.label ?? p.tool ?? "")}</p>
          <p className="sched">
            {(p.spot as ActionSpot | null)?.label ? (
              <>
                In <b>{String((p.spot as ActionSpot).label)}</b> via <b>{String(p.account ?? "")}</b>
              </>
            ) : (
              <>
                Via <b>{String(p.account ?? "")}</b>
                {!!p.target && <span className="muted"> · on {String(p.target)}</span>}
              </>
            )}
          </p>
          {(p.spot as ActionSpot | null)?.new && <NewSpotNote />}
          <ul className="kvs">
            {((p.fields as { key: string; value: string }[]) ?? []).map((f) => (
              <li key={f.key}>
                <code>{f.key}</code> <span className={`to ${more ? "" : "clip"}`}>{f.value}</span>
              </li>
            ))}
          </ul>
          {((p.fields as { value: string }[]) ?? []).some((f) => f.value.length > 160) && !compact && (
            <button className="btn t sm" onClick={() => setMore((m) => !m)}>
              {more ? "Less" : "Read it all"}
            </button>
          )}
          {!!p.description && <p className="muted sm">{String(p.description)}</p>}
        </>
      ) : isPlan ? (
        <>
          <p className="ttl">{String(p.period ?? "")}</p>
          {!!p.summary && <p className="sched">{String(p.summary)}</p>}
          <ul className="objs">
            {objectives.map((o) => (
              <li key={o.id}>
                <Target size={12} />
                <div>
                  <b>{o.title}</b>
                  <span>
                    {o.kpi_label ?? o.kpi}
                    {o.target != null ? ` · target ${o.target}${o.unit ? ` ${o.unit}` : ""}` : ""}
                    {o.by ? ` · by ${new Date(o.by).toLocaleDateString([], { day: "numeric", month: "short" })}` : ""}
                  </span>
                  {o.why && <small>{o.why}</small>}
                </div>
              </li>
            ))}
          </ul>
          {((p.channels as string[]) ?? []).length > 0 && <p className="muted sm">Where it plays out: {(p.channels as string[]).join(", ")}</p>}
          {replaces && <p className="muted sm">Replaces v{replaces.version} ({replaces.period}){p.reason ? `: ${String(p.reason)}` : ""}.</p>}
        </>
      ) : isCampaign ? (
        <>
          <p className="ttl">{String(p.title ?? "")}</p>
          {!!p.goal && <p className="sched">{String(p.goal)}</p>}
          {p.update ? (
            <ul className="kvs">
              {((p.changes as { key: string; from: unknown; to: unknown }[]) ?? []).map((c) => (
                <li key={c.key}>
                  <code>{c.key}</code> <span className="from">{String(c.from ?? "")}</span> <span className="to">{String(c.to ?? "")}</span>
                </li>
              ))}
            </ul>
          ) : (
            <p className="muted sm">
              {p.objective_title ? `For: ${String(p.objective_title)}` : "No objective"}
              {((p.channels as string[]) ?? []).length ? ` · ${(p.channels as string[]).join(", ")}` : ""}
              {p.start || p.end ? ` · ${[p.start, p.end].filter(Boolean).join(" to ")}` : ""}
            </p>
          )}
        </>
      ) : isPublish || isEmail ? (
        <>
          <p className="ttl">{isEmail ? String(p.subject ?? "") : String((p.asset as { title?: string })?.title ?? "")}</p>
          <p className="sched">
            <b>{isEmail ? emailWhere(p) : publishWhere(p)}</b>
            {scheduled && <span className="muted"> · at {when(String(p.at))}</span>}
          </p>
          {isPublish && (p.target as PublishTarget | undefined)?.new && <NewSpotNote />}
          {!!p.excerpt && <p className={`instr ${more ? "open" : ""}`}>{String(p.excerpt)}</p>}
          {String(p.excerpt ?? "").length > 220 && !compact && (
            <button className="btn t sm" onClick={() => setMore((m) => !m)}>
              {more ? "Less" : "Read it all"}
            </button>
          )}
          {files.length > 0 && <p className="muted sm">Attached: {files.map((f) => f.name).join(", ")}</p>}
          {Object.keys((p.options as Record<string, unknown>) ?? {}).length > 0 && (
            <p className="muted sm">
              {Object.entries(p.options as Record<string, unknown>)
                .map(([k, v]) => `${k.replace(/_/g, " ")}: ${Array.isArray(v) ? v.map((x) => (x && typeof x === "object" ? String((x as { path?: unknown }).path ?? JSON.stringify(x)) : String(x))).join(", ") : String(v)}`)
                .join(" · ")}
            </p>
          )}
          {!!p.note && <p className="muted sm">{String(p.note)}</p>}
        </>
      ) : isChannels ? (
        <ChannelChanges p={p} />
      ) : isApi ? (
        <>
          <p className="ttl">{String(p.summary ?? "")}</p>
          <p className="sched">
            <code>
              {String(p.method)} {String(p.path)}
            </code>
          </p>
          {Object.keys((p.body as Record<string, unknown>) ?? {}).length > 0 && <pre className={`instr api-body ${more ? "open" : ""}`}>{JSON.stringify(p.body, null, 2)}</pre>}
          {JSON.stringify(p.body ?? {}).length > 220 && !compact && (
            <button className="btn t sm" onClick={() => setMore((m) => !m)}>
              {more ? "Less" : "Show all of it"}
            </button>
          )}
        </>
      ) : isChannel && !isList ? (
        <>
          <p className="ttl">{String(p.label || p.domain || "")}</p>
          {!!p.url && (
            <p className="sched">
              <a href={String(p.url)} target="_blank" rel="noreferrer">
                {String(p.url).replace(/^https?:\/\//, "")}
              </a>
            </p>
          )}
          {!!p.description && <p className="muted sm">{String(p.description)}</p>}
          <p className="muted sm">Seemo asks before it publishes there until you say otherwise on the channel.</p>
        </>
      ) : isList ? (
        <>
          <p className="ttl">{String(p.label ?? "")}</p>
          {!!p.description && <p className="sched">{String(p.description)}</p>}
          <p className="muted sm">It starts empty: you add people under Channels, and Seemo never sees their addresses.</p>
        </>
      ) : isAbility ? (
        <>
          <p className="ttl">{String(p.name)}</p>
          <p className="sched">{String(p.description ?? "")}</p>
          <p className={`instr ${more ? "open" : ""}`}>{abilityText}</p>
          {abilityText.length > 220 && !compact && (
            <button className="btn t sm" onClick={() => setMore((m) => !m)}>
              {more ? "Less" : "Read all of it"}
            </button>
          )}
          <p className="muted sm">Becomes an ability for {String(p.scope_text ?? "this project")}; Seemo reads these instructions before matching requests.</p>
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
          {(p.restart as boolean) && <p className="warn">Address change: KIQR restarts on it when this turn ends.</p>}
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
          {no}
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
