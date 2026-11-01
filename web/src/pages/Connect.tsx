// Connect an account: pick the service (grouped as on the Accounts page,
// from what is installed and what the marketplace has; one tile per service,
// however many abilities it has), which installs every ability of it not
// installed yet, sign in once (or give the few keys it needs), pick the
// places it needs (the GA4 property, the Search Console property), then
// choose what Seemo may do there. Done lands on the Accounts page with
// the new card lit. Opened from "Connect an account" and from every place
// that used to send the user to install or add something for a service;
// `?service=` or `?group=` (or the props) narrow it. A service that runs a
// program KIQR installs (a browser) waits for it first; one that signs in
// on that program asks where the password is and signs in, with the live
// view when a person is needed.
import { useEffect, useMemo, useRef, useState } from "react";
import { Link, useNavigate, useParams, useSearchParams } from "react-router-dom";
import { AnimatePresence, motion } from "motion/react";
import { AlertCircle, ArrowLeft, Check, Download, Hand, KeyRound, Loader2, LogIn, Plus, RefreshCw, Terminal } from "lucide-react";
import { api, useAsync, useEvents, PROGRAM_EVENTS, SERVICE_GROUPS, type Ability, type Account, type KindInfo, type PlaceKind, type ProgramStatus } from "../api";
import { ConnectButton } from "./ConnectButton";
import { AccountKeys, ACCOUNT_EVENTS, accountIdentity, accountService, byGroup, chosenLabel, Fields, LoginEditor, ownLabel, permissionSummary, PermissionMatrix, placeFields, PlaceFieldsForm, placeKinds, placeNoun, placeReady, placesOf, sentence, serviceOf, sessionChip, signsInByHand, useSession, type Field } from "./Accounts";
import { Hands, startUrlOf } from "./Hands";
import { LogPane, SystemProgram, useMachine, useProgramLog } from "./Programs";
import { isService, serviceIcon } from "./abilityBits";
import "../profile.css";
import "../settings.css";
import "../accounts.css";

/** One service in the grid: its abilities here and in the marketplace. */
type Service = { id: string; name: string; icon?: string; group: string; avatar: string | null; installed: Ability[]; market: Ability[] };

/** The ability an account of the service is made with: one that publishes first, then one that signs in with a button. */
function primary(list: Ability[]): Ability | undefined {
  return [...list].sort((a, b) => score(b) - score(a))[0];
}
function score(a: Ability) {
  return (a.channels.length ? 4 : 0) + (a.connect ? 2 : 0) + (a.sources.length ? 1 : 0) - (a.paused ? 8 : 0);
}

/** What a service's tile and sign-in say: the one ability's summary, or the names of its several. */
function serviceSummary(s: Service): string {
  const all = [...s.installed, ...s.market];
  return all.length > 1 ? all.map((a) => a.name).join(" · ") : (all[0]?.summary ?? "");
}

/** A short name from the first web address among the values: "whitening.se". */
function hostOf(values: string[]): string {
  for (const v of values) {
    try {
      const u = new URL(v.trim());
      if (u.hostname) return u.hostname.replace(/^www\./, "");
    } catch {
      /* not a URL */
    }
  }
  return "";
}

/** The system guessed from the browser, only for a daemon too old to say its own: `macos`, `windows` or `linux`. */
function guessOS(): string {
  const p = `${navigator.platform ?? ""} ${navigator.userAgent ?? ""}`.toLowerCase();
  if (/mac|iphone|ipad/.test(p)) return "macos";
  if (/win/.test(p)) return "windows";
  return "linux";
}
const OS_NAME: Record<string, string> = { macos: "macOS", windows: "Windows", linux: "Linux" };

/** Whether a service runs here: an ability without `platforms` runs everywhere; one of them must list this system. */
function runsHere(s: Service, os: string): boolean {
  const all = [...s.installed, ...s.market];
  return !all.length || all.some((a) => !a.platforms?.length || a.platforms.includes(os));
}

