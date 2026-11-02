// What you can do with an asset, by where it stands. Nobody sets the status:
// it follows from the facts. Approve picks where it goes, a channel or
// somewhere new that becomes one with the yes (Ready), Schedule sets the
// time (Scheduled), Publish sends it (Published). Unpublish, Archive and
// Bring back are the only steps backwards. The same block sits under a post
// in the chat, in the Library's modal, on the asset page and on the To do page.
import { useState } from "react";
import { Link } from "react-router-dom";
import { AlertTriangle, Archive, ArchiveRestore, CalendarClock, Check, ExternalLink, Loader2, MapPin, Send, Undo2 } from "lucide-react";
import { FieldInput } from "./FieldInput";
import { api, type Asset, type AssetStatus, type Channel, type ChannelKind, type Spot } from "../api";
import { networkOf, useNetworks } from "../networks";
import { ago } from "./Project";
import { channelText, filled, SpotFields, useChannels } from "./Channels";
import { openSeemo } from "../SeemoWidget";

export const STATUSES: AssetStatus[] = ["draft", "ready", "scheduled", "published", "archived"];
export const STATUS_TONE: Record<AssetStatus, string> = { draft: "", ready: "acc", scheduled: "ai", published: "ok", archived: "" };
/** Badge wording, sentence case like "All". Ready reads as Approved: the user said yes to it for an account. */
export const STATUS_LABEL: Record<AssetStatus, string> = { draft: "Draft", ready: "Approved", scheduled: "Scheduled", published: "Published", archived: "Archive" };

