// What you can do with an asset, by where it stands. Nobody sets the status:
// it follows from the facts. Approve picks where it goes, an account and a
// place in it (Ready), Schedule sets the time (Scheduled), Publish sends it
// (Published). Unpublish, Archive and
// Bring back are the only steps backwards. The same block sits under a post
// in the chat, in the Library's modal, on the asset page and on the To do page.
import { useState } from "react";
import { Link } from "react-router-dom";
import { AlertTriangle, Archive, ArchiveRestore, CalendarClock, Check, ExternalLink, Loader2, Send, Undo2 } from "lucide-react";
import { FieldInput } from "./FieldInput";
import { api, useAsync, type Account, type Asset, type AssetStatus, type ChannelKindInfo, type MailList } from "../api";
import { networkOf, useNetworks } from "../networks";
import type { Network } from "../api";
import { ago } from "./Project";
import { connectPath, placeText } from "./Accounts";
import { openSeemo } from "../SeemoWidget";

export const STATUSES: AssetStatus[] = ["draft", "ready", "scheduled", "published", "archived"];
export const STATUS_TONE: Record<AssetStatus, string> = { draft: "", ready: "acc", scheduled: "ai", published: "ok", archived: "" };
/** Badge wording, sentence case like "All". Ready reads as Approved: the user said yes to it for an account. */
export const STATUS_LABEL: Record<AssetStatus, string> = { draft: "Draft", ready: "Approved", scheduled: "Scheduled", published: "Published", archived: "Archive" };

export function StatusChip({ a, channel }: { a: Asset; channel?: string }) {
  const text = a.status === "archived" ? "Archived" : a.status === "ready" && channel ? `Approved for ${channel}` : a.status === "scheduled" && a.publish_at ? `Scheduled ${whenAt(a.publish_at)}` : STATUS_LABEL[a.status];
  return <span className={`chip ${STATUS_TONE[a.status]}`}>{text}</span>;
}

/** "Mon 2 Nov, 09:00" in the viewer's timezone. */
export function whenAt(iso: string) {
  const d = new Date(iso);
  return d.toLocaleString([], { weekday: "short", day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });
}