export function ConnectPage({ service: givenService, group: givenGroup }: { service?: string; group?: string }) {
  const { slug = "" } = useParams();
  const [params] = useSearchParams();
  const navigate = useNavigate();
  const data = useAsync(() => api.abilities(slug), [slug]);
  const accounts = useAsync(() => api.accounts(slug).catch(() => ({ connections: [] as Account[] })), [slug]);
  useEvents(null, (e) => ["extension_installed", "extension_removed", "marketplace_refreshed"].includes(e.type) && data.reload(), { demo: false });
  const wantService = givenService ?? params.get("service") ?? "";
  const [group, setGroup] = useState(givenGroup ?? params.get("group") ?? "all");
  const [picked, setPicked] = useState<string | null>(wantService || null);
  const [created, setCreated] = useState<string | null>(null);
  const [startUrl, setStartUrl] = useState<string | undefined>(undefined);
  const [signedIn, setSignedIn] = useState(false);
  const [placed, setPlaced] = useState(false);
  // Installing a service's abilities, one after the other: which, and how many are done.
  const [installing, setInstalling] = useState<{ id: string; names: string[]; done: number } | null>(null);
  const tried = useRef<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [more, setMore] = useState(false);

  const services = useMemo(() => {
    const out = new Map<string, Service>();
    const add = (a: Ability, installed: boolean) => {
      if (!isService(a)) return;
      const s = serviceOf(a);
      const cur = out.get(s.id) ?? { id: s.id, name: s.name, icon: s.icon, group: s.group, avatar: a.avatar ?? null, installed: [], market: [] };
      (installed ? cur.installed : cur.market).push(a);
      if (!cur.avatar && a.avatar) cur.avatar = a.avatar;
      out.set(s.id, cur);
    };
    for (const a of data.data?.abilities ?? []) add(a, true);
    for (const a of data.data?.market ?? []) add(a, false);
    return [...out.values()].sort((a, b) => a.name.localeCompare(b.name));
  }, [data.data]);
  const service = services.find((s) => s.id === picked || s.installed.some((a) => a.id === picked) || s.market.some((a) => a.id === picked)) ?? null;
  const have = (id: string) => (accounts.data?.connections ?? []).filter((c) => accountService(c).id === id).length;
  const shown = services.filter((s) => group === "all" || s.group === group);
  const groups = byGroup(shown, (s) => s.group);
  const present = SERVICE_GROUPS.filter((g) => services.some((s) => s.group === g.id));
  // What runs here depends on the daemon's machine, not the device showing the page.
  const machine = useMachine();
  const os = machine ? machine.os || guessOS() : "";

  /** A tile: every ability of the service the marketplace has and this machine does not is installed first, then the sign-in. */
  const choose = async (s: Service) => {
    tried.current = s.id;
    setError(null);
    setMore(false);
    const todo = [...s.market].sort((a, b) => score(b) - score(a));
    if (!todo.length) return setPicked(s.id);
    setInstalling({ id: s.id, names: todo.map((a) => a.name), done: 0 });
    try {
      for (const [i, ab] of todo.entries()) {
        await api.installAbility(ab.id);
        setInstalling((cur) => cur && { ...cur, done: i + 1 });
      }
      data.reload();
      setPicked(s.id);
    } catch (e) {
      setError((e as Error).message);
      data.reload();
    } finally {
      setInstalling(null);
    }
  };
  // Landed on a service with something still to install (an entry point named it): install it first, as a tile click would.
  useEffect(() => {
    if (service && service.market.length && !installing && !error && tried.current !== service.id) void choose(service);
  }, [service?.id, service?.market.length]);
  const step = created && signedIn ? (placed ? 4 : 3) : created || (service?.installed.length && !installing) ? 2 : 1;
  const done = (id: string) => navigate(`/p/${slug}/accounts#account=${encodeURIComponent(id)}`);

  return (
    <div className="prof">
      <div className="cm-head">
        <div>
          <p className="cap">Setup</p>
          <h1 className="d2">Connect an account</h1>
          <p className="muted">Where you post, the address you send from, your site, your numbers. Seemo signs in to it for this project only, and asks before it acts there until you say otherwise.</p>
        </div>
        <Link className="btn t" to={`/p/${slug}/accounts`}>
          <ArrowLeft size={14} /> Accounts
        </Link>
      </div>
      <div className="cw-steps" aria-label="Steps">
        {["Service", "Sign in", "Places", "What Seemo may do"].map((t, i) => (
          <span key={t} className={step === i + 1 ? "on" : step > i + 1 ? "done" : ""}>
            {step > i + 1 ? <Check size={11} /> : <b className="mono">{i + 1}</b>} {t}
          </span>
        ))}
      </div>
      {error && <p className="err">{error}</p>}

      {step === 1 && installing && <Installing names={installing.names} done={installing.done} />}
      {step === 1 && !installing && (
        <>
          {present.length > 1 && (
            <div className="cw-filter">
              <button className={`chip ${group === "all" ? "acc" : ""}`} onClick={() => setGroup("all")}>
                All
              </button>
              {present.map((g) => (
                <button key={g.id} className={`chip ${group === g.id ? "acc" : ""}`} onClick={() => setGroup(g.id)}>
                  {g.label}
                </button>
              ))}
            </div>
          )}
          {!data.data && <Loader2 size={16} className="spin" />}
          <div className="acx-groups">
            {groups.map((g) => (
              <section key={g.id} className="acx-group">
                <h2>{g.label}</h2>
                <div className="cw-grid">
                  {g.items.map((s) => (
                    <Tile key={s.id} s={s} count={have(s.id)} os={os} onPick={() => void choose(s)} />
                  ))}
                </div>
              </section>
            ))}
            {data.data && (
              <section className="acx-group">
                {groups.length > 0 && <h2>Something else</h2>}
                <div className="cw-grid">
                  <button className="cw-tile more" onClick={() => setMore((m) => !m)}>
                    <span className="ic">
                      <Plus size={17} />
                    </span>
                    <b>Add another service</b>
                    <small>{more ? "Connecting a service the marketplace does not have yet is coming in the next release. Until then an ability folder or git URL installs under Abilities → Advanced." : "One that is not listed here."}</small>
                  </button>
                </div>
              </section>
            )}
          </div>
        </>
      )}

      {step === 2 && service && !created && (
        <SignIn
          key={service.id}
          slug={slug}
          service={service}
          onBack={() => (setPicked(null), setError(null))}
          onCreated={(id, url) => (setStartUrl(url), setCreated(id))}
          onPrograms={data.reload}
        />
      )}
      {step === 2 && created && <SessionStep slug={slug} id={created} startUrl={startUrl} onDone={() => setSignedIn(true)} />}
      {step === 3 && created && <PlaceSteps slug={slug} id={created} onDone={() => setPlaced(true)} />}
      {step === 4 && created && <Permissions slug={slug} id={created} onDone={() => done(created)} />}
    </div>
  );
}