export function StatusChip({ a, where }: { a: Asset; where?: string }) {
  const text = a.status === "archived" ? "Archived" : a.status === "ready" && where ? `Approved for ${where}` : a.status === "scheduled" && a.publish_at ? `Scheduled ${whenAt(a.publish_at)}` : STATUS_LABEL[a.status];
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

/** Where an asset can go now: the channels that publish, those of the post's network first (all when none fits). */
function fitting(a: Asset, channels: Channel[], netKind: string | undefined) {
  const kind = netKind ?? a.tags[0]?.toLowerCase();
  const hit = kind ? channels.filter((e) => e.kind === kind) : [];
  return hit.length ? hit : channels;
}

/** Why a channel cannot take it now, in a few words; nothing when it can. */
function blocked(c: Channel): string {
  return c.readiness.ready ? "" : ` (${c.readiness.text.charAt(0).toLowerCase()}${c.readiness.text.slice(1)})`;
}

/** A select of channels, owned first: "r/selfhosted · as u/acme_dev". */
function ChannelPicker({ channels, value, onChange }: { channels: Channel[]; value: string; onChange: (id: string) => void }) {
  return (
    <select className="input sm" value={value} onChange={(e) => onChange(e.target.value)} aria-label="Where it goes">
      {(
        [
          ["owned", "Owned"],
          ["third_party", "Third-party"],
        ] as const
      ).map(([o, label]) => {
        const of = channels.filter((e) => e.ownership === o);
        return of.length ? (
          <optgroup key={o} label={label}>
            {of.map((e) => (
              <option key={e.id} value={e.id} disabled={!e.readiness.ready}>
                {channelText(e)}
                {blocked(e)}
              </option>
            ))}
          </optgroup>
        ) : null;
      })}
    </select>
  );
}

/** Somewhere that is no channel yet: a kind and the fields that name it. The yes makes it a channel. */
function SpotForm({ kinds, busy, onApprove, onCancel }: { kinds: ChannelKind[]; busy: boolean; onApprove: (spot: Spot) => void; onCancel: () => void }) {
  const [kindId, setKindId] = useState(kinds[0]?.kind ?? "");
  const kind = kinds.find((k) => k.kind === kindId);
  const [fields, setFields] = useState<Record<string, string>>({});
  const missing = !kind || Object.entries(kind.target ?? {}).some(([n, s]) => !s.optional && !(fields[n] ?? "").trim());
  return (
    <div className="aa-spot">
      <div className="au-form">
        <label className="acc-field">
          <span className="nm">What it is</span>
          <FieldInput value={kindId} onChange={(v) => (setKindId(v), setFields({}))} options={kinds.map((k) => k.kind)} labels={Object.fromEntries(kinds.map((k) => [k.kind, k.name || k.label]))} ariaLabel="Kind" plain />
        </label>
        {kind && <SpotFields kind={kind} values={fields} onChange={setFields} />}
      </div>
      <div className="aa-row">
        <button className="btn p sm" disabled={busy || missing} onClick={() => kind && onApprove({ kind: kind.kind, fields: filled(fields) })}>
          <Check size={14} /> Approve for it
        </button>
        <button className="btn t sm" onClick={onCancel}>
          Cancel
        </button>
        <span className="new-spot">It becomes a channel.</span>
      </div>
    </div>
  );
}

export function AssetActions({ slug, a, compact, onChange }: { slug: string; a: Asset; compact?: boolean; onChange: () => void }) {
  const view = useChannels(slug);
  const networks = useNetworks(slug);
  const entries = (view.data?.channels ?? []).filter((e) => !e.suggested);
  const publishing = entries.filter((e) => e.publishes && e.publish !== "off");
  // Somewhere new: a kind whose spots have names (a subreddit), published to by an ability here.
  const kinds = (view.data?.kinds ?? []).filter((k) => k.kind !== "list" && k.publishes && !k.account_is_target && Object.keys(k.target ?? {}).length > 0);
  const canSpot = kinds.length > 0;
  const fits = fitting(a, publishing, networkOf(a, networks)?.kind);
  const usable = fits.filter((e) => e.readiness.ready);
  const [pick, setPick] = useState("");
  const [mode, setMode] = useState<"" | "schedule" | "where" | "spot">("");
  const [at, setAt] = useState(defaultAt);
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [opts, setOpts] = useState<Record<string, string>>({});
  const own = entries.find((e) => e.id === a.target);
  const ownText = own?.label ?? a.target ?? "";
  const chosenId = pick || (usable.find((e) => e.id !== a.target) ?? usable[0])?.id || "";
  const chosen = publishing.find((e) => e.id === chosenId);
  // Choices a publish there offers (a blog: draft or published), the first value by default.
  const offered = Object.entries(own?.options ?? {}).filter(([, s]) => s.enum?.length);
  // Somewhere whose publish needs what only Seemo writes (a site's source files, as a pull request): the button asks Seemo instead.
  const prepared = own?.prepared ?? [];
  const askSeemo = async () => {
    const c = await api.createChat(slug, `Publish ${a.title}`);
    openSeemo(c.id);
    await api.sendMessage(slug, c.id, `Publish "${a.title}" to ${ownText}: prepare what it needs (${prepared.join(", ")}) the way the site is built and propose the publish.`);
  };

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
  const approveSpot = (spot: Spot) => void run("approve", () => api.approveAsset(slug, a, { spot }));
  const spotForm = mode === "spot" && <SpotForm kinds={kinds} busy={!!busy} onApprove={approveSpot} onCancel={() => setMode("")} />;
  const elsewhere = canSpot && mode !== "spot" && (
    <button className="btn t sm" disabled={!!busy} onClick={() => setMode("spot")}>
      <MapPin size={14} /> Somewhere else…
    </button>
  );

  const scheduleRow = mode === "schedule" && (
    <div className="aa-row">
      <input className="input sm" type="datetime-local" value={at} onChange={(e) => setAt(e.target.value)} />
      <button className="btn p sm" disabled={!!busy || !at} onClick={() => void run("schedule", () => api.scheduleAsset(slug, a, new Date(at).toISOString(), {}, opts))}>
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
    body = !view.data ? null : publishing.length === 0 ? (
      <>
        <div className="aa-row">
          <span className="muted sm">
            Approve it once there is somewhere to send it: <Link to={`/p/${slug}/channels#add`}>add a channel</Link>.
          </span>
          {elsewhere}
          {archive}
        </div>
        {spotForm}
      </>
    ) : (
      <>
        <div className="aa-row">
          {mode !== "spot" &&
            (fits.length === 1 ? (
              <button className="btn p sm" disabled={!!busy || !fits[0].readiness.ready} title={fits[0].readiness.ready ? channelText(fits[0]) : `${fits[0].label}: ${fits[0].readiness.text}`} onClick={() => void run("approve", () => api.approveAsset(slug, a, { target: fits[0].id }))}>
                {spin("approve", Check)} Approve for {fits[0].label}
              </button>
            ) : (
              <>
                <ChannelPicker channels={fits} value={chosenId} onChange={setPick} />
                <button className="btn p sm" disabled={!!busy || !chosen?.readiness.ready} onClick={() => chosen && void run("approve", () => api.approveAsset(slug, a, { target: chosen.id }))}>
                  {spin("approve", Check)} Approve
                </button>
              </>
            ))}
          {elsewhere}
          {archive}
        </div>
        {fits.length > 0 && usable.length === 0 && mode !== "spot" && (
          <p className="muted sm">
            {fits.length === 1 ? `${fits[0].readiness.text}.` : "None of these can be published to right now."} <Link to={`/p/${slug}/channels${fits.length === 1 ? `#channel=${encodeURIComponent(fits[0].id)}` : ""}`}>See to it</Link>.
          </p>
        )}
        {spotForm}
      </>
    );
  } else if (a.status === "ready" || a.status === "scheduled") {
    const others = publishing.filter((e) => e.id !== a.target);
    body = (
      <>
        <div className="aa-row">
          {a.status === "scheduled" && a.publish_at ? (
            <span className="chip ai">
              <CalendarClock size={11} /> {whenAt(a.publish_at)} to {ownText}
            </span>
          ) : (
            <span className="chip acc" title={own ? channelText(own) : undefined}>
              <Check size={11} /> Approved for {ownText}
            </span>
          )}
          {mode === "where" ? (
            <>
              {others.length > 0 ? <ChannelPicker channels={others} value={others.some((e) => e.id === chosenId) ? chosenId : others[0].id} onChange={setPick} /> : <span className="muted sm">No other channel publishes this.</span>}
              {others.length > 0 && (
                <button
                  className="btn g sm"
                  disabled={!!busy}
                  onClick={() => {
                    const to = others.some((e) => e.id === chosenId) ? chosenId : others[0].id;
                    void run("approve", () => api.approveAsset(slug, a, { target: to }));
                  }}
                >
                  {spin("approve", Check)} Use it
                </button>
              )}
              {elsewhere}
              <button className="btn t sm" onClick={() => setMode("")}>
                Cancel
              </button>
            </>
          ) : mode === "spot" ? null : (
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
                <button className="btn p sm" disabled={!!busy} onClick={() => void run("publish", () => api.publishAsset(slug, a, {}, "", opts))}>
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
              {(others.length > 0 || canSpot) && (
                <button className="btn t sm" disabled={!!busy} onClick={() => (setPick(""), setMode("where"))}>
                  Change where
                </button>
              )}
              {archive}
            </>
          )}
        </div>
        {scheduleRow}
        {spotForm}
      </>
    );
  } else {
    body = (
      <>
        <ul className="lib-pubs">
          {a.publications.map((p, i) => (
            <li key={i}>
              <Send size={12} />
              <span>{p.to ?? entries.find((e) => e.id === p.channel)?.label ?? p.channel ?? p.account}</span>
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
