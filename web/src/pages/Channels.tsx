// Marketing → Channels: everywhere the business's people are. Owned ones (its
// profiles, pages, blog and mailing lists) and third-party ones (a subreddit,
// a forum). Each row shows its site, what Seemo may do there and who it posts
// as, or the one thing missing with its fix. A channel opens in a drawer
// (`#channel=<id>`): who publishes, read / publish / reply / learn, its name,
// and its people when it is a list (`/p/<slug>/channels/<id>`). `#add` adds
// one by link or site. The asset actions share the helpers here.
import { useEffect, useState } from "react";
import { Link, useLocation, useNavigate, useParams } from "react-router-dom";
import { AnimatePresence } from "motion/react";
import { AlertTriangle, Check, ChevronDown, ChevronUp, ExternalLink, Loader2, Mail, Plus, Radio, Sparkles, Trash2, Users, X } from "lucide-react";
import { api, CHANNEL_EVENTS, useEvents, type Channel, type ChannelKind, type ChannelMode, type ChannelPatch, type ChannelsView, type Ownership, type SpecField } from "../api";
import { FieldInput, enumOf } from "./FieldInput";
import { Favicon } from "./Favicon";
import { ListPage } from "./Lists";
import { Drawer } from "./Vault";
import { ago } from "./Project";
import "../profile.css";
import "../accounts.css";
import "../vault.css";
import "../channels.css";

// ------------------------------------------------------------------ shared

const cache = new Map<string, Promise<ChannelsView>>();

function load(slug: string): Promise<ChannelsView> {
  let p = cache.get(slug);
  if (!p) {
    p = api.channels(slug);
    p.catch(() => cache.delete(slug));
    cache.set(slug, p);
  }
  return p;
}

/** The project's channels, loaded once and shared by every block that needs them; reloaded when one, an account or an ability changes. */
export function useChannels(slug: string): { data: ChannelsView | null; error: string | null; reload: () => void } {
  const [data, setData] = useState<ChannelsView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [key, setKey] = useState(0);
  useEffect(() => {
    let live = true;
    load(slug).then(
      (v) => live && (setData(v), setError(null)),
      (e: Error) => live && setError(e.message),
    );
    return () => {
      live = false;
    };
  }, [slug, key]);
  const reload = () => {
    cache.delete(slug);
    setKey((k) => k + 1);
  };
  useEvents(slug, (e) => CHANNEL_EVENTS.includes(e.type) && reload(), { demo: false });
  return { data, error, reload };
}

/** "r/selfhosted · as u/acme_dev"; the publisher once when the channel is the profile itself. */
export function channelText(c: Channel) {
  const who = c.publisher_view?.label;
  return who && who !== c.label ? `${c.label} · as ${who}` : c.label;
}

/** A channel's icon: its site's favicon; a list's envelope. */
export function ChannelIcon({ c, size = 30 }: { c: Pick<Channel, "domain" | "kind">; size?: number }) {
  if (c.kind === "list")
    return (
      <span className="fav ch-list-ic" style={{ width: size, height: size }} aria-hidden>
        <Mail size={Math.round(size * 0.5)} />
      </span>
    );
  return <Favicon domain={c.domain} size={size} />;
}

/** The kind's fields that name a spot (a subreddit): a select for a fixed set, text otherwise. */
export function SpotFields({ kind, values, onChange }: { kind: ChannelKind; values: Record<string, string>; onChange: (v: Record<string, string>) => void }) {
  if (kind.account_is_target) return null;
  return (
    <>
      {Object.entries(kind.target ?? {}).map(([name, spec]) => (
        <SpotField key={name} name={name} spec={spec} value={values[name] ?? ""} onChange={(v) => onChange({ ...values, [name]: v })} />
      ))}
    </>
  );
}