/** The service's abilities going on to this machine, one line each. */
function Installing({ names, done }: { names: string[]; done: number }) {
  return (
    <div className="pcard cw-panel">
      <p className="muted sm">Installing what Seemo needs for this service, once for this machine:</p>
      <ul className="cw-prog">
        {names.map((n, i) => (
          <li key={n} className={i < done ? "done" : ""}>
            {i < done ? <Check size={12} /> : i === done ? <Loader2 size={12} className="spin" /> : <span style={{ width: 12 }} />} {n}
          </li>
        ))}
      </ul>
    </div>
  );
}

function Tile({ s, count, os, onPick }: { s: Service; count: number; os: string; onPick: () => void }) {
  const Icon = serviceIcon(s.icon, s.group);
  const [broken, setBroken] = useState(false);
  const ab = primary(s.installed.length ? s.installed : s.market);
  // A program it runs that is not on this computer: it can be installed, but not run until it is.
  const here = !os || runsHere(s, os);
  const note = !here ? `Not on ${OS_NAME[os] ?? os} yet` : ab?.program_found === false ? "Needs a program this computer does not have yet" : !s.installed.length && ab?.runs_program ? "Runs a program on this computer" : null;
  return (
    <button className={`cw-tile ${count ? "have" : ""} ${here ? "" : "unsupported"}`} onClick={onPick} disabled={!here} title={here ? undefined : `${s.name} does not run on ${OS_NAME[os] ?? os} yet`}>
      <span className="ic">{s.avatar && !broken ? <img src={s.avatar} alt="" onError={() => setBroken(true)} /> : <Icon size={17} />}</span>
      <b>{s.name}</b>
      <small className={s.installed.length + s.market.length > 1 ? "parts" : ""}>{serviceSummary(s)}</small>
      {note && (
        <small className="note">
          <Terminal size={11} /> {note}
        </small>
      )}
      {count ? <span className="chip sm ok">{count === 1 ? "Connected" : `${count} connected`}</span> : null}
    </button>
  );
}

