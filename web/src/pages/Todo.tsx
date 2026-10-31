// Seemo → To do: what waits for you, in one place. Seemo's open questions
// from every chat (answered here or there), drafts to approve, what is
// approved but not sent, publishes coming up, and scheduled publishes that
// failed. The sidebar count is the approvals, drafts and failures.
import { Link, useParams } from "react-router-dom";
import { CalendarClock, CheckCircle2, Download, ExternalLink, MessagesSquare, Send, Sparkles } from "lucide-react";
import { useState } from "react";
import { api, useAsync, useEvents, useDemoSeemo, type Asset, type Card, type Suggestion } from "../api";
import { Guide } from "./Guide";
import { ApprovalCard } from "../chat/Conversation";
import { AssetEmbed } from "./AssetEmbed";
import { AssetActions, StatusChip, whenAt } from "./AssetActions";
import { assetIcon } from "../assetIcons";
import "../profile.css";
import "../chat.css";
import "../library.css";

export const TODO_EVENTS = ["suggestions_changed", "brand_updated", "extension_installed", "extension_removed", "asset_saved", "asset_deleted", "asset_published", "asset_approved", "asset_unpublished", "approval_requested", "approval_resolved", "chat_deleted"];

export function TodoPage() {
  const { slug = "" } = useParams();
  const todo = useAsync(() => api.todo(slug), [slug]);
  const types = useAsync(() => api.assetTypes(), []);
  const onEvent = (e: { type: string }) => TODO_EVENTS.includes(e.type) && todo.reload();
  useEvents(slug, onEvent, { demo: false });
  useDemoSeemo(onEvent);
  const d = todo.data;
  // The getting-started steps sit here until every one is done or skipped; the page is "Getting started" until then.
  const [guideDone, setGuideDone] = useState<boolean | undefined>(undefined);
  const starting = guideDone === false;
  const empty = d && guideDone !== false && d.approvals.length === 0 && d.drafts.length === 0 && d.ready.length === 0 && d.scheduled.length === 0 && d.attention.length === 0;
  return (
    <div className="prof todo">
      <div className="cm-head">
        <div>
          <p className="cap">For you</p>
          <h1 className="d2">{starting ? "Getting started" : "To do"}</h1>
          <p className="muted">{starting ? "A few steps from your website to marketing that runs itself. Once they are done, this page is what waits for you: Seemo's questions, drafts to approve, and what is set to go out." : "What waits for you. Seemo's questions from every chat, drafts to approve, and what is set to go out. Approve a draft for a channel; publish it now or pick a time."}</p>
        </div>
        {d && d.count > 0 && (
          <div className="cm-acts">
            <span className="chip acc">{d.count} waiting</span>
          </div>
        )}
      </div>

      <Guide slug={slug} hideWhenComplete onComplete={setGuideDone} />

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
        <section className="todo-sec" id="todo-drafts">
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

      {d && (d.suggestions?.length ?? 0) > 0 && (
        <section className="todo-sec">
          <h2>
            <Sparkles size={15} /> Seemo suggests <span className="n">{d.suggestions!.length}</span>
          </h2>
          <ul className="todo-rows">
            {d.suggestions!.map((s) => (
              <SuggestionRow key={s.kind} slug={slug} s={s} onChange={todo.reload} />
            ))}
          </ul>
        </section>
      )}
    </div>
  );
}

/** A profile the business has where an ability in the marketplace could publish. */
function SuggestionRow({ slug, s, onChange }: { slug: string; s: Suggestion; onChange: () => void }) {
  const [busy, setBusy] = useState(false);
  return (
    <li className="todo-row sugg">
      <span className="what">
        <span className="ic">
          <Sparkles size={15} />
        </span>
        <span className="min0">
          <b>
            I found your{" "}
            <a href={s.profile_url} target="_blank" rel="noreferrer">
              {s.name} page <ExternalLink size={11} />
            </a>
          </b>
          <small>The marketplace has “{s.ability_name}”. With it I can write and publish there.</small>
        </span>
      </span>
      <Link className="btn p sm" to={`/p/${slug}/abilities#ability=${encodeURIComponent(s.ability)}`}>
        <Download size={13} /> Install
      </Link>
      <button className="btn t sm" disabled={busy} onClick={() => (setBusy(true), api.dismissSuggestion(slug, s.kind).then(onChange, () => setBusy(false)))}>
        Not now
      </button>
    </li>
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
