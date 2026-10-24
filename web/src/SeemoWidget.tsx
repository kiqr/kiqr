// Seemo in the corner: the project's active chat, live, on every screen but
// the Chats page (which is the same chat, full size). Crawl narration shows
// here too, between the messages. Anything that starts a chat elsewhere (the
// getting-started guide) opens the widget with `openSeemo(chatId)`.
import { useEffect, useState } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import { Maximize2, Minus, Plus } from "lucide-react";
import { api, useDemoSeemo, useEvents, type KiqrEvent } from "./api";
import { Composer, Conversation } from "./chat/Conversation";
import { activeChatKey, useChat } from "./useChat";

const OPEN_EVENT = "kiqr:seemo-open";

/** Open the widget on this chat, from anywhere in the app. */
export function openSeemo(chat: string) {
  window.dispatchEvent(new CustomEvent(OPEN_EVENT, { detail: { chat } }));
}

type Narration = { text: string; at: Date };

export function SeemoWidget({ project }: { project: string | null }) {
  const [open, setOpen] = useState(false);
  const [unread, setUnread] = useState(0);
  const [crawling, setCrawling] = useState(false);
  const [active, setActive] = useState<string | null>(null);
  const [narration, setNarration] = useState<Narration[]>([]);
  const { pathname } = useLocation();
  const navigate = useNavigate();
  const onChatsPage = /^\/p\/[^/]+\/chats/.test(pathname);
  const chat = useChat(project, active);

  // Which chat to follow: the one last opened, else the newest.
  useEffect(() => {
    setNarration([]);
    setActive(null);
    if (!project) return;
    let remembered: string | null = null;
    try {
      remembered = localStorage.getItem(activeChatKey(project));
    } catch {
      /* private window */
    }
    api
      .chats(project)
      .then((list) => setActive(remembered && list.some((c) => c.id === remembered) ? remembered : (list[0]?.id ?? null)))
      .catch(() => setActive(null));
  }, [project]);

  useEffect(() => {
    const onOpen = (e: Event) => {
      const id = (e as CustomEvent<{ chat: string }>).detail?.chat;
      if (id) setActive(id);
      setOpen(true);
      setUnread(0);
    };
    window.addEventListener(OPEN_EVENT, onOpen);
    return () => window.removeEventListener(OPEN_EVENT, onOpen);
  }, []);

  const onEvent = (e: KiqrEvent) => {
    if (e.type === "seemo_says") {
      setNarration((n) => [...n.slice(-19), { text: e.text as string, at: new Date(e.at) }]);
      if (!open) setUnread((n) => n + 1);
    }
    if (e.type === "page_queued") setCrawling(true);
    if (e.type === "page_parsed") setCrawling((e.waiting as number) > 0);
    if (e.type === "crawl_finished") setCrawling(false);
    if (e.type === "chat_created") setActive(e.chat as string);
    if (e.type === "chat_deleted" && e.chat === active) setActive(null);
    if (e.type === "chat_message" && e.chat === active && !open) {
      const m = e.message as { role: string };
      if (m.role === "seemo") setUnread((n) => n + 1);
    }
  };
  useEvents(project, onEvent, { demo: false });
  useDemoSeemo(onEvent);

  if (onChatsPage || !project) return null;
  const busy = crawling || chat.busy;

  if (!open) {
    return (
      <button
        className={`seemo-launch ${busy ? "busy" : ""}`}
        aria-label="Open Seemo"
        onClick={() => {
          setOpen(true);
          setUnread(0);
        }}
      >
        {unread > 0 && <span className="n">{unread}</span>}
      </button>
    );
  }

  const send = async (text: string) => {
    if (active) return chat.send(text);
    const c = await api.createChat(project);
    setActive(c.id);
    await api.sendMessage(project, c.id, text);
  };
  const startNew = () => setActive(null);

  const trailing = narration.length > 0 && (
    <div className="narr" aria-label="From the crawl">
      {narration.slice(-3).map((n, i) => (
        <p key={i}>
          <span className="t">{n.at.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}</span>
          {n.text}
        </p>
      ))}
    </div>
  );

  return (
    <div className="seemo" role="dialog" aria-label="Seemo">
      <div className="top">
        <span className="av" />
        <div style={{ minWidth: 0 }}>
          <b>Seemo</b>
          <small title={chat.chat?.title}>{chat.chat?.title ?? `${project} · CMO`}</small>
        </div>
        <div className="acts">
          <button onClick={startNew} aria-label="New chat" title="New chat">
            <Plus size={16} />
          </button>
          <a
            href={`/p/${project}/chats${active ? `/${active}` : ""}`}
            aria-label="Open in Chats"
            title="Open in Chats"
            onClick={(e) => {
              e.preventDefault();
              navigate(`/p/${project}/chats${active ? `/${active}` : ""}`);
            }}
          >
            <Maximize2 size={15} />
          </a>
          <button aria-label="Minimize" title="Minimize" onClick={() => setOpen(false)}>
            <Minus size={16} />
          </button>
        </div>
      </div>
      {active && chat.messages.length + (chat.streaming ? 1 : 0) > 0 ? (
        <Conversation slug={project} messages={chat.messages} streaming={chat.streaming} compact trailing={trailing} />
      ) : (
        <div className="conv compact">
          <div className="cm ai">
            <span className="av" />
            <div className="body">
              <div className="chat-md">
                <p>{crawling ? "I'm reading the site now and will tell you what I find." : "Ask me about the business, have me write for the library, or put work on a schedule."}</p>
              </div>
            </div>
          </div>
          {trailing}
        </div>
      )}
      {chat.error && <p style={{ margin: "0 14px", color: "var(--bad)", fontSize: 12 }}>{chat.error}</p>}
      <Composer compact onSend={send} onStop={() => void chat.stop()} busy={chat.busy} placeholder="Ask Seemo" />
    </div>
  );
}