/** Step two: the service's way in. A sign-in button when it has one, else as few fields as possible; the first place when it needs one. */
function SignIn({ slug, service, backLabel = "Another service", onBack, onCreated, onPrograms }: { slug: string; service: Service; backLabel?: string; onBack: () => void; onCreated: (id: string, startUrl?: string) => void; onPrograms?: () => void }) {
  // The one-button sign-in is always the way in when any ability of the service has one; typed keys are the last resort.
  const ab = service.installed.filter((a) => a.connect).sort((a, b) => score(b) - score(a))[0] ?? primary(service.installed);
  const Icon = serviceIcon(service.icon, service.group);
  // The kind it publishes as: this ability's, else another of the service's (the one signing in may only read).
  const kind: KindInfo | undefined = ab?.kinds?.[0] ?? service.installed.flatMap((a) => a.kinds ?? [])[0];
  // The first place, only when nothing can go out without one, and only what is typed: a field picked from a list read through the account waits for the places step.
  const place = placeFields(Object.fromEntries(Object.entries(kind?.place ?? {}).filter(([, f]) => !f.optional && !f.choices)));
  const [mode, setMode] = useState<"connect" | "keys">(ab?.connect ? "connect" : "keys");
  const [values, setValues] = useState<Record<string, string>>({});
  const [label, setLabel] = useState("");
  const [moreKeys, setMoreKeys] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [started, setStarted] = useState<{ id: string; paste: boolean } | null>(null);
  const [account, setAccount] = useState<Account | null>(null);
  const reload = async (id: string) => setAccount(await api.account(slug, id).catch(() => null));
  // The programs the service runs (a browser) are installed when its abilities are; until they are here, that comes first.
  const [programsReady, setProgramsReady] = useState(false);
  const needPrograms = service.installed.filter((a) => (a.programs ?? []).some((p) => !p.found));
  useEvents(slug, (e) => started && ACCOUNT_EVENTS.includes(e.type) && void reload(started.id), { demo: false });
  useEffect(() => {
    if (started) void reload(started.id);
  }, [started?.id]);
  if (!ab)
    return (
      <div className="pcard cw-panel">
        <p className="muted">Nothing for {service.name} is installed yet.</p>
        <div className="acts">
          <button className="btn t" onClick={onBack}>
            <ArrowLeft size={14} /> Back
          </button>
        </div>
      </div>
    );
  if (needPrograms.length && !programsReady)
    return <ProgramsStep service={service.name} abilities={needPrograms} onReady={() => (setProgramsReady(true), onPrograms?.())} onBack={onBack} backLabel={backLabel} />;
  const keyField = (k: Ability["auth"][number]): Field => ({ key: k.env, label: k.label || "Key", help: k.help, placeholder: k.placeholder, optional: k.optional, options: k.options, secret: /PASSWORD|KEY|TOKEN|SECRET/.test(k.env) });
  // With a button: only what it asks first. With keys: the required ones, the rest behind More.
  const asked = mode === "connect" ? ab.auth.filter((k) => ab.connect?.needs.includes(k.env)) : ab.auth.filter((k) => !k.optional);
  const extra = mode === "keys" ? ab.auth.filter((k) => k.optional) : [];
  const fields = [...place, ...asked.map(keyField)];
  const placeValues = Object.fromEntries(place.map((f) => [f.key, values[f.key] ?? ""]).filter(([, v]) => v.trim()));
  const ok = fields.every((f) => f.optional || (values[f.key] ?? "").trim());
  const name = label.trim() || hostOf(Object.values(values));
  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      const secrets = Object.fromEntries(ab.auth.map((k) => [k.env, values[k.env] ?? ""]).filter(([, v]) => v.trim()));
      const c = await api.createAccount(slug, { ability: ab.id, label: name || undefined, kind: kind?.kind, place: Object.keys(placeValues).length ? placeValues : undefined, secrets });
      onCreated(c.id, startUrlOf(...Object.values(values)));
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="pcard cw-panel">
      <div className="cw-head">
        <span className="ic">{service.avatar ? <img src={service.avatar} alt="" /> : <Icon size={19} />}</span>
        <div>
          <h2>{service.name}</h2>
          <p className="muted sm">{serviceSummary(service)}</p>
        </div>
      </div>
      {started ? (
        <>
          {account?.ready ? (
            <p className="ok sm">
              <Check size={13} /> Connected{accountIdentity(account) ? ` as ${accountIdentity(account)}` : ""}.
            </p>
          ) : account?.signed_in ? (
            <p className="muted sm">Signed in. One more step: choose below.</p>
          ) : started.paste ? (
            <p className="muted sm">Approve in the tab that opened. It shows what KIQR needs once, because it only sends it back over https: paste it below.</p>
          ) : (
            <p className="muted sm">
              <Loader2 size={12} className="spin" /> Waiting for you to approve in the tab that opened.
            </p>
          )}
          {account && !account.ready && <AccountKeys slug={slug} a={account} only={started.paste || account.signed_in ? "missing" : undefined} onChange={() => void reload(started.id)} />}
          <div className="acts">
            <button className={`btn ${account?.ready ? "p" : "g"}`} onClick={() => onCreated(started.id)}>
              {account?.ready ? "Next" : "Next, finish this later"}
            </button>
          </div>
        </>
      ) : (
        <>
          <Fields fields={fields} values={values} onChange={(k, v) => setValues({ ...values, [k]: v })} />
          {moreKeys && (
            <>
              <label className="acc-field">
                <span className="nm">
                  Name<small className="dim"> optional</small>
                </span>
                <input className="input" value={label} placeholder={hostOf(Object.values(values)) || service.name} onChange={(e) => setLabel(e.target.value)} aria-label="Name" />
              </label>
              <Fields fields={extra.map(keyField)} values={values} onChange={(k, v) => setValues({ ...values, [k]: v })} />
            </>
          )}
          {error && <p className="err sm">{error}</p>}
          <div className="acts">
            {mode === "connect" && ab.connect ? (
              <ConnectButton slug={slug} ability={ab.id} connect={ab.connect} auth={ab.auth.map((k) => ({ ...k, set: null }))} values={values} disabled={!ok} fresh={{ label: name || undefined, kind: kind?.kind, place: Object.keys(placeValues).length ? placeValues : undefined }} onStarted={(id, paste) => setStarted({ id, paste })} />
            ) : (
              <button className="btn p" disabled={!ok || busy} onClick={() => void save()}>
                {busy ? <Loader2 size={14} className="spin" /> : <Check size={14} />} Connect
              </button>
            )}
            <button className="btn t" onClick={onBack}>
              <ArrowLeft size={14} /> {backLabel}
            </button>
          </div>
          <p className="acc-links">
            {mode === "connect" && ab.auth.length > 0 && (
              <button className="lnk-btn" onClick={() => setMode("keys")}>
                Enter the keys yourself
              </button>
            )}
            {mode === "keys" && ab.connect && (
              <button className="lnk-btn" onClick={() => setMode("connect")}>
                {ab.connect.label} instead
              </button>
            )}
            {!moreKeys && (
              <button className="lnk-btn" onClick={() => setMoreKeys(true)}>
                More
              </button>
            )}
          </p>
        </>
      )}
    </div>
  );
}