function SpotField({ name, spec, value, onChange }: { name: string; spec: SpecField; value: string; onChange: (v: string) => void }) {
  return (
    <label className="acc-field">
      <span className="nm">
        {spec.label ?? name.replace(/_/g, " ")}
        {spec.optional && <small className="dim"> optional</small>}
      </span>
      <FieldInput value={value} onChange={onChange} options={enumOf(spec)} optional={spec.optional} placeholder={spec.placeholder} mono ariaLabel={spec.label ?? name} />
      {spec.help && <span className="ds muted">{spec.help}</span>}
    </label>
  );
}

/** Only the fields with a value, as the daemon wants them. */
export const filled = (v: Record<string, string>) => Object.fromEntries(Object.entries(v).filter(([, x]) => x.trim()).map(([k, x]) => [k, x.trim()]));

const MODE_WORD: Record<ChannelMode, string> = { off: "Off", ask: "Asks first", auto: "On its own" };

/** The one line under a row or in a picker: who Seemo posts as, or what is missing. */
export function ChannelState({ c }: { c: Channel }) {
  const r = c.readiness;
  if (r.state === "ready")
    return (
      <span className="chip ok sm">
        <b /> as {c.publisher_view?.label ?? c.publisher}
      </span>
    );
  if (r.state === "publish_off") return <span className="chip sm">Publishing off</span>;
  if (r.state === "suggested") return <span className="chip ai sm">Suggested</span>;
  return (
    <span className="chip warn sm" title={r.text}>
      <AlertTriangle size={11} /> {SHORT[r.state] ?? r.text}
    </span>
  );
}

const SHORT: Partial<Record<Channel["readiness"]["state"], string>> = {
  no_publisher: "No publisher",
  publisher_gone: "Publisher gone",
  publisher_not_ready: "Publisher needs a look",
  no_ability: "Nothing publishes here",
};

// ------------------------------------------------------------------ page

export function ChannelsPage() {
  const { slug = "", id } = useParams();
  return id ? <ListPage slug={slug} id={decodeURIComponent(id)} /> : <ChannelsIndex slug={slug} />;
}