/** Tomorrow 09:00, as the datetime-local input wants it. */
function defaultAt() {
  const d = new Date();
  d.setDate(d.getDate() + 1);
  d.setHours(9, 0, 0, 0);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/** The accounts of a post's network (its kind); everything that can publish otherwise. */
function fitting(a: Asset, ready: Account[], networks: Network[]) {
  const net = networkOf(a, networks);
  if (!net) return ready;
  const hit = ready.filter((c) => c.kind === net.kind);
  return hit.length ? hit : ready;
}

/** One place an asset can go: an account, and a place in it when it has places. */
type Target = { key: string; account: Account; place: string; text: string };
const keyOf = (account: string, place?: string | null) => `${account}::${place ?? ""}`;

/** Every account that publishes, one entry per place where it has places: "hello@kiqr.dev → Newsletter (412)". */
function targetsOf(accounts: Account[], lists: MailList[]): Target[] {
  return accounts.flatMap((c) =>
    (c.places ?? []).length
      ? [...(c.places ?? [])].sort((x, y) => Number(!!y.default) - Number(!!x.default)).map((p) => ({ key: keyOf(c.id, p.id), account: c, place: p.id, text: `${c.label} → ${placeText(p, lists)}` }))
      : [{ key: keyOf(c.id), account: c, place: "", text: c.label }],
  );
}

export function AssetActions({ slug, a, compact, onChange }: { slug: string; a: Asset; compact?: boolean; onChange: () => void }) {
  const accounts = useAsync(() => api.accounts(slug).then((r) => r.connections).catch(() => [] as Account[]), [slug]);
  // The kinds' per-publish choices (a blog: draft or published) come with the older view.
  const kinds = useAsync(() => api.channels(slug).then((r) => r.kinds).catch(() => [] as ChannelKindInfo[]), [slug]);
  const lists = useAsync(() => api.lists(slug).then((r) => r.lists).catch(() => [] as MailList[]), [slug]);
  const all = (accounts.data ?? []).filter((c) => c.publishes ?? c.channel);
  const ready = all.filter((c) => c.ready);
  const networks = useNetworks(slug);
  const targets = targetsOf(fitting(a, ready, networks), lists.data ?? []);
  const everywhere = targetsOf(all, lists.data ?? []);
  const [pick, setPick] = useState("");
  const [mode, setMode] = useState<"" | "schedule" | "channel">("");
  const [at, setAt] = useState(defaultAt);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const ownKey = a.channel ? keyOf(a.channel, a.place) : "";
  const own = everywhere.find((t) => t.key === ownKey) ?? everywhere.find((t) => t.account.id === a.channel && !a.place);
  const ownText = own?.text ?? all.find((c) => c.id === a.channel)?.label ?? a.channel ?? "";
  const chosenKey = pick || (mode !== "channel" && ownKey) || targets[0]?.key || "";
  const chosen = everywhere.find((t) => t.key === chosenKey) ?? targets.find((t) => t.key === chosenKey);
  const chosenAccount = chosen?.account.id ?? a.channel ?? "";
  const chosenPlace = chosen?.place ?? a.place ?? "";
  const chosenLabel = chosen?.text ?? chosenKey;
  // Choices the account's kind offers per publish (a blog: draft or published), the first value by default.
  const target = all.find((c) => c.id === (a.channel || chosenAccount));
  const kindInfo = kinds.data?.find((k) => k.kind === target?.kind && k.connection === target?.ability);
  const offered = Object.entries(kindInfo?.options ?? {}).filter(([, s]) => s.enum?.length);
  // A kind whose publish needs what only Seemo writes (a site's source files, as a pull request): the button asks Seemo instead.
  const prepared = kindInfo?.prepared ?? [];
  const askSeemo = async () => {
    const c = await api.createChat(slug, `Publish ${a.title}`);
    openSeemo(c.id);
    await api.sendMessage(slug, c.id, `Publish "${a.title}" to ${ownText || chosenLabel}: prepare what it needs (${prepared.join(", ")}) the way the site is built and propose the publish.`);
  };
  const [opts, setOpts] = useState<Record<string, string>>({});

  const run = async (what: string, fn: () => Promise<unknown>) => {
    setBusy(what);
    setError(null);
    try {
      await fn();
      setMode("");
      setPick("");
      onChange();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(null);
    }
  };
  const spin = (what: string, Icon: typeof Check) => (busy === what ? <Loader2 size={14} className="spin" /> : <Icon size={14} />);

  const picker = targets.length > 1 && (
    <select className="input sm" value={chosenKey} onChange={(e) => setPick(e.target.value)} aria-label="Where it goes">
      {targets.map((t) => (
        <option key={t.key} value={t.key}>
          {t.text}
        </option>
      ))}
    </select>
  );
  const scheduleRow = mode === "schedule" && (
    <div className="aa-row">
      <input className="input sm" type="datetime-local" value={at} onChange={(e) => setAt(e.target.value)} />
      <button className="btn p sm" disabled={!!busy || !at} onClick={() => void run("schedule", () => api.scheduleAsset(slug, a, new Date(at).toISOString(), chosenAccount, opts, chosenPlace))}>
        {spin("schedule", CalendarClock)} Schedule
      </button>
      <button className="btn t sm" onClick={() => setMode("")}>
        Cancel
      </button>
    </div>
  );
  const archive = !compact || a.status === "draft" ? (
    <button className="btn t sm" disabled={!!busy} title="Out of the way, not deleted" onClick={() => void run("archive", () => api.archiveAsset(slug, a, true))}>
      {spin("archive", Archive)} Archive
    </button>
  ) : null;

  let body: React.ReactNode;
  if (a.status === "archived") {
    body = (
      <div className="aa-row">
        <span className="muted sm">In the archive.</span>
        <button className="btn g sm" disabled={!!busy} onClick={() => void run("archive", () => api.archiveAsset(slug, a, false))}>
          {spin("archive", ArchiveRestore)} Bring it back
        </button>
      </div>
    );
  } else if (a.status === "draft") {
    body = !accounts.data ? null : ready.length === 0 ? (
      <div className="aa-row">
        <span className="muted sm">
          Approve it once there is somewhere to send it. <Link to={connectPath(slug)}>Connect an account</Link>.
        </span>
        {archive}
      </div>
    ) : (
      <div className="aa-row">
        {picker}
        <button className="btn p sm" disabled={!!busy || !chosen} onClick={() => void run("approve", () => api.approveAsset(slug, a, chosenAccount, chosenPlace))}>
          {spin("approve", Check)} Approve{targets.length === 1 ? ` for ${chosenLabel}` : ""}
        </button>
        {archive}
      </div>
    );
  } else if (a.status === "ready" || a.status === "scheduled") {
    body = (
      <>
        <div className="aa-row">
          {a.status === "scheduled" && a.publish_at ? (
            <span className="chip ai">
              <CalendarClock size={11} /> {whenAt(a.publish_at)} to {ownText}
            </span>
          ) : (
            <span className="chip acc">
              <Check size={11} /> Approved for {ownText}
            </span>
          )}
          {mode === "channel" ? (
            <>
              {picker || <span className="muted sm">Nowhere else to send it.</span>}
              <button className="btn g sm" disabled={!!busy || !chosen || chosenKey === ownKey} onClick={() => void run("approve", () => api.approveAsset(slug, a, chosenAccount, chosenPlace))}>
                {spin("approve", Check)} Use it
              </button>
              <button className="btn t sm" onClick={() => setMode("")}>
                Cancel
              </button>
            </>
          ) : (
            <>
              {offered.map(([name, spec]) => (
                <span key={name} className="pub-opt" title={spec.description}>
                  <FieldInput options={spec.enum} value={opts[name] ?? spec.enum?.[0] ?? ""} onChange={(v) => setOpts({ ...opts, [name]: v })} ariaLabel={name} />
                </span>
              ))}
              {prepared.length ? (
                <button className="btn p sm" disabled={!!busy} title={`Publishing here means ${prepared.join(" and ")} written the way the site is built, which Seemo does`} onClick={() => void run("ask", askSeemo)}>
                  {spin("ask", Send)} Ask Seemo to publish
                </button>
              ) : (
                <button className="btn p sm" disabled={!!busy} onClick={() => void run("publish", () => api.publishAsset(slug, a, "", "", opts))}>
                  {spin("publish", Send)} Publish now
                </button>
              )}
              {a.status === "scheduled" ? (
                <button className="btn t sm" disabled={!!busy} onClick={() => void run("unschedule", () => api.unscheduleAsset(slug, a))}>
                  {spin("unschedule", Undo2)} Unschedule
                </button>
              ) : (
                <button className="btn g sm" disabled={!!busy} onClick={() => setMode(mode === "schedule" ? "" : "schedule")}>
                  <CalendarClock size={14} /> Schedule
                </button>
              )}
              {targetsOf(ready, lists.data ?? []).length > 1 && (
                <button className="btn t sm" disabled={!!busy} onClick={() => setMode("channel")}>
                  Change where
                </button>
              )}
              {archive}
            </>
          )}
        </div>
        {scheduleRow}
      </>
    );
  } else {
    body = (
      <>
        <ul className="lib-pubs">
          {a.publications.map((p, i) => (
            <li key={i}>
              <Send size={12} />
              <span>{everywhere.find((t) => t.key === keyOf(p.connector, p.place))?.text ?? (accounts.data ?? []).find((c) => c.id === p.connector)?.label ?? p.connector}</span>
              {p.url ? (
                <a href={p.url} target="_blank" rel="noreferrer">
                  open <ExternalLink size={11} />
                </a>
              ) : (
                <small>{p.account ?? ""}</small>
              )}
              <small>{ago(p.at)}</small>
            </li>
          ))}
        </ul>
        <div className="aa-row">
          <button className="btn t sm" disabled={!!busy} title="KIQR stops counting it as published. What went out stays where it went." onClick={() => void run("unpublish", () => api.unpublishAsset(slug, a))}>
            {spin("unpublish", Undo2)} Unpublish
          </button>
          {archive}
        </div>
      </>
    );
  }
  return (
    <div className={`aa ${compact ? "compact" : ""}`}>
      {a.publish_error && (
        <p className="err">
          <AlertTriangle size={13} /> The scheduled publish failed: {a.publish_error}
        </p>
      )}
      {body}
      {error && (
        <p className="err">
          <AlertTriangle size={13} /> {error}
        </p>
      )}
    </div>
  );
}