/**
 * The wizard's last two steps in place, for one ability's service (the
 * ability window, which the welcome flow opens before the app is): sign in,
 * then what Seemo may do. One not installed yet is installed first.
 */
export function ConnectService({ slug, ability, onInstalled, onDone, onCancel }: { slug: string; ability: Ability; onInstalled: () => void; onDone: (id: string) => void; onCancel: () => void }) {
  const svc = serviceOf(ability);
  const [created, setCreated] = useState<string | null>(null);
  const [startUrl, setStartUrl] = useState<string | undefined>(undefined);
  const [signedIn, setSignedIn] = useState(false);
  const [placed, setPlaced] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  if (!ability.installed)
    return (
      <div className="pcard cw-panel">
        <p className="muted sm">First KIQR installs {ability.name} on this machine; then you sign in to {svc.name}.</p>
        {error && <p className="err sm">{error}</p>}
        <div className="acts">
          <button className="btn p sm" disabled={busy} onClick={() => (setBusy(true), setError(null), api.installAbility(ability.id).then(onInstalled, (e) => setError((e as Error).message)).finally(() => setBusy(false)))}>
            {busy ? <Loader2 size={13} className="spin" /> : <Check size={13} />} Install and continue
          </button>
          <button className="btn t sm" onClick={onCancel}>
            Cancel
          </button>
        </div>
      </div>
    );
  if (created && !signedIn) return <SessionStep slug={slug} id={created} startUrl={startUrl} onDone={() => setSignedIn(true)} />;
  if (created && !placed) return <PlaceSteps slug={slug} id={created} onDone={() => setPlaced(true)} />;
  if (created) return <Permissions slug={slug} id={created} onDone={() => onDone(created)} />;
  return <SignIn slug={slug} service={{ id: svc.id, name: svc.name, icon: svc.icon, group: svc.group, avatar: ability.avatar ?? null, installed: [ability], market: [] }} backLabel="Cancel" onBack={onCancel} onCreated={(id, url) => (setStartUrl(url), setCreated(id))} onPrograms={onInstalled} />;
}

/** Step three: what Seemo may do there, with the defaults the account started with. Saved as it is changed. */
function Permissions({ slug, id, onDone }: { slug: string; id: string; onDone: () => void }) {
  const [a, setA] = useState<Account | null>(null);
  const load = () => api.account(slug, id).then(setA, () => setA(null));
  useEffect(() => {
    void load();
  }, [slug, id]);
  return (
    <AnimatePresence>
      <motion.div className="pcard cw-panel" initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }}>
        {!a ? (
          <Loader2 size={16} className="spin" />
        ) : (
          <>
            <div className="cw-head">
              <div>
                <h2>
                  What Seemo may do on {accountService(a).name}
                  {accountIdentity(a) ? ` (${accountIdentity(a)})` : ""}
                </h2>
                <p className="muted sm">{permissionSummary(a)}. Change it here or on the account at any time.</p>
              </div>
            </div>
            <PermissionMatrix slug={slug} a={a} onChange={() => void load()} />
            <p className="muted sm">“Ask me first” puts a card in the chat and waits for your yes. “On its own” still puts the card there, as the record.</p>
            <div className="acts">
              <button className="btn p" onClick={onDone}>
                <Check size={14} /> Done
              </button>
            </div>
          </>
        )}
      </motion.div>
    </AnimatePresence>
  );
}


