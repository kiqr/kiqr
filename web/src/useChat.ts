// One chat, kept live: the transcript from the API plus the turn in progress
// from events. Shared by the Chats page and the floating widget, so both show
// the same conversation at the same moment.
import { useCallback, useEffect, useRef, useState } from "react";
import { api, useDemoSeemo, useEvents, type ChatView, type KiqrEvent, type Message, type Step, type Streaming } from "./api";
import { DEMO } from "./demo";

export function useChat(slug: string | null, chatId: string | null) {
  const [chat, setChat] = useState<ChatView | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [streaming, setStreaming] = useState<Streaming | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const current = useRef<string | null>(null);
  current.current = chatId;

  const reload = useCallback(() => {
    if (!slug || !chatId) {
      setChat(null);
      setMessages([]);
      setStreaming(null);
      return;
    }
    setLoading(true);
    api
      .chat(slug, chatId)
      .then((c) => {
        if (current.current !== chatId) return;
        setChat(c);
        setMessages(c.messages);
        setStreaming(c.streaming);
        setError(null);
      })
      .catch((e) => setError((e as Error).message))
      .finally(() => setLoading(false));
  }, [slug, chatId]);
  useEffect(reload, [reload]);

  const onEvent = (e: KiqrEvent) => {
    if (!chatId || e.chat !== chatId) return;
    switch (e.type) {
      case "chat_turn_started":
        setStreaming({ message: e.message as string, text: "", steps: [], started: e.at });
        break;
      case "chat_delta":
        setStreaming((s) => ({ message: s?.message ?? "", text: (s?.text ?? "") + (e.text as string), steps: s?.steps ?? [], started: s?.started ?? e.at }));
        break;
      case "chat_tool": {
        const step: Step = { tool: e.tool as string, label: e.label as string, ok: e.status === "started" ? null : e.status === "done", note: (e.note as string) || undefined, image: (e.image as string) || undefined };
        setStreaming((s) => {
          const steps = [...(s?.steps ?? [])];
          if (e.status === "started") steps.push(step);
          else {
            const i = steps.findIndex((x) => x.ok === null && x.tool === step.tool);
            if (i >= 0) steps[i] = step;
            else steps.push(step);
          }
          return { message: s?.message ?? "", text: s?.text ?? "", steps, started: s?.started ?? e.at };
        });
        break;
      }
      case "chat_message": {
        const m = e.message as Message;
        setMessages((all) => (all.some((x) => x.id === m.id) ? all.map((x) => (x.id === m.id ? m : x)) : [...all, m]));
        if (m.role === "seemo") setStreaming((s) => (s && s.message === m.id ? null : s));
        break;
      }
      case "chat_turn_finished":
        setStreaming(null);
        // Self-heal: if a frame was lost, the transcript is the record.
        if (!DEMO) reload();
        break;
      case "chat_renamed":
        setChat((c) => (c ? { ...c, title: e.title as string } : c));
        break;
    }
  };
  useEvents(slug, onEvent, { demo: false });
  useDemoSeemo(onEvent);

  const send = useCallback(
    async (text: string) => {
      if (!slug || !chatId) return;
      setError(null);
      try {
        await api.sendMessage(slug, chatId, text);
      } catch (e) {
        setError((e as Error).message);
        throw e;
      }
    },
    [slug, chatId],
  );
  const stop = useCallback(() => (slug && chatId ? api.stopChat(slug, chatId).catch(() => undefined) : Promise.resolve(undefined)), [slug, chatId]);

  return { chat, messages, streaming, busy: !!streaming, error, loading, send, stop, reload };
}

/** The chat the widget follows: the latest one, or the one just started. */
export function activeChatKey(slug: string) {
  return `kiqr.chat.active.${slug}`;
}
