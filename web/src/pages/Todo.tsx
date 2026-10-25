// Seemo → To do: what waits for you, in one place. Seemo's open questions
// from every chat (answered here or there), drafts to approve, what is
// approved but not sent, publishes coming up, and scheduled publishes that
// failed. The sidebar count is the approvals, drafts and failures.
import { Link, useParams } from "react-router-dom";
import { CalendarClock, CheckCircle2, MessagesSquare, Send } from "lucide-react";
import { api, useAsync, useEvents, useDemoSeemo, type Asset, type Card } from "../api";
import { ApprovalCard } from "../chat/Conversation";
import { AssetEmbed } from "./AssetEmbed";
import { AssetActions, StatusChip, whenAt } from "./AssetActions";
import { assetIcon } from "../assetIcons";
import "../profile.css";
import "../chat.css";
import "../library.css";

export const TODO_EVENTS = ["asset_saved", "asset_deleted", "asset_published", "asset_approved", "asset_unpublished", "approval_requested", "approval_resolved", "chat_deleted"];

export function TodoPage() {
  const { slug = "" } = useParams();
  const todo = useAsync(() => api.todo(slug), [slug]);
  const types = useAsync(() => api.assetTypes(), []);
  const onEvent = (e: { type: string }) => TODO_EVENTS.includes(e.type) && todo.reload();
  useEvents(slug, onEvent, { demo: false });
  useDemoSeemo(onEvent);
  const d = todo.data;
  const empty = d && d.approvals.length === 0 && d.drafts.length === 0 && d.ready.length === 0 && d.scheduled.length === 0 && d.attention.length === 0;
  return (
    <div className="prof todo">
      <div className="cm-head">
        <div>
          <p className="cap">Seemo</p>
          <h1 className="d2">To do</h1>
          <p className="muted">What waits for you. Seemo's questions from every chat, drafts to approve, and what is set to go out. Approve a draft for a channel; publish it now or pick a time.</p>
        </div>
        {d && d.count > 0 && (
          <div className="cm-acts">
            <span className="chip acc">{d.count} waiting</span>
          </div>
        )}
      </div>

      {empty && (
        <div className="pcard none">
          <CheckCircle2 size={22} className="ac2" />
          <p className="t">Nothing waiting</p>
          <p className="muted">Seemo's drafts and questions land here. Ask it for something in a chat, or let a task run.</p>
          <Link className="btn g" to={`/p/${slug}/chats`}>
            <MessagesSquare size={14} /> Open chats
          </Link>
        </div>
      )}

      {d && d.approvals.length > 0 && (
        <section className="todo-sec">
          <h2>
            Waiting for your yes <span className="n">{d.approvals.length}</span>
          </h2>
          {d.approvals.map((x) => {
            const card: Card = { kind: "approval", approval: x.id, approval_kind: x.kind, status: "pending", payload: x.payload };
            return (
              <div className="todo-approval" key={x.id}>
                <ApprovalCard slug={slug} card={card} />
                {x.chat && (
                  <Link className="where" to={`/p/${slug}/chats/${x.chat}`}>
                    <MessagesSquare size={12} /> In the chat “{x.chat_title ?? "Chat"}”
                  </Link>
                )}
              </div>
            );
          })}
        </section>
      )}

      {d && d.attention.length > 0 && (
        <section className="todo-sec">
          <h2>
            Needs a hand <span className="n">{d.attention.length}</span>
          </h2>
          <ul className="todo-rows">
            {d.attention.map((a) => (
              <Row key={`${a.type}/${a.id}`} slug={slug} a={a} icon={types.data?.find((t) => t.id === a.type)?.icon} onChange={todo.reload} />
            ))}
          </ul>
        </section>
      )}

      {d && d.drafts.length > 0 && (
        <section className="todo-sec">
          <h2>
            Drafts to approve <span className="n">{d.drafts.length}</span>
          </h2>
          <div className="todo-drafts">
            {d.drafts.map((a) => (
              <AssetEmbed key={`${a.type}/${a.id}`} slug={slug} type={a.type} id={a.id} given={a} compact={false} onChange={todo.reload} />
            ))}
          </div>
        </section>
      )}

      {d && d.ready.length > 0 && (
        <section className="todo-sec">
          <h2>
            Approved, not sent yet <span className="n">{d.ready.length}</span>
          </h2>
          <ul className="todo-rows">
            {d.ready.map((a) => (
              <Row key={`${a.type}/${a.id}`} slug={slug} a={a} icon={types.data?.find((t) => t.id === a.type)?.icon} onChange={todo.reload} />
            ))}
          </ul>
        </section>
      )}

      {d && d.scheduled.length > 0 && (
        <section className="todo-sec">
          <h2>
            <CalendarClock size={15} /> Coming up <span className="n">{d.scheduled.length}</span>
          </h2>
          <ul className="todo-rows">
            {d.scheduled.map((a) => (
              <Row key={`${a.type}/${a.id}`} slug={slug} a={a} icon={types.data?.find((t) => t.id === a.type)?.icon} onChange={todo.reload} when={a.publish_at ?? undefined} />
            ))}
          </ul>
          <p className="muted sm">
            <Send size={12} /> The daemon publishes these on time; they are on the <Link to={`/p/${slug}/calendar`}>calendar</Link> too.
          </p>
        </section>
      )}
    </div>
  );
}

/** One asset as a row: what it is, where it stands, what you can do. */
function Row({ slug, a, icon, when, onChange }: { slug: string; a: Asset; icon?: string; when?: string; onChange: () => void }) {
  const Icon = assetIcon(icon);
  return (
    <li className="todo-row">
      <Link className="what" to={`/p/${slug}/library/${a.type}/${a.id}`}>
        <span className="ic">
          <Icon size={15} />
        </span>
        <span className="min0">
          <b>{a.title}</b>
          <small>
            {when ? whenAt(when) : a.summary ?? ""}
          </small>
        </span>
      </Link>
      {a.status !== "ready" && a.status !== "scheduled" && <StatusChip a={a} />}
      <AssetActions slug={slug} a={a} compact onChange={onChange} />
    </li>
  );
}