function ChannelsIndex({ slug }: { slug: string }) {
  const view = useChannels(slug);
  const location = useLocation();
  const navigate = useNavigate();
  const [review, setReview] = useState(false);
  const setHash = (h: string) => navigate({ hash: h }, { replace: true });
  const open = decodeURIComponent(/#channel=([^&]+)/.exec(location.hash)?.[1] ?? "");
  const adding = location.hash === "#add";
  const all = view.data?.channels ?? [];
  const kinds = view.data?.kinds ?? [];
  const suggestions = all.filter((c) => c.suggested);
  const kept = all.filter((c) => !c.suggested);
  const owned = kept.filter((c) => c.ownership === "owned");
  const third = kept.filter((c) => c.ownership === "third_party");
  const current = all.find((c) => c.id === open);
  const openOne = (id: string) => setHash(`channel=${encodeURIComponent(id)}`);
  return (
    <div className="prof">
      <div className="cm-head">
        <div>
          <p className="cap">Marketing</p>
          <h1 className="d2">Channels</h1>
          <p className="muted">Everywhere your people are: your own profiles, pages and lists, and the communities you take part in. Each says what Seemo may do there and who it posts as.</p>
        </div>
        <button className="btn p" onClick={() => setHash("add")}>
          <Plus size={15} /> Add a channel
        </button>
      </div>
      {view.error && <p className="err">{view.error}</p>}
      {suggestions.length > 0 && (
        <div className="pcard au-sugg">
          <button className="au-sugg-head" onClick={() => setReview((r) => !r)} aria-expanded={review}>
            <Sparkles size={15} />
            <span className="grow">
              Seemo found {suggestions.length} {suggestions.length === 1 ? "place" : "places"} worth your time
            </span>
            <span className="lnk">Review {review ? <ChevronUp size={13} /> : <ChevronDown size={13} />}</span>
          </button>
          {review && (
            <ul className="au-rows">
              {suggestions.map((c) => (
                <SuggestionRow key={c.id} slug={slug} c={c} onChange={view.reload} />
              ))}
            </ul>
          )}
        </div>
      )}
      {view.data && kept.length === 0 && (
        <div className="pcard none">
          <Radio size={22} className="ac2" />
          <p className="t">No channels yet</p>
          <p className="muted">Add your X profile, your company page, a subreddit you take part in, or a mailing list. Seemo needs nothing else to know where your people are; to post there, pick who it posts as.</p>
          <button className="btn g" onClick={() => setHash("add")}>
            <Plus size={14} /> Add a channel
          </button>
        </div>
      )}
      <div className="acx-groups">
        {owned.length > 0 && <Section title="Owned" hint="Your profiles, pages, blog and mailing lists" items={owned} onOpen={openOne} />}
        {third.length > 0 && <Section title="Third-party" hint="Communities and places you take part in" items={third} onOpen={openOne} />}
      </div>
      {!view.data && !view.error && <Loader2 size={16} className="spin" />}
      <AnimatePresence>
        {adding && view.data && (
          <Drawer key="add" title="Add a channel" iconNode={<Radio size={22} />} onClose={() => setHash("")}>
            <AddChannel slug={slug} kinds={kinds} onDone={(c) => (view.reload(), setHash(`channel=${encodeURIComponent(c.id)}`))} onCancel={() => setHash("")} />
          </Drawer>
        )}
        {current && (
          <Drawer key={current.id} title={current.label} icon={current.kind === "list" ? undefined : current.domain} iconNode={current.kind === "list" ? <Mail size={22} /> : undefined} sub={subline(current)} onClose={() => setHash("")}>
            <ChannelDetails slug={slug} c={current} onChange={view.reload} onRemoved={() => (setHash(""), view.reload())} />
          </Drawer>
        )}
        {open && view.data && !current && (
          <Drawer key="gone" title="Not a channel" iconNode={<Radio size={22} />} onClose={() => setHash("")}>
            <p className="muted">There is no channel “{open}” any more.</p>
          </Drawer>
        )}
      </AnimatePresence>
    </div>
  );
}

function subline(c: Channel): string {
  const kind = c.kind === "list" ? "Mailing list" : c.kind_name ?? "";
  const where = c.kind === "list" ? "" : c.url ? c.url.replace(/^https?:\/\/(www\.)?/, "").replace(/\/$/, "") : c.domain;
  return [kind, where].filter(Boolean).join(" · ");
}

function Section({ title, hint, items, onOpen }: { title: string; hint: string; items: Channel[]; onOpen: (id: string) => void }) {
  return (
    <section className="acx-group">
      <h2>
        {title} <span className="n">{items.length}</span>
      </h2>
      <p className="muted sm ch-hint">{hint}</p>
      <div className="vt-list">
        {items.map((c) => (
          <button key={c.id} className="vt-row ch-row" onClick={() => onOpen(c.id)}>
            <ChannelIcon c={c} size={32} />
            <span className="min0 nm">
              <b>{c.label}</b>
              <small>{c.kind === "list" ? `${(c.contacts ?? 0) - (c.unsubscribed ?? 0)} people` : subline(c)}</small>
            </span>
            <span className="ch-modes">
              <span className={`ch-mode ${c.publish}`} title="Publishing">
                Publish: {MODE_WORD[c.publish].toLowerCase()}
              </span>
            </span>
            <ChannelState c={c} />
          </button>
        ))}
      </div>
    </section>
  );
}

/** Something Seemo found: why, and Add or Dismiss. */
function SuggestionRow({ slug, c, onChange }: { slug: string; c: Channel; onChange: () => void }) {
  const [busy, setBusy] = useState<"add" | "dismiss" | null>(null);
  const [error, setError] = useState<string | null>(null);
  const run = async (what: "add" | "dismiss") => {
    setBusy(what);
    setError(null);
    try {
      await (what === "add" ? api.updateChannel(slug, c.id, { add: true }) : api.removeChannel(slug, c.id));
      onChange();
    } catch (x) {
      setError((x as Error).message);
      setBusy(null);
    }
  };
  return (
    <li className="au-row">
      <span className="what">
        <ChannelIcon c={c} />
        <span className="min0">
          <b>
            {c.label}
            {c.url && (
              <a className="dim" href={c.url} target="_blank" rel="noreferrer" aria-label="Open it">
                {" "}
                <ExternalLink size={11} />
              </a>
            )}
          </b>
          <small>{subline(c)}</small>
          {c.reason && <small className="ai">{c.reason}</small>}
        </span>
      </span>
      <button className="btn p sm" disabled={!!busy} onClick={() => void run("add")}>
        {busy === "add" ? <Loader2 size={13} className="spin" /> : <Plus size={13} />} Add
      </button>
      <button className="btn t sm" disabled={!!busy} onClick={() => void run("dismiss")}>
        {busy === "dismiss" ? <Loader2 size={13} className="spin" /> : <X size={13} />} Dismiss
      </button>
      {error && <p className="err sm au-err">{error}</p>}
    </li>
  );
}

// ------------------------------------------------------------------ one channel

const ACT_CHOICES: { v: ChannelMode; label: string }[] = [
  { v: "auto", label: "On its own" },
  { v: "ask", label: "Ask me first" },
  { v: "off", label: "Off" },
];

/** One channel: who Seemo posts as, what it may do there, its name and link, its people (a list), and remove. */
function ChannelDetails({ slug, c, onChange, onRemoved }: { slug: string; c: Channel; onChange: () => void; onRemoved: () => void }) {
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [label, setLabel] = useState(c.label);
  const [confirm, setConfirm] = useState(false);
  useEffect(() => setLabel(c.label), [c.label]);
  const isList = c.kind === "list";
  const save = async (what: string, patch: ChannelPatch) => {
    setBusy(what);
    setError(null);
    try {
      await api.updateChannel(slug, c.id, patch);
      onChange();
    } catch (x) {
      setError((x as Error).message);
    } finally {
      setBusy(null);
    }
  };
  const remove = async () => {
    setBusy("remove");
    try {
      await api.removeChannel(slug, c.id);
      onRemoved();
    } catch (x) {
      setError((x as Error).message);
      setBusy(null);
    }
  };
  const learn = async () => {
    setBusy("learn-now");
    setError(null);
    try {
      await api.learnChannel(slug, c.id);
    } catch (x) {
      setError((x as Error).message);
    } finally {
      setBusy(null);
    }
  };
  const r = c.readiness;
  return (
    <div className="vt-detail">
      {!r.ready && r.state !== "publish_off" && (
        <div className="ch-need">
          <AlertTriangle size={16} />
          <span className="grow">{r.text}</span>
          {r.fix?.action === "abilities" && (
            <Link className="btn g sm" to={`/p/${slug}/abilities`}>
              {r.fix.label}
            </Link>
          )}
          {r.fix?.action === "account" && r.fix.account && (
            <Link className="btn g sm" to={`/p/${slug}/accounts#account=${encodeURIComponent(r.fix.account)}`}>
              {r.fix.label}
            </Link>
          )}
          {r.fix?.action === "add" && (
            <button className="btn p sm" disabled={!!busy} onClick={() => void save("add", { add: true })}>
              <Plus size={13} /> Add it
            </button>
          )}
        </div>
      )}

      <section>
        <p className="cap">{isList ? "Sent from" : "Seemo posts as"}</p>
        <Publisher slug={slug} c={c} busy={busy === "publisher"} onPick={(p) => void save("publisher", { publisher: p })} />
      </section>

      <section>
        <p className="cap">What Seemo may do here</p>
        <div className="acx-perms">
          {!isList && (
            <Perm title="Read" help="Posts, replies and mentions here" value={c.read ? "auto" : "off"} choices={[{ v: "auto", label: "On" }, { v: "off", label: "Off" }]} busy={busy === "read"} onPick={(v) => void save("read", { read: v === "auto" })} />
          )}
          <Perm title={isList ? "Send" : "Publish"} help={isList ? "Emails to everyone on the list" : "Posts, articles and updates"} value={c.publish} choices={ACT_CHOICES} busy={busy === "publish"} onPick={(v) => void save("publish", { publish: v })} />
          {!isList && <Perm title="Reply and engage" help="Answering, liking, following, moderating" value={c.reply} choices={ACT_CHOICES} busy={busy === "reply"} onPick={(v) => void save("reply", { reply: v })} />}
          {c.ownership === "owned" && c.can_learn && (
            <Perm title="Learn from it" help={c.last_learned ? `Last read ${ago(c.last_learned)}${c.learned ? `, ${c.learned} documents` : ""}` : "What is here teaches Seemo about the business"} value={c.learn ? "auto" : "off"} choices={[{ v: "auto", label: "On" }, { v: "off", label: "Off" }]} busy={busy === "learn"} onPick={(v) => void save("learn", { learn: v === "auto" })} />
          )}
        </div>
        <p className="muted sm">On its own: it goes out at once and the chat keeps the card. Anywhere that is not a channel, Seemo always asks.</p>
        {c.learn && (
          <button className="btn t sm ch-learn" disabled={!!busy} onClick={() => void learn()}>
            {busy === "learn-now" ? <Loader2 size={13} className="spin" /> : <Sparkles size={13} />} Learn from it now
          </button>
        )}
      </section>

      {isList && (
        <section>
          <p className="cap">People</p>
          <Link className="btn g sm ch-people" to={`/p/${slug}/channels/${encodeURIComponent(c.id)}`}>
            <Users size={14} /> {(c.contacts ?? 0) - (c.unsubscribed ?? 0)} people{c.unsubscribed ? `, ${c.unsubscribed} left` : ""} · open the list
          </Link>
        </section>
      )}

      <section>
        <p className="cap">About it</p>
        <div className="vt-inline">
          <input className="input" value={label} aria-label="Name" onChange={(e) => setLabel(e.target.value)} />
          {label.trim() && label.trim() !== c.label && (
            <button className="btn g sm" disabled={!!busy} onClick={() => void save("label", { label: label.trim() })}>
              {busy === "label" ? <Loader2 size={13} className="spin" /> : <Check size={13} />} Rename
            </button>
          )}
        </div>
        {!isList && (
          <div className="ch-about">
            <div className="seg" role="radiogroup" aria-label="Whose it is">
              {(["owned", "third_party"] as Ownership[]).map((o) => (
                <button key={o} role="radio" aria-checked={c.ownership === o} className={c.ownership === o ? "on" : ""} disabled={busy === "ownership"} onClick={() => c.ownership !== o && void save("ownership", { ownership: o })}>
                  {o === "owned" ? "Ours" : "Someone else's"}
                </button>
              ))}
            </div>
            {c.url && (
              <a className="sm" href={c.url} target="_blank" rel="noreferrer">
                {c.url.replace(/^https?:\/\//, "")} <ExternalLink size={11} />
              </a>
            )}
            {c.ability_name && <span className="muted sm">Published by {c.ability_name}</span>}
          </div>
        )}
        <p className="muted sm">
          {c.publications ? `${c.publications} ${c.publications === 1 ? "publish" : "publishes"}${c.last_published ? `, the last ${ago(c.last_published)}` : ""}. ` : ""}
          Added {ago(c.created_at)}
          {c.added_by === "seemo" ? " by Seemo" : c.added_by === "crawl" ? " from your website" : ""}.
        </p>
      </section>

      {error && <p className="err sm">{error}</p>}
      <div className="vf-acts">
        {confirm ? (
          <span className="acc-confirm">
            <span className="muted sm">{isList ? "Delete the list and everyone on it?" : "Remove this channel? What was published stays where it went."}</span>
            <button className="btn t sm bad" disabled={!!busy} onClick={() => void remove()}>
              {busy === "remove" ? <Loader2 size={12} className="spin" /> : <Check size={12} />} {isList ? "Delete" : "Remove"}
            </button>
            <button className="btn t sm" onClick={() => setConfirm(false)}>
              Keep
            </button>
          </span>
        ) : (
          <button className="btn t sm" onClick={() => setConfirm(true)}>
            <Trash2 size={13} /> {isList ? "Delete the list" : "Remove the channel"}
          </button>
        )}
      </div>
    </div>
  );
}

function Perm({ title, help, value, choices, busy, onPick }: { title: string; help: string; value: ChannelMode; choices: { v: ChannelMode; label: string }[]; busy: boolean; onPick: (v: ChannelMode) => void }) {
  return (
    <div className="acx-perm">
      <div className="min0">
        <b>{title}</b>
        <span className="muted sm">{help}</span>
      </div>
      <div className="seg" role="radiogroup" aria-label={title}>
        {choices.map((ch) => (
          <button key={ch.v} role="radio" aria-checked={value === ch.v} className={value === ch.v ? `on ${ch.v}` : ""} disabled={busy} onClick={() => value !== ch.v && onPick(ch.v)}>
            {ch.label}
          </button>
        ))}
      </div>
    </div>
  );
}

/** Who Seemo posts as: one of the accounts that can publish here, or nobody yet. */
function Publisher({ slug, c, busy, onPick }: { slug: string; c: Channel; busy: boolean; onPick: (id: string) => void }) {
  const options = c.publishers;
  const cur = c.publisher_view;
  if (!options.length && !cur)
    return (
      <div className="ch-pub none">
        <span className="muted sm">
          {c.readiness.state === "no_ability" ? (
            <>Nobody to pick until an ability publishes here. The channel works for planning meanwhile.</>
          ) : (
            <>
              Nobody to post as yet. <Link to={`/p/${slug}/accounts/connect`}>Add a {c.kind === "list" ? "mail account" : `${c.domain || "site"} account`}</Link> and pick it here.
            </>
          )}
        </span>
      </div>
    );
  return (
    <div className="ch-pub">
      <select className="input" value={c.publisher} disabled={busy} aria-label="Posts as" onChange={(e) => onPick(e.target.value)}>
        <option value="">Nobody yet</option>
        {cur && !options.some((o) => o.account === cur.account) && <option value={cur.account}>{cur.label}</option>}
        {options.map((o) => (
          <option key={o.account} value={o.account}>
            {o.label}
            {o.ready ? "" : " (needs a look)"}
          </option>
        ))}
      </select>
      {busy && <Loader2 size={14} className="spin" />}
      {cur && !cur.ready && (
        <Link className="sm" to={`/p/${slug}/accounts#account=${encodeURIComponent(cur.account)}`}>
          It does not work right now
        </Link>
      )}
    </div>
  );
}

// ------------------------------------------------------------------ add

/** Add a channel: a link or a site (the kind and its fields are read from it), a kind with its fields, or a mailing list. */
function AddChannel({ slug, kinds, onDone, onCancel }: { slug: string; kinds: ChannelKind[]; onDone: (c: Channel) => void; onCancel: () => void }) {
  const [what, setWhat] = useState<"link" | "kind" | "list">("link");
  const [url, setUrl] = useState("");
  const spotKinds = kinds.filter((k) => k.kind !== "list" && !k.account_is_target && Object.keys(k.target ?? {}).length > 0);
  const [kindId, setKindId] = useState(spotKinds[0]?.kind ?? "");
  const kind = spotKinds.find((k) => k.kind === kindId);
  const [fields, setFields] = useState<Record<string, string>>({});
  const [label, setLabel] = useState("");
  const [description, setDescription] = useState("");
  const [ownership, setOwnership] = useState<Ownership | "">("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const missing = what === "link" ? !url.trim() : what === "list" ? !label.trim() : !kind || Object.entries(kind.target ?? {}).some(([n, s]) => !s.optional && !(fields[n] ?? "").trim());
  const add = async () => {
    setBusy(true);
    setError(null);
    try {
      const u = url.trim();
      const body =
        what === "list"
          ? { kind: "list", label: label.trim(), description: description.trim() || undefined }
          : what === "kind"
            ? { kind: kind!.kind, fields: filled(fields), label: label.trim() || undefined, ownership }
            : { ...(/^[a-z]+:\/\//i.test(u) || u.includes("/") ? { url: /^[a-z]+:\/\//i.test(u) ? u : `https://${u}` } : { domain: u }), label: label.trim() || undefined, ownership };
      onDone(await api.addChannel(slug, body));
    } catch (x) {
      setError((x as Error).message);
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="vf">
      <div className="seg ch-what" role="radiogroup" aria-label="What to add">
        {(
          [
            ["link", "A link or site"],
            ...(spotKinds.length ? [["kind", "A community"]] : []),
            ["list", "A mailing list"],
          ] as [typeof what, string][]
        ).map(([v, l]) => (
          <button key={v} role="radio" aria-checked={what === v} className={what === v ? "on" : ""} onClick={() => (setWhat(v), setError(null))}>
            {l}
          </button>
        ))}
      </div>
      {what === "link" && (
        <label className="acc-field">
          <span className="nm">Link or site</span>
          <div className="ch-url">
            {url.trim() && <Favicon domain={hostOf(url)} size={22} />}
            <input className="input" autoFocus value={url} placeholder="https://x.com/yourcompany" onChange={(e) => setUrl(e.target.value)} onKeyDown={(e) => e.key === "Enter" && !missing && void add()} />
          </div>
          <span className="ds muted">A profile, a page, a subreddit, a forum category. Seemo reads what it is from the link.</span>
        </label>
      )}
      {what === "kind" && (
        <div className="au-form">
          <label className="acc-field">
            <span className="nm">What it is</span>
            <FieldInput value={kindId} onChange={(v) => (setKindId(v), setFields({}))} options={spotKinds.map((k) => k.kind)} labels={Object.fromEntries(spotKinds.map((k) => [k.kind, k.name || k.label]))} ariaLabel="Kind" plain />
          </label>
          {kind && <SpotFields kind={kind} values={fields} onChange={setFields} />}
        </div>
      )}
      {what === "list" ? (
        <>
          <label className="acc-field">
            <span className="nm">Name</span>
            <input className="input" autoFocus value={label} placeholder="Newsletter" onChange={(e) => setLabel(e.target.value)} />
          </label>
          <label className="acc-field">
            <span className="nm">
              What it is for<small className="dim"> optional</small>
            </span>
            <input className="input" value={description} placeholder="Product news, once a month" onChange={(e) => setDescription(e.target.value)} />
          </label>
        </>
      ) : (
        <>
          <label className="acc-field">
            <span className="nm">
              Name<small className="dim"> optional</small>
            </span>
            <input className="input" value={label} placeholder="As you call it" onChange={(e) => setLabel(e.target.value)} />
          </label>
          <div className="acc-field">
            <span className="nm">Whose is it?</span>
            <div className="seg" role="radiogroup" aria-label="Whose it is">
              {(
                [
                  ["", "Let Seemo tell"],
                  ["owned", "Ours"],
                  ["third_party", "Someone else's"],
                ] as [Ownership | "", string][]
              ).map(([v, l]) => (
                <button key={v || "auto"} role="radio" aria-checked={ownership === v} className={ownership === v ? "on" : ""} onClick={() => setOwnership(v)}>
                  {l}
                </button>
              ))}
            </div>
          </div>
        </>
      )}
      <p className="muted sm">Seemo asks before it publishes anywhere new. You choose who it posts as, and whether it may post on its own, once the channel is added.</p>
      {error && <p className="err sm">{error}</p>}
      <div className="vf-acts">
        <button className="btn p" disabled={busy || missing} onClick={() => void add()}>
          {busy ? <Loader2 size={14} className="spin" /> : <Check size={14} />} {what === "list" ? "Make the list" : "Add the channel"}
        </button>
        <button className="btn t" onClick={onCancel}>
          Cancel
        </button>
      </div>
    </div>
  );
}

function hostOf(u: string): string {
  const s = u.trim().replace(/^[a-z]+:\/\//i, "");
  return s.split(/[/?#]/)[0].replace(/^www\./, "").toLowerCase();
}
