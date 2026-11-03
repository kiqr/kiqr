// Seemo → Memory: the owner's rules Seemo follows in every chat and task,
// and what it learned from its own work. Everything here can be changed;
// a lesson from a task run waits for a Keep before Seemo uses it.
import { useState } from "react";
import { Link, useParams } from "react-router-dom";
import { Brain, Check, Pencil, Plus, Trash2, X } from "lucide-react";
import { api, useAsync, useDemoSeemo, useEvents, type Memory, type MemoryKind, type MemoryScope } from "../api";
import { Switch } from "./Switch";
import { ago } from "./Project";
import "../profile.css";
import "../chat.css";

const KINDS: { kind: MemoryKind; label: string; hint: string }[] = [
  { kind: "rule", label: "Rules", hint: "How things must or must not be done" },
  { kind: "preference", label: "Preferences", hint: "How you like things" },
  { kind: "fact", label: "Facts", hint: "What is true about the business that the profile does not say" },
  { kind: "lesson", label: "Lessons", hint: "What Seemo learned from how its work went" },
];
const ONE: Record<MemoryKind, string> = { rule: "Rule", preference: "Preference", fact: "Fact", lesson: "Lesson" };
// Where a memory holds, by channel ownership: promoting is right on our own profiles, not in someone else's thread.
const WHERE: { scope: MemoryScope; label: string; chip: string }[] = [
  { scope: "everywhere", label: "Everywhere", chip: "" },
  { scope: "own", label: "On our channels", chip: "On our channels" },
  { scope: "others", label: "In others' communities", chip: "In others' threads and groups" },
];

function WhereSelect({ value, onChange }: { value: MemoryScope; onChange: (s: MemoryScope) => void }) {
  return (
    <select className="input sm" value={value} onChange={(e) => onChange(e.target.value as MemoryScope)} aria-label="Where it holds">
      {WHERE.map((w) => (
        <option key={w.scope} value={w.scope}>
          {w.label}
        </option>
      ))}
    </select>
  );
}

export function MemoryPage() {
  const { slug = "" } = useParams();
  const mem = useAsync(() => api.memory(slug), [slug]);
  const onEvent = (e: { type: string }) => {
    if (e.type.startsWith("memory_")) mem.reload();
  };
  useEvents(slug, onEvent, { demo: false });
  useDemoSeemo(onEvent);
  const [text, setText] = useState("");
  const [kind, setKind] = useState<MemoryKind>("rule");
  const [scope, setScope] = useState<MemoryScope>("everywhere");
  const [err, setErr] = useState<string | null>(null);
  const items = mem.data?.items ?? [];
  const pending = items.filter((m) => m.pending);
  const run = async (fn: () => Promise<unknown>) => {
    setErr(null);
    try {
      await fn();
      mem.reload();
      return true;
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
      return false;
    }
  };
  const add = async () => {
    if (!text.trim()) return;
    if (await run(() => api.addMemory(slug, text.trim(), kind, scope))) setText("");
  };
  return (
    <div className="prof">
      <div className="cm-head">
        <div>
          <p className="cap">Seemo</p>
          <h1 className="d2">Memory</h1>
          <p className="muted">What Seemo remembers here. It reads all of it before every chat and every task. Your rules come first; what it learned from its own work comes after. Change or remove anything.</p>
        </div>
        {mem.data && (
          <label className="mem-learn">
            <Switch on={mem.data.learning} label="Learn from every chat and task" onChange={(on) => void run(() => api.setLearning(slug, on))} />
            <span>Learn from every chat and task</span>
          </label>
        )}
      </div>

      <div className="pcard mem-add">
        <input className="input" placeholder="Never promote; help first. Being useful is the promotion." value={text} maxLength={300} onChange={(e) => setText(e.target.value)} onKeyDown={(e) => e.key === "Enter" && void add()} aria-label="Something Seemo should remember" />
        <select className="input sm" value={kind} onChange={(e) => setKind(e.target.value as MemoryKind)} aria-label="Kind">
          {KINDS.map((k) => (
            <option key={k.kind} value={k.kind}>
              {ONE[k.kind]}
            </option>
          ))}
        </select>
        <WhereSelect value={scope} onChange={setScope} />
        <button className="btn p" disabled={!text.trim()} onClick={() => void add()}>
          <Plus size={14} /> Add
        </button>
      </div>
      {err && <p className="err">{err}</p>}

      {pending.length > 0 && (
        <section className="mem-sec">
          <p className="cap">Learned in a task; keep it?</p>
          <p className="muted sm">A task reads pages and threads nobody checked, so Seemo uses these only after you keep them.</p>
          <div className="tasks">
            {pending.map((m) => (
              <Row key={m.id} slug={slug} m={m} run={run} />
            ))}
          </div>
        </section>
      )}

      {mem.data && items.length === 0 && (
        <div className="pcard none">
          <Brain size={22} className="ac2" />
          <p className="t">Nothing yet</p>
          <p className="muted">Tell Seemo how you want things done (“never promote, just help”) and it keeps it here, or add a rule above. Seemo also notes what it learns from its work.</p>
        </div>
      )}

      {KINDS.map(({ kind: k, label, hint }) => {
        const list = items.filter((m) => m.kind === k && !m.pending);
        if (list.length === 0) return null;
        return (
          <section className="mem-sec" key={k}>
            <p className="cap" title={hint}>
              {label}
            </p>
            <div className="tasks">
              {list.map((m) => (
                <Row key={m.id} slug={slug} m={m} run={run} />
              ))}
            </div>
          </section>
        );
      })}
    </div>
  );
}