/**
 * Step three: one question per kind of place the account still needs ("Which GA4 property?",
 * "Which Search Console property?"), each picked from what the account has. Skipped when none is
 * needed. With exactly one place and no name of its own, the account is named after it.
 */
function PlaceSteps({ slug, id, onDone }: { slug: string; id: string; onDone: () => void }) {
  const [a, setA] = useState<Account | null>(null);
  const [todo, setTodo] = useState<PlaceKind[] | null>(null);
  const [i, setI] = useState(0);
  const [values, setValues] = useState<Record<string, string>>({});
  const [read, setRead] = useState<Record<string, { value: string; label: string }[]>>({});
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    api.account(slug, id).then(
      (acc) => {
        setA(acc);
        const need = placeKinds(acc).filter((k) => k.needs && !k.lists && !placesOf(acc, k).length && Object.keys(k.fields ?? {}).length > 0);
        setTodo(need);
        if (!need.length) onDone();
      },
      () => onDone(),
    );
  }, [slug, id]);
  const finish = async (acc: Account | null) => {
    if (acc && !ownLabel(acc) && (acc.places ?? []).length === 1) await api.updateAccount(slug, id, { label: acc.places![0].label }).catch(() => undefined);
    onDone();
  };
  const next = async (acc: Account | null) => {
    setValues({});
    setRead({});
    setError(null);
    if (todo && i + 1 < todo.length) setI(i + 1);
    else await finish(acc);
  };
  const k = todo?.[i];
  if (!a || !k) return <Loader2 size={16} className="spin" />;
  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      const r = await api.addPlace(slug, id, { kind: k.kind, fields: values, label: chosenLabel(k.fields, values, read) });
      setA(r);
      await next(r);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  return (
    <motion.div key={k.kind} className="pcard cw-panel" initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }}>
      <div className="cw-head">
        <div>
          <h2>Which {placeNoun(k)}?</h2>
          <p className="muted sm">
            {k.verb === "Measures" ? "Seemo reads the numbers of the one you pick, once a day, into Performance." : k.lists ? "Who Seemo sends to." : "Where Seemo works on your account."}
            {todo!.length > 1 ? ` ${i + 1} of ${todo!.length}.` : ""}
          </p>
        </div>
      </div>
      <PlaceFieldsForm slug={slug} account={id} fields={k.fields} values={values} onChange={(f, v) => setValues((cur) => ({ ...cur, [f]: v }))} onChoices={(f, c) => setRead((cur) => ({ ...cur, [f]: c }))} />
      {error && <p className="err sm">{error}</p>}
      <div className="acts">
        <button className="btn p" disabled={busy || !placeReady(k.fields, values)} onClick={() => void save()}>
          {busy ? <Loader2 size={14} className="spin" /> : <Check size={14} />} Use it
        </button>
        <button className="btn t" disabled={busy} onClick={() => void next(a)}>
          Pick it later
        </button>
      </div>
    </motion.div>
  );
}

type ProgramLine = { program: string; version: string; ability: string; p: ProgramStatus; state: "waiting" | "installing" | "installed" | "failed"; note: string };

/**
 * "Getting the browser ready": the programs the service runs, which KIQR
 * installs on this machine once the user says so. Nothing downloads before
 * "Install now": the step names what would be installed and where. Then the
 * progress comes from `program_*` events, with the installer's own output in
 * a log; Try again asks once more. A system package (Xvfb) shows its command,
 * and "Install it for me" when the desktop can ask for the password. Coming
 * back to the page while an install runs picks it up where it is.
 */
