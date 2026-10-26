// What you can do with an asset, by where it stands. Nobody sets the status:
// it follows from the facts. Approve picks the channel (Ready), Schedule sets
// the time (Scheduled), Publish sends it (Published). Unpublish, Archive and
// Bring back are the only steps backwards. The same block sits under a post
// in the chat, in the Library's modal, on the asset page and on the To do page.
import { useState } from "react";
import { Link } from "react-router-dom";
import { AlertTriangle, Archive, ArchiveRestore, CalendarClock, Check, ExternalLink, Loader2, Send, Undo2 } from "lucide-react";
import { FieldInput } from "./FieldInput";
import { api, useAsync, type Asset, type AssetStatus, type Channel } from "../api";
import { networkOf, useNetworks } from "../networks";
import type { Network } from "../api";
import { ago } from "./Project";

export const STATUSES: AssetStatus[] = ["draft", "ready", "scheduled", "published", "archived"];
export const STATUS_TONE: Record<AssetStatus, string> = { draft: "", ready: "acc", scheduled: "ai", published: "ok", archived: "" };
/** Badge wording, sentence case like "All". Ready reads as Approved: the user said yes to it for a channel. */
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

/** The channels of a post's network (its channel kind); everything that can publish otherwise. */
function fitting(a: Asset, ready: Channel[], networks: Network[]) {
  const net = networkOf(a, networks);
  if (!net) return ready;
  const hit = ready.filter((c) => c.kind === net.kind);
  return hit.length ? hit : ready;
}

export function AssetActions({ slug, a, compact, onChange }: { slug: string; a: Asset; compact?: boolean; onChange: () => void }) {
  const channels = useAsync(() => api.channels(slug).catch(() => ({ channels: [] as Channel[], kinds: [] })), [slug]);
  const all = channels.data?.channels ?? [];
  const ready = all.filter((c) => c.ready);
  const networks = useNetworks(slug);
  const choices = fitting(a, ready, networks);
  const [pick, setPick] = useState("");
  const [mode, setMode] = useState<"" | "schedule" | "channel">("");
  const [at, setAt] = useState(defaultAt);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const own = all.find((c) => c.id === a.channel);
  const chosen = pick || (mode !== "channel" && a.channel) || choices[0]?.id || "";
  const chosenLabel = all.find((c) => c.id === chosen)?.label ?? chosen;
  // Choices the channel's kind offers per publish (a blog: draft or published), the first value by default.
  const target = all.find((c) => c.id === (a.channel || chosen));
  const offered = Object.entries(channels.data?.kinds.find((k) => k.kind === target?.kind && k.connection === target?.ability)?.options ?? {}).filter(([, s]) => s.enum?.length);
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

  const picker = choices.length > 1 && (
    <select className="input sm" value={chosen} onChange={(e) => setPick(e.target.value)} aria-label="Channel">
      {choices.map((c) => (
        <option key={c.id} value={c.id}>
          {c.label} ({c.destination_text})
        </option>
      ))}
    </select>
  );
  const scheduleRow = mode === "schedule" && (
    <div className="aa-row">
      <input className="input sm" type="datetime-local" value={at} onChange={(e) => setAt(e.target.value)} />
      <button className="btn p sm" disabled={!!busy || !at} onClick={() => void run("schedule", () => api.scheduleAsset(slug, a, new Date(at).toISOString(), chosen, opts))}>
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
    body = !channels.data ? null : ready.length === 0 ? (
      <div className="aa-row">
        <span className="muted sm">
          Approve it once there is somewhere to send it. <Link to={`/p/${slug}/channels`}>Add a channel</Link>.
        </span>
        {archive}
      </div>
    ) : (
      <div className="aa-row">
        {picker}
        <button className="btn p sm" disabled={!!busy || !chosen} onClick={() => void run("approve", () => api.approveAsset(slug, a, chosen))}>
          {spin("approve", Check)} Approve{choices.length === 1 ? ` for ${chosenLabel}` : ""}
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
              <CalendarClock size={11} /> {whenAt(a.publish_at)} to {own?.label ?? a.channel}
            </span>
          ) : (
            <span className="chip acc">
              <Check size={11} /> Approved for {own?.label ?? a.channel}
            </span>
          )}
          {mode === "channel" ? (
            <>
              {picker || <span className="muted sm">No other channel.</span>}
              <button className="btn g sm" disabled={!!busy || !chosen || chosen === a.channel} onClick={() => void run("approve", () => api.approveAsset(slug, a, chosen))}>
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
              <button className="btn p sm" disabled={!!busy} onClick={() => void run("publish", () => api.publishAsset(slug, a, "", "", opts))}>
                {spin("publish", Send)} Publish now
              </button>
              {a.status === "scheduled" ? (
                <button className="btn t sm" disabled={!!busy} onClick={() => void run("unschedule", () => api.unscheduleAsset(slug, a))}>
                  {spin("unschedule", Undo2)} Unschedule
                </button>
              ) : (
                <button className="btn g sm" disabled={!!busy} onClick={() => setMode(mode === "schedule" ? "" : "schedule")}>
                  <CalendarClock size={14} /> Schedule
                </button>
              )}
              {ready.length > 1 && (
                <button className="btn t sm" disabled={!!busy} onClick={() => setMode("channel")}>
                  Change channel
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
              <span>{all.find((c) => c.id === p.connector)?.label ?? p.connector}</span>
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
          <button className="btn t sm" disabled={!!busy} title="KIQR stops counting it as published. What is on the channel stays there." onClick={() => void run("unpublish", () => api.unpublishAsset(slug, a))}>
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