function Source({ slug, m }: { slug: string; m: Memory }) {
  const from = m.from ?? "";
  if (m.by === "user") return from.startsWith("chat:") ? <Link to={`/p/${slug}/chats/${from.slice(5)}`}>you, in a chat</Link> : <span>you</span>;
  if (from.startsWith("chat:"))
    return (
      <span className="chip ai">
        <span>
          learned in <Link to={`/p/${slug}/chats/${from.slice(5)}`}>a chat</Link>
        </span>
      </span>
    );
  if (from.startsWith("run:"))
    return (
      <span className="chip ai">
        <span>
          learned in <Link to={`/p/${slug}/tasks#${from.slice(4).split("/")[0]}`}>a task run</Link>
        </span>
      </span>
    );
  return <span className="chip ai">learned</span>;
}

function Row({ slug, m, run }: { slug: string; m: Memory; run: (fn: () => Promise<unknown>) => Promise<boolean> }) {
  const [edit, setEdit] = useState<string | null>(null);
  const [kind, setKind] = useState<MemoryKind>(m.kind);
  const [scope, setScope] = useState<MemoryScope>(m.scope ?? "everywhere");
  const [confirm, setConfirm] = useState(false);
  const save = async () => {
    if (edit === null || !edit.trim()) return;
    if (await run(() => api.updateMemory(slug, m.id, { text: edit.trim(), kind, scope }))) setEdit(null);
  };
  return (
    <div className="pcard task mem">
      <div className="row">
        <span className="ic">
          <Brain size={15} />
        </span>
        <div className="min0">
          {edit === null ? (
            <p className="mem-text">{m.text}</p>
          ) : (
            <div className="mem-edit">
              <input className="input" autoFocus value={edit} maxLength={300} onChange={(e) => setEdit(e.target.value)} onKeyDown={(e) => (e.key === "Enter" ? void save() : e.key === "Escape" && setEdit(null))} aria-label="Memory" />
              <select className="input sm" value={kind} onChange={(e) => setKind(e.target.value as MemoryKind)} aria-label="Kind">
                {KINDS.map((k) => (
                  <option key={k.kind} value={k.kind}>
                    {ONE[k.kind]}
                  </option>
                ))}
              </select>
              <WhereSelect value={scope} onChange={setScope} />
            </div>
          )}
          <p className="meta">
            {m.scope && m.scope !== "everywhere" && <span className="chip">{WHERE.find((w) => w.scope === m.scope)?.chip}</span>}
            <Source slug={slug} m={m} />
            <span>{ago(m.updated_at)}</span>
          </p>
        </div>
        <div className="acts">
          {edit !== null ? (
            <>
              <button className="btn t" title="Save" aria-label="Save" onClick={() => void save()}>
                <Check size={15} />
              </button>
              <button className="btn t" title="Cancel" aria-label="Cancel" onClick={() => setEdit(null)}>
                <X size={15} />
              </button>
            </>
          ) : (
            <>
              {m.pending && (
                <button className="btn g sm" onClick={() => void run(() => api.keepMemory(slug, m.id))}>
                  <Check size={14} /> Keep
                </button>
              )}
              <button className="btn t" title="Edit" aria-label="Edit" onClick={() => (setKind(m.kind), setScope(m.scope ?? "everywhere"), setEdit(m.text))}>
                <Pencil size={15} />
              </button>
              {confirm ? (
                <>
                  <button className="btn t bad" title="Yes, remove" aria-label="Yes, remove" onClick={() => void run(() => api.deleteMemory(slug, m.id))}>
                    <Check size={15} />
                  </button>
                  <button className="btn t" title="Keep it" aria-label="Keep it" onClick={() => setConfirm(false)}>
                    <X size={15} />
                  </button>
                </>
              ) : (
                <button className="btn t" title={m.pending ? "Drop" : "Remove"} aria-label={m.pending ? "Drop" : "Remove"} onClick={() => (m.pending ? void run(() => api.deleteMemory(slug, m.id)) : setConfirm(true))}>
                  <Trash2 size={15} />
                </button>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
