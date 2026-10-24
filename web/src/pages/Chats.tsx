// Chats with Seemo: every conversation of the project on the left, the open
// one on the right, live. The floating widget shows the same chat elsewhere.
import { useEffect, useState } from "react";
import { NavLink, useNavigate, useParams } from "react-router-dom";
import { Check, Pencil, Plus, Trash2, X } from "lucide-react";
import { api, useAsync, useDemoSeemo, useEvents, type Chat } from "../api";
import { Composer, Conversation, ago } from "../chat/Conversation";
import { activeChatKey, useChat } from "../useChat";
import "../chat.css";

const SUGGESTIONS = ["What do you know about us, in a few lines?", "What should we focus on this quarter?", "Write three social media posts for the library", "Every Monday at 09:00, draft a LinkedIn post"];

export function ChatsPage() {
  const { slug = "", id } = useParams();
  const navigate = useNavigate();
  const list = useAsync(() => api.chats(slug), [slug]);
  const chat = useChat(slug, id ?? null);
  const [live, setLive] = useState<Set<string>>(new Set());
  const onEvent = (e: { type: string; chat?: unknown }) => {
    const c = e.chat as string | undefined;
    if (["chat_created", "chat_renamed", "chat_deleted", "chat_turn_finished"].includes(e.type)) list.reload();
    if (e.type === "chat_turn_started" && c) setLive((s) => new Set(s).add(c));
    if (e.type === "chat_turn_finished" && c)
      setLive((s) => {
        const n = new Set(s);
        n.delete(c);
        return n;
      });
    if (e.type === "chat_deleted" && c === id) navigate(`/p/${slug}/chats`);
  };
  useEvents(slug, onEvent, { demo: false });
  useDemoSeemo(onEvent);
  // The widget follows the chat open here.
  useEffect(() => {
    if (!id) return;
    try {
      localStorage.setItem(activeChatKey(slug), id);
    } catch {
      /* private window */
    }
  }, [slug, id]);

  const startChat = async (text: string) => {
    const c = await api.createChat(slug);
    await api.sendMessage(slug, c.id, text);
    navigate(`/p/${slug}/chats/${c.id}`);
  };
  const newChat = () => navigate(`/p/${slug}/chats`);

  return (
    <div className="chat-page">
      <aside className="chat-list" aria-label="Chats">
        <button className="btn p" onClick={newChat}>
          <Plus size={15} /> New chat
        </button>
        <div className="items">
          {list.data?.length === 0 && <p className="empty muted">No chats yet. Ask Seemo something.</p>}
          {list.data?.map((c) => (
            <NavLink key={c.id} to={`/p/${slug}/chats/${c.id}`} title={c.title}>
              {live.has(c.id) && <i className="live-dot" />}
              <span className="nm">{c.title}</span>
              <span className="ago">{ago(c.updated_at)}</span>
              {c.last_message && <span className="lm">{c.last_message}</span>}
            </NavLink>
          ))}
        </div>
      </aside>
      <section className="chat-main">
        {id && chat.chat ? (
          <>
            <Head slug={slug} chat={chat.chat} onDeleted={() => navigate(`/p/${slug}/chats`)} />
            <Conversation slug={slug} messages={chat.messages} streaming={chat.streaming} />
            {chat.error && <p className="err" style={{ padding: "0 24px", color: "var(--bad)", fontSize: 13 }}>{chat.error}</p>}
            <Composer onSend={chat.send} onStop={() => void chat.stop()} busy={chat.busy} autoFocus />
          </>
        ) : id ? (
          <div className="chat-empty">{chat.error ? <p>{chat.error}</p> : <p className="muted">Loading…</p>}</div>
        ) : (
          <>
            <div className="chat-empty">
              <span className="av" />
              <h2>Ask Seemo</h2>
              <p>It has read your website and the business profile. Ask about strategy, have it write for the library, put work on a schedule, or change how KIQR runs.</p>
              <div className="sugg">
                {SUGGESTIONS.map((s) => (
                  <button key={s} onClick={() => void startChat(s)}>
                    {s}
                  </button>
                ))}
              </div>
            </div>
            <Composer onSend={startChat} autoFocus placeholder="Ask Seemo anything about your marketing" />
          </>
        )}
      </section>
    </div>
  );
}

function Head({ slug, chat, onDeleted }: { slug: string; chat: Chat; onDeleted: () => void }) {
  const [editing, setEditing] = useState(false);
  const [title, setTitle] = useState(chat.title);
  const [confirm, setConfirm] = useState(false);
  useEffect(() => setTitle(chat.title), [chat.title]);
  const save = async () => {
    const t = title.trim();
    setEditing(false);
    if (t && t !== chat.title) await api.renameChat(slug, chat.id, t).catch(() => setTitle(chat.title));
  };
  const remove = async () => {
    await api.deleteChat(slug, chat.id);
    onDeleted();
  };
  return (
    <div className="chat-head">
      <span className="av" style={{ width: 26, height: 26, borderRadius: 8 }} />
      {editing ? (
        <input value={title} autoFocus onChange={(e) => setTitle(e.target.value)} onBlur={() => void save()} onKeyDown={(e) => (e.key === "Enter" ? void save() : e.key === "Escape" ? setEditing(false) : null)} />
      ) : (
        <h1 title={chat.title} onDoubleClick={() => setEditing(true)}>
          {chat.title}
        </h1>
      )}
      {confirm ? (
        <>
          <span className="muted" style={{ fontSize: 13 }}>
            Delete this chat?
          </span>
          <button className="btn t bad" onClick={() => void remove()} aria-label="Yes, delete">
            <Check size={15} />
          </button>
          <button className="btn t" onClick={() => setConfirm(false)} aria-label="Keep it">
            <X size={15} />
          </button>
        </>
      ) : (
        <>
          <button className="btn t" onClick={() => setEditing(true)} aria-label="Rename" title="Rename">
            <Pencil size={15} />
          </button>
          <button className="btn t" onClick={() => setConfirm(true)} aria-label="Delete" title="Delete">
            <Trash2 size={15} />
          </button>
        </>
      )}
    </div>
  );
}
