// Marketing → Channels → a mailing list: the people this project emails there. The
// addresses stay on this machine, outside the project's history; the page
// shows them, adds people (pasted, or a CSV file read here and sent as text)
// with how they agreed to hear from the business, marks those who asked to
// leave, and shows what was sent. A send itself goes through Seemo: "Send to
// this list" starts a chat, and the email comes back as a card for a yes.
import { useMemo, useRef, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { ArrowLeft, Check, FileUp, Loader2, Mail, Plus, Search, Send, Trash2, UserMinus, X } from "lucide-react";
import { api, useAsync, useEvents, type Channel, type Contact } from "../api";
import { openSeemo } from "../SeemoWidget";
import { ago } from "./Project";
import "../profile.css";
import "../accounts.css";

/** What changes a list: its people, a send, the channel itself or who sends it. */
const LIST_EVENTS = ["list_sending", "channel_saved", "channel_deleted", "account_saved", "account_state_changed"];

/** People on it who still hear from the business. */
const reachable = (l: Pick<Channel, "contacts" | "unsubscribed">) => (l.contacts ?? 0) - (l.unsubscribed ?? 0);

/** Start a chat that asks Seemo for an email to the list; the card for the send comes back there. */
async function askForEmail(slug: string, l: Channel) {
  const c = await api.createChat(slug, `Email to ${l.label}`);
  openSeemo(c.id);
  await api.sendMessage(slug, c.id, `Write an email to the ${l.label} list (${reachable(l)} people) and propose sending it to them from our mail account.`);
}

/** Lines of addresses into people: "Ana Ruiz <ana@example.com>", "ana@example.com, Ana Ruiz", or the address alone. */
function parsePeople(text: string): { email: string; name?: string }[] {
  const out: { email: string; name?: string }[] = [];
  for (const raw of text.split(/[\n;]+/)) {
    const line = raw.trim();
    if (!line) continue;
    const angle = /^(.*?)<([^>]+@[^>]+)>/.exec(line);
    if (angle) {
      out.push({ email: angle[2].trim(), name: angle[1].replace(/["',]/g, "").trim() || undefined });
      continue;
    }
    const parts = line.split(/[,\t]/).map((p) => p.trim());
    const email = parts.find((p) => /\S+@\S+\.\S+/.test(p));
    if (email) out.push({ email, name: parts.find((p) => p && p !== email) });
  }
  return out;
}

export function ListPage({ slug, id }: { slug: string; id: string }) {
  const navigate = useNavigate();
  const list = useAsync(() => api.channel(slug, id), [slug, id]);
  const people = useAsync(() => api.contacts(slug, id), [slug, id]);
  const sends = useAsync(() => api.listSends(slug, id).catch(() => ({ list: id, sends: [] })), [slug, id]);
  useEvents(slug, (e) => {
    if (!LIST_EVENTS.includes(e.type) || (e.list && e.list !== id) || (e.channel && e.channel !== id)) return;
    if (e.type === "channel_deleted") return navigate(`/p/${slug}/channels`);
    if (e.type === "list_sending") return sends.reload();
    list.reload();
    people.reload();
  }, { demo: false });
  const [q, setQ] = useState("");
  const [asking, setAsking] = useState(false);
  const [confirm, setConfirm] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const l = list.data;
  // The account it is sent from, and whether it works now.
  const via = l?.publisher_view;
  const all = people.data?.contacts ?? [];
  const shown = useMemo(() => {
    const needle = q.trim().toLowerCase();
    return needle ? all.filter((c) => `${c.email} ${c.name ?? ""}`.toLowerCase().includes(needle)) : all;
  }, [all, q]);
  const changed = () => (list.reload(), people.reload());
  const send = async () => {
    if (!l) return;
    setAsking(true);
    setError(null);
    try {
      await askForEmail(slug, l);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setAsking(false);
    }
  };
  const remove = async () => {
    try {
      await api.removeChannel(slug, id);
      navigate(`/p/${slug}/channels`);
    } catch (e) {
      setError((e as Error).message);
    }
  };
  if (list.error) return <div className="prof"><p className="err">{list.error}</p></div>;
  return (
    <div className="prof">
      <div className="cm-head">
        <div>
          <p className="cap">
            <Link to={`/p/${slug}/channels`}>
              <ArrowLeft size={11} /> Channels
            </Link>
          </p>
          <h1 className="d2">{l?.label ?? "…"}</h1>
          <p className="muted">
            {l ? `${reachable(l)} ${reachable(l) === 1 ? "person hears" : "people hear"} from you here${l.unsubscribed ? `; ${l.unsubscribed} asked to leave` : ""}.` : ""} {l?.description}
          </p>
          {l && (
            <p className="muted sm">
              {via ? (
                <>
                  Sent from <Link to={`/p/${slug}/vault#credential=${encodeURIComponent(via.credential)}`}>{via.label}</Link>
                  {!via.ready && <span className="warn"> · it does not work right now</span>}
                </>
              ) : (
                <>
                  No mail account sends it yet. <Link to={`/p/${slug}/channels#channel=${encodeURIComponent(id)}`}>Pick one</Link>.
                </>
              )}
            </p>
          )}
        </div>
        <div className="cm-acts">
          {confirm ? (
            <span className="acc-confirm">
              <span className="muted sm">Delete the list and everyone on it?</span>
              <button className="btn t sm bad" onClick={() => void remove()}>
                <Check size={13} /> Delete
              </button>
              <button className="btn t sm" onClick={() => setConfirm(false)}>
                Keep
              </button>
            </span>
          ) : (
            <button className="btn t" onClick={() => setConfirm(true)}>
              <Trash2 size={14} /> Delete
            </button>
          )}
          <button className="btn p" disabled={asking || !l || reachable(l) === 0} onClick={() => void send()} title={l && reachable(l) === 0 ? "Add people first" : "Seemo writes it and asks you before it goes out"}>
            {asking ? <Loader2 size={15} className="spin" /> : <Send size={15} />} Send to this list
          </button>
        </div>
      </div>
      {error && <p className="err">{error}</p>}
      <div className="ls-cols">
        <div className="pcard">
          <p className="cap">People</p>
          {all.length > 8 && (
            <label className="mk-search ls-search">
              <Search size={14} />
              <input className="input" placeholder="Find someone" value={q} onChange={(e) => setQ(e.target.value)} />
            </label>
          )}
          {people.loading && !people.data ? (
            <Loader2 size={14} className="spin" />
          ) : all.length === 0 ? (
            <p className="muted sm">Nobody yet. Add people on the right.</p>
          ) : (
            <table className="ls-table">
              <thead>
                <tr>
                  <th>Email</th>
                  <th>Name</th>
                  <th>How they agreed</th>
                  <th>Added</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {shown.map((c) => (
                  <Row key={c.email} slug={slug} list={id} c={c} onChange={changed} />
                ))}
              </tbody>
            </table>
          )}
          {l && (l.contacts ?? 0) > all.length && <p className="muted sm">Showing {all.length} of {l.contacts}.</p>}
        </div>
        <div className="ls-side">
          <AddPeople slug={slug} id={id} onDone={changed} />
          <div className="pcard" style={{ marginTop: 12 }}>
            <p className="cap">Sent to this list</p>
            {(sends.data?.sends ?? []).length === 0 ? (
              <p className="muted sm">Nothing yet.</p>
            ) : (
              <ul className="ls-sends">
                {sends.data!.sends.map((s) => (
                  <li key={s.key}>
                    <Mail size={13} />
                    <span className="grow" title={s.key}>
                      {s.key.replace(/^[a-z]+-/, "").replace(/-/g, " ")}
                    </span>
                    {s.running && (
                      <span className="chip sm acc">
                        <Loader2 size={10} className="spin" /> sending
                      </span>
                    )}
                    <span className="mono">
                      {s.sent} sent{s.failed ? `, ${s.failed} failed` : ""}
                    </span>
                    {s.at && <span className="mono">{ago(s.at)}</span>}
                  </li>
                ))}
              </ul>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

function Row({ slug, list, c, onChange }: { slug: string; list: string; c: Contact; onChange: () => void }) {
  const [busy, setBusy] = useState(false);
  const [confirm, setConfirm] = useState(false);
  const run = async (f: () => Promise<unknown>) => {
    setBusy(true);
    try {
      await f();
      onChange();
    } finally {
      setBusy(false);
      setConfirm(false);
    }
  };
  return (
    <tr className={c.unsubscribed ? "gone" : ""}>
      <td className="mono">{c.email}</td>
      <td>{c.name}</td>
      <td className="muted">{c.unsubscribed ? `Unsubscribed ${ago(c.unsubscribed)}` : c.consent}</td>
      <td className="muted">{c.added ? ago(c.added) : ""}</td>
      <td className="acts">
        {confirm ? (
          <>
            <button className="btn t sm bad" disabled={busy} onClick={() => void run(() => api.removeContact(slug, list, c.email))}>
              <Check size={12} /> Remove
            </button>
            <button className="btn t sm" onClick={() => setConfirm(false)} aria-label="Keep">
              <X size={12} />
            </button>
          </>
        ) : (
          <>
            {!c.unsubscribed && (
              <button className="btn t sm" disabled={busy} title="They asked to leave: never written to again" onClick={() => void run(() => api.unsubscribe(slug, list, c.email))}>
                <UserMinus size={12} /> Unsubscribe
              </button>
            )}
            <button className="btn t sm" disabled={busy} title="Remove from the list" aria-label="Remove" onClick={() => setConfirm(true)}>
              <Trash2 size={12} />
            </button>
          </>
        )}
      </td>
    </tr>
  );
}

/** Paste addresses, or pick a CSV file (read here, sent as text), with how these people agreed to hear from you. */
function AddPeople({ slug, id, onDone }: { slug: string; id: string; onDone: () => void }) {
  const [text, setText] = useState("");
  const [file, setFile] = useState<{ name: string; csv: string } | null>(null);
  const [consent, setConsent] = useState("");
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);
  const input = useRef<HTMLInputElement>(null);
  const pasted = parsePeople(text);
  const pick = async (f: File | undefined) => {
    if (!f) return;
    if (f.size > 20 * 1024 * 1024) return setMsg({ ok: false, text: "That file is over 20 MB." });
    setFile({ name: f.name, csv: await f.text() });
    setMsg(null);
  };
  const add = async () => {
    setBusy(true);
    setMsg(null);
    try {
      if (file) {
        const r = await api.importContacts(slug, id, file.csv, consent.trim());
        setMsg({ ok: true, text: `Found ${r.found} in ${file.name}; ${r.added} new on the list.` });
        setFile(null);
        if (input.current) input.current.value = "";
      } else {
        const r = await api.addContacts(slug, id, pasted, consent.trim());
        setMsg({ ok: true, text: `${r.added} new on the list${pasted.length > r.added ? `; ${pasted.length - r.added} were on it already` : ""}.` });
        setText("");
      }
      onDone();
    } catch (e) {
      setMsg({ ok: false, text: (e as Error).message });
    } finally {
      setBusy(false);
    }
  };
  const ready = (file || pasted.length > 0) && consent.trim().length > 0;
  return (
    <div className="pcard ls-add">
      <p className="cap">Add people</p>
      {file ? (
        <p className="sm">
          <FileUp size={13} /> <span className="mono">{file.name}</span>{" "}
          <button className="lnk-btn" onClick={() => (setFile(null), input.current && (input.current.value = ""))}>
            Another way
          </button>
        </p>
      ) : (
        <>
          <textarea className="input" placeholder={"ana@example.com\nBen Ode <ben@example.com>"} value={text} onChange={(e) => setText(e.target.value)} aria-label="Addresses, one per line" />
          <p className="muted sm">
            One per line{pasted.length ? `: ${pasted.length} found` : ""}. Or{" "}
            <button className="lnk-btn" onClick={() => input.current?.click()}>
              pick a CSV file
            </button>{" "}
            with an email column.
          </p>
        </>
      )}
      <input ref={input} type="file" accept=".csv,text/csv,text/plain" hidden onChange={(e) => void pick(e.target.files?.[0])} />
      <label className="acc-field">
        <span className="nm">How did they agree to hear from you?</span>
        <input className="input" value={consent} placeholder="Signed up on the website" onChange={(e) => setConsent(e.target.value)} />
        <span className="ds muted sm">Kept with each person, so you can always say why they get your email.</span>
      </label>
      <div className="acts">
        <button className="btn p sm" disabled={!ready || busy} onClick={() => void add()}>
          {busy ? <Loader2 size={13} className="spin" /> : <Plus size={13} />} Add {file ? "from the file" : pasted.length > 1 ? `${pasted.length} people` : "them"}
        </button>
      </div>
      {msg && <p className={`sm ${msg.ok ? "ok" : "err"}`}>{msg.text}</p>}
    </div>
  );
}
