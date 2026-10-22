import { useEffect, useRef, useState } from "react";
import { useEvents } from "./api";

type Msg = { from: "seemo" | "you"; text: string; at: Date };

export function SeemoWidget({ project }: { project: string | null }) {
  const [open, setOpen] = useState(false);
  const [msgs, setMsgs] = useState<Msg[]>([]);
  const [unread, setUnread] = useState(0);
  const [busy, setBusy] = useState(false);
  const end = useRef<HTMLDivElement>(null);

  useEvents(project, (e) => {
    if (e.type === "seemo_says") {
      setMsgs((m) => [...m, { from: "seemo", text: e.text as string, at: new Date(e.at) }]);
      if (!open) setUnread((n) => n + 1);
    }
    if (e.type === "page_queued") setBusy(true);
    if (e.type === "page_parsed") setBusy((e.waiting as number) > 0);
    if (e.type === "crawl_finished") setBusy(false);
  });
  useEffect(() => {
    end.current?.scrollIntoView({ block: "end" });
  }, [msgs, open]);
  useEffect(() => {
    setMsgs([]);
  }, [project]);

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

  return (
    <div className="seemo" role="dialog" aria-label="Seemo">
      <div className="top">
        <span className="av" />
        <div>
          <b>Seemo</b>
          <small>{project ? `${project} · CMO` : "No project selected"}</small>
        </div>
        <button className="x" aria-label="Close" onClick={() => setOpen(false)}>
          ×
        </button>
      </div>
      <div className="msgs">
        {msgs.length === 0 && <div className="ms s">{project ? "I'll tell you what I find as I read the site." : "Add a project and I'll read its website."}</div>}
        {msgs.map((m, i) => (
          <div key={i} className={`ms ${m.from === "seemo" ? "s" : "y"}`}>
            {m.text}
            <span className="t">{m.at.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}</span>
          </div>
        ))}
        <div ref={end} />
      </div>
      <div className="inp">
        <input placeholder="Chat arrives in the next release" disabled />
        <button disabled aria-label="Send">
          ↑
        </button>
      </div>
    </div>
  );
}