function ProgramsStep({ service, abilities, onReady, onBack, backLabel }: { service: string; abilities: Ability[]; onReady: () => void; onBack: () => void; backLabel: string }) {
  const line = (ability: string, p: ProgramStatus): ProgramLine => ({ program: p.program, version: p.version, ability, p, state: p.found ? "installed" : "waiting", note: "" });
  const [lines, setLines] = useState<ProgramLine[]>(() => {
    const seen = new Map<string, ProgramLine>();
    for (const a of abilities) for (const p of a.programs ?? []) if (!seen.has(p.program) || !p.found) seen.set(p.program, line(a.id, p));
    return [...seen.values()];
  });
  const [started, setStarted] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const set = (program: string, f: (l: ProgramLine) => ProgramLine) => setLines((cur) => cur.map((l) => (l.program === program ? f(l) : l)));
  const system = (l: ProgramLine) => l.state !== "installed" && l.p.how === "system";
  const log = useProgramLog(lines.map((l) => l.program), started);
  // An install already running (the page was left and opened again) is shown as it is, not asked about again.
  useEffect(() => {
    let gone = false;
    api.programLog().then((r) => {
      if (gone || !lines.some((l) => r.programs.includes(l.program))) return;
      setStarted(true);
      setLines((cur) => cur.map((l) => (l.state === "waiting" && !system(l) ? { ...l, state: "installing" } : l)));
    }, () => {});
    return () => {
      gone = true;
    };
  }, []);
  /** A fresh status from the daemon: what is found now, and the commands as it gives them. */
  const fresh = (list: ProgramStatus[]) => {
    for (const p of list) set(p.program, (l) => ({ ...l, p, ...(p.found ? { state: "installed" as const, note: "" } : {}) }));
  };
  useEvents(
    null,
    (e) => {
      if (!PROGRAM_EVENTS.includes(e.type) || e.type === "program_log") return;
      const program = String(e.program ?? "");
      if (e.type === "program_installing") (setStarted(true), set(program, (l) => ({ ...l, state: "installing", note: String(e.note ?? "") })));
      if (e.type === "program_installed") set(program, (l) => ({ ...l, state: "installed", note: "" }));
      if (e.type === "program_failed") set(program, (l) => ({ ...l, state: "failed", note: String(e.message ?? "") }));
    },
    { demo: false },
  );
  const done = lines.every((l) => l.state === "installed");
  useEffect(() => {
    if (done) onReady();
  }, [done]);
  const install = async () => {
    setStarted(true);
    setBusy(true);
    setError(null);
    setLines((cur) => cur.map((l) => (l.state === "installed" || system(l) ? l : { ...l, state: "installing", note: "" })));
    try {
      for (const a of abilities) fresh((await api.installPrograms(a.id)).programs ?? []);
    } catch (e) {
      setError((e as Error).message);
      setLines((cur) => cur.map((l) => (l.state === "installing" ? { ...l, state: "failed" } : l)));
    } finally {
      setBusy(false);
    }
  };
  const failed = lines.some((l) => l.state === "failed" && !system(l));
  const own = lines.filter((l) => l.state !== "installed" && !system(l));
  const installing = lines.some((l) => l.state === "installing");
  return (
    <div className="pcard cw-panel">
      <div className="cw-head">
        <div>
          <h2>Getting the browser ready</h2>
          {!started ? (
            <p className="muted sm">
              {service} runs in a real browser on this machine. Before it can, KIQR has to install {own.length === 1 ? "one program" : `${own.length} programs`} into its own folder (<span className="mono">~/.kiqr/bin</span>), nothing elsewhere on your system. They are pinned to one version and checked before anything is unpacked. It takes a minute or two, and nothing is downloaded until you say so.
            </p>
          ) : (
            <p className="muted sm">KIQR is installing what {service} runs on this machine, once. You can leave this page and come back.</p>
          )}
        </div>
      </div>
      <ul className="cw-prog">
        {lines.map((l) => (
          <li key={l.program} className={l.state === "installed" ? "done" : l.state === "failed" && !system(l) ? "bad" : ""}>
            {l.state === "installed" ? <Check size={12} /> : l.state === "installing" ? <Loader2 size={12} className="spin" /> : <AlertCircle size={12} />}
            <span>
              {l.program} <span className="mono dim">{l.version}</span>
              {system(l) ? <span className="note"> · from your system</span> : l.p.how === "none" && l.state !== "installed" ? <span className="note"> · no way to install it here</span> : null}
              {!system(l) && l.state === "waiting" && <span className="note"> · not installed yet</span>}
              {!system(l) && l.note && <span className="note"> · {l.note}</span>}
            </span>
          </li>
        ))}
      </ul>
      {lines.filter(system).map((l) => (
        <SystemProgram key={l.program} ability={l.ability} p={l.p} onDone={fresh} />
      ))}
      {error && <p className="err sm">{error}</p>}
      <div className="acts">
        {own.length > 0 && (
          <button className={`btn ${!started || failed ? "p" : "t"} sm`} disabled={busy || installing} onClick={() => void install()}>
            {busy || installing ? <Loader2 size={13} className="spin" /> : !started ? <Download size={13} /> : <RefreshCw size={13} />} {!started ? "Install now" : installing ? "Installing…" : failed ? "Try again" : "Check again"}
          </button>
        )}
        <button className="btn t sm" onClick={onBack}>
          <ArrowLeft size={13} /> {backLabel}
        </button>
      </div>
      <LogPane lines={log} />
    </div>
  );
}

