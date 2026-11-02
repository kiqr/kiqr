// What you can do with an asset, by where it stands. Nobody sets the status:
// it follows from the facts. Approve picks where it goes, an Audience entry or
// somewhere new that joins the Audience with the yes (Ready), Schedule sets the
// time (Scheduled), Publish sends it (Published). Unpublish, Archive and
// Bring back are the only steps backwards. The same block sits under a post
// in the chat, in the Library's modal, on the asset page and on the To do page.
import { useState } from "react";
import { Link } from "react-router-dom";
import { AlertTriangle, Archive, ArchiveRestore, CalendarClock, Check, ExternalLink, Loader2, MapPin, Send, Undo2 } from "lucide-react";
import { FieldInput } from "./FieldInput";
import { api, SECTIONS, type Asset, type AssetStatus, type AudienceEntry, type AudienceKind, type Spot } from "../api";
import { networkOf, useNetworks } from "../networks";
import { ago } from "./Project";
import { connectHref, entryText, filled, SpotFields, useAudience } from "./Audience";
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

/** Where an asset can go now: the Audience entries that publish, those of the post's network first (all when none fits). */
function fitting(a: Asset, entries: AudienceEntry[], netKind: string | undefined) {
  const kind = netKind ?? a.tags[0]?.toLowerCase();
  const hit = kind ? entries.filter((e) => e.kind === kind) : [];
  return hit.length ? hit : entries;
}

/** A select of Audience entries, grouped by section: "r/selfhosted · via u/acme_dev". */
function EntryPicker({ entries, value, onChange }: { entries: AudienceEntry[]; value: string; onChange: (id: string) => void }) {
  return (
    <select className="input sm" value={value} onChange={(e) => onChange(e.target.value)} aria-label="Where it goes">
      {SECTIONS.map((s) => {
        const of = entries.filter((e) => e.section === s.id);
        return of.length ? (
          <optgroup key={s.id} label={s.label}>
            {of.map((e) => (
              <option key={e.id} value={e.id} disabled={!e.via?.ready}>
                {entryText(e)}
                {!e.via ? " (needs an account)" : !e.via.ready ? " (its account does not work now)" : ""}
              </option>
            ))}
          </optgroup>
        ) : null;
      })}
    </select>
  );
}

/** Somewhere the Audience does not have yet: a kind, the account, the fields that name it. The yes adds it. */
function SpotForm({ slug, kinds, busy, onApprove, onCancel }: { slug: string; kinds: AudienceKind[]; busy: boolean; onApprove: (spot: Spot) => void; onCancel: () => void }) {
  const usable = kinds.filter((k) => k.section !== "list" && k.publishes !== false);
  const [kindId, setKindId] = useState(usable.find((k) => k.accounts.length)?.kind ?? usable[0]?.kind ?? "");
  const kind = usable.find((k) => k.kind === kindId);
  const [account, setAccount] = useState("");
  const [fields, setFields] = useState<Record<string, string>>({});
  const accountId = account || kind?.accounts[0]?.id || "";
  const missing = !kind || !accountId || (!kind.account_is_target && Object.entries(kind.target ?? {}).some(([n, s]) => !s.optional && !(fields[n] ?? "").trim()));
  return (
    <div className="aa-spot">
      <div className="au-form">
        <label className="acc-field">
          <span className="nm">What it is</span>
          <select className="input" value={kindId} onChange={(e) => (setKindId(e.target.value), setAccount(""), setFields({}))}>
            {SECTIONS.filter((s) => s.id !== "list").map((s) => {
              const of = usable.filter((k) => k.section === s.id);
              return of.length ? (
                <optgroup key={s.id} label={s.label}>
                  {of.map((k) => (
                    <option key={k.kind} value={k.kind}>
                      {k.name || k.label}
                    </option>
                  ))}
                </optgroup>
              ) : null;
            })}
          </select>
        </label>
        {kind && kind.accounts.length > 0 && (
          <label className="acc-field">
            <span className="nm">Through</span>
            <FieldInput value={accountId} onChange={setAccount} options={kind.accounts.map((x) => x.id)} labels={Object.fromEntries(kind.accounts.map((x) => [x.id, x.label]))} ariaLabel="Account" plain />
          </label>
        )}
        {kind && kind.accounts.length > 0 && <SpotFields slug={slug} kind={kind} account={accountId} values={fields} onChange={setFields} />}
      </div>
      {kind && kind.accounts.length === 0 && (
        <p className="muted sm">
          <Link to={connectHref(slug, kind.service)}>Connect a {kind.service_name ?? kind.name} account first</Link>
        </p>
      )}
      <div className="aa-row">
        <button className="btn p sm" disabled={busy || missing} onClick={() => kind && onApprove({ kind: kind.kind, account: accountId, fields: filled(fields) })}>
          <Check size={14} /> Approve for it
        </button>
        <button className="btn t sm" onClick={onCancel}>
          Cancel
        </button>
        <span className="new-spot">It joins your Audience.</span>
      </div>
    </div>
  );
}

export function AssetActions({ slug, a, compact, onChange }: { slug: string; a: Asset; compact?: boolean; onChange: () => void }) {
  const view = useAudience(slug);
  const networks = useNetworks(slug);
  const entries = (view.data?.entries ?? []).filter((e) => !e.suggested);
  const publishing = entries.filter((e) => e.publishes);
  const kinds = view.data?.kinds ?? [];
  const canSpot = kinds.some((k) => k.section !== "list" && k.accounts.length > 0);
  const fits = fitting(a, publishing, networkOf(a, networks)?.kind);
  const usable = fits.filter((e) => e.via?.ready);
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
  const spotForm = mode === "spot" && <SpotForm slug={slug} kinds={kinds} busy={!!busy} onApprove={approveSpot} onCancel={() => setMode("")} />;
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
            Approve it once there is somewhere to send it: add a page or community to <Link to={`/p/${slug}/audience`}>your Audience</Link>
            {canSpot ? "." : <>, through an account (<Link to={connectHref(slug)}>connect one</Link>).</>}
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
              <button className="btn p sm" disabled={!!busy || !fits[0].via?.ready} title={fits[0].via?.ready ? entryText(fits[0]) : `${entryText(fits[0])}: its account does not work now`} onClick={() => void run("approve", () => api.approveAsset(slug, a, { target: fits[0].id }))}>
                {spin("approve", Check)} Approve for {fits[0].label}
              </button>
            ) : (
              <>
                <EntryPicker entries={fits} value={chosenId} onChange={setPick} />
                <button className="btn p sm" disabled={!!busy || !chosen?.via?.ready} onClick={() => chosen && void run("approve", () => api.approveAsset(slug, a, { target: chosen.id }))}>
                  {spin("approve", Check)} Approve
                </button>
              </>
            ))}
          {elsewhere}
          {archive}
        </div>
        {fits.length > 0 && usable.length === 0 && mode !== "spot" && (
          <p className="muted sm">
            The account behind {fits.length === 1 ? "it" : "these"} does not work right now. <Link to={`/p/${slug}/accounts`}>See accounts</Link>.
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
            <span className="chip acc" title={own ? entryText(own) : undefined}>
              <Check size={11} /> Approved for {ownText}
            </span>
          )}
          {mode === "where" ? (
            <>
              {others.length > 0 ? <EntryPicker entries={others} value={others.some((e) => e.id === chosenId) ? chosenId : others[0].id} onChange={setPick} /> : <span className="muted sm">Nowhere else in your Audience.</span>}
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
              <span>{p.to ?? entries.find((e) => e.id === p.audience)?.label ?? p.audience ?? p.account}</span>
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
