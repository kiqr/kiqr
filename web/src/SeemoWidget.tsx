import { useEffect, useRef, useState } from "react";
import { useEvents } from "./api";

type Msg = { from: "seemo" | "you"; text: string; at: Date };

export function SeemoWidget({ project }: { project: string | null }) {
  const [open, setOpen] = useState(false);
  const [msgs, setMsgs] = useState<Msg[]>([]);
  const [unread, setUnread] = useState(0);
  const end = useRef<HTMLDivElement>(null);

  useEvents(project, (e) => {
    if (e.type === "seemo_says") {
      setMsgs((m) => [...m, { from: "seemo", text: e.text as string, at: new Date(e.at) }]);
      if (!open) setUnread((n) => n + 1);
    }
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
        className="seemo-launcher"
        aria-label="Open Seemo"
        onClick={() => {
          setOpen(true);
          setUnread(0);
        }}
      >
        {unread > 0 ? unread : "s"}
      </button>
    );
  }

  return (
    <div className="seemo" role="dialog" aria-label="Seemo">
      <div className="seemo-top">
        <span className="avatar">s</span>
        <div>
          <div className="who">Seemo</div>
          <div className="ctx">{project ?? "No project selected"}</div>
        </div>
        <button className="close" aria-label="Close" onClick={() => setOpen(false)}>
          ×
        </button>
      </div>
      <div className="seemo-msgs">
        {msgs.length === 0 && (
          <div className="m seemo-m body">
            {project ? "I'll tell you what I find as I read the site." : "Add a project and I'll read its website."}
          </div>
        )}
        {msgs.map((m, i) => (
          <div key={i} className={`m ${m.from === "seemo" ? "seemo-m" : "you"} body`}>
            {m.text}
            <span className="t">{m.at.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}</span>
          </div>
        ))}
        <div ref={end} />
      </div>
      <div className="seemo-input">
        <input placeholder="Chat arrives in the next release" disabled />
        <button disabled>Send</button>
      </div>
    </div>
  );
}