/**
 * For an account that signs in on a hosted program: where the password is
 * (saved in KIQR, or "I'll sign in myself"), then Sign in and how it went;
 * "Needs a hand" opens the live view. Skipped for one that does not.
 */
function SessionStep({ slug, id, startUrl, onDone }: { slug: string; id: string; startUrl?: string; onDone: () => void }) {
  const [a, setA] = useState<Account | null>(null);
  const [phase, setPhase] = useState<"where" | "kiqr" | "signin">("where");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [hands, setHands] = useState(false);
  const load = () =>
    api.account(slug, id).then(
      (acc) => {
        setA(acc);
        if (!acc.has_session) onDone();
      },
      () => onDone(),
    );
  useEffect(() => {
    void load();
  }, [slug, id]);
  useEvents(slug, (e) => ACCOUNT_EVENTS.includes(e.type) && e.connection === id && void load(), { demo: false });
  const [st, setSt] = useSession(slug, a ?? { id, session: undefined });
  if (!a?.has_session) return <Loader2 size={16} className="spin" />;
  const byHand = signsInByHand(a);
  const name = accountIdentity(a) || accountService(a).name;
  const signIn = async () => {
    setBusy(true);
    setError(null);
    try {
      const r = await api.accountSignIn(slug, id);
      if (r.started) setSt({ ...(st ?? { message: "", checked_at: null, signed_in_at: null }), state: "checking", message: "" });
      else if (r.session) setSt(r.session);
      void load();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  const state = st?.state ?? "unknown";
  const chip = sessionChip(st);
  const ask = phase === "where" && !byHand && a.login?.store !== "kiqr";
  return (
    <motion.div className="pcard cw-panel" initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }}>
      <div className="cw-head">
        <div>
          <h2>{ask || phase === "kiqr" ? "Where's your password?" : `Sign in to ${name}`}</h2>
          <p className="muted sm">{byHand ? "This site is signed in by hand: you sign in once in the live view, a picture of the browser KIQR runs, and the browser keeps it." : ask || phase === "kiqr" ? "KIQR signs in on its own browser with it, and again whenever the site signs it out. Seemo never sees it." : "KIQR opens the site in its browser and signs in."}</p>
        </div>
      </div>

      {ask && (
        <div className="cw-choice">
          <button onClick={() => setPhase("kiqr")}>
            <b>
              <KeyRound size={13} /> Save it in KIQR
            </b>
            <small>Your user name, password and, if the site asks for one, the code from your authenticator app. Kept on this machine only.</small>
          </button>
          <button onClick={() => setPhase("signin")}>
            <b>
              <Hand size={13} /> I'll sign in myself
            </b>
            <small>Sign in by hand in the live view. When the site signs KIQR out, it asks you again.</small>
          </button>
        </div>
      )}
      {phase === "kiqr" && <LoginEditor slug={slug} a={a} ask={false} onChange={() => void load()} onSaved={() => (setPhase("signin"), void signIn())} />}

      {!ask && phase !== "kiqr" && (
        <>
          <p className="sm">
            <span className={`chip sm ${chip.cls}`}>
              {chip.spin ? <Loader2 size={11} className="spin" /> : <b />} {chip.text}
            </span>
          </p>
          {state === "needs_hand" && (
            <div className="acx-hand">
              <p>
                {st?.message ? `${sentence(st.message)}. ` : ""}Finish the sign-in in the live view.
              </p>
            </div>
          )}
          {error && <p className="err sm">{error}</p>}
          <div className="acts">
            {!byHand && state !== "signed_in" && state !== "checking" && (
              <button className={`btn ${state === "needs_hand" ? "g" : "p"}`} disabled={busy} onClick={() => void signIn()}>
                {busy ? <Loader2 size={14} className="spin" /> : <LogIn size={14} />} {state === "needs_hand" ? "Try again" : "Sign in"}
              </button>
            )}
            {a.live_view && state !== "signed_in" && state !== "checking" && (byHand || state === "needs_hand" || a.login?.store !== "kiqr") && (
              <button className={`btn ${byHand || state === "needs_hand" ? "p" : "g"}`} onClick={() => setHands(true)}>
                <Hand size={14} /> Take the controls
              </button>
            )}
            <button className={`btn ${state === "signed_in" ? "p" : "t"}`} onClick={onDone}>
              {state === "signed_in" ? "Next" : "Next, finish this later"}
            </button>
          </div>
        </>
      )}
      {hands && <Hands slug={slug} account={id} name={name} startUrl={startUrl ?? startUrlOf(a.identity, a.destination_text)} onClose={(signedIn) => (setHands(false), signedIn && setSt({ ...(st ?? { message: "", checked_at: null, signed_in_at: null }), state: "checking", message: "" }), void load())} />}
    </motion.div>
  );
}
