// Connect an account: pick the service (grouped as on the Accounts page,
// from what is installed and what the marketplace has; one tile per service,
// however many abilities it has), which installs every ability of it not
// installed yet; when the project has accounts of it already, sign one of
// them in again or add another; sign in once (a button, or the few keys it
// needs); pick what its abilities read through it (the GA4 property) when
// they need that; choose what Seemo may do there; and Done says what joined
// the Audience. A one-button sign-in makes no account until it comes back:
// the daemon makes it and `connect_finished` names it. Opened from "Connect
// an account" and from every place that used to send the user to install or
// add something for a service; `?service=` or `?group=` (or the props)
// narrow it. A service that runs a program KIQR installs (a browser) waits
// for it first; one that signs in on that program asks where the password
// is and signs in, with the live view when a person is needed.
import { useEffect, useMemo, useRef, useState } from "react";
import { Link, useNavigate, useParams, useSearchParams } from "react-router-dom";
import { AnimatePresence, motion } from "motion/react";
import { AlertCircle, ArrowLeft, Check, ChevronDown, ChevronRight, Download, Hand, KeyRound, Loader2, LogIn, Plus, RefreshCw, Terminal, Users } from "lucide-react";
import { api, useAsync, useEvents, ACCOUNT_EVENTS, AUDIENCE_EVENTS, PROGRAM_EVENTS, SERVICE_GROUPS, type Ability, type Account, type ProgramStatus } from "../api";
import { ConnectButton } from "./ConnectButton";
import { AccountKeys, accountIdentity, accountService, ActOnOwn, byGroup, Fields, LoginEditor, permissionSummary, PermissionMatrix, sentence, serviceOf, SettingField, settingsPicked, settingValues, signInNow, signsInByHand, StateChip, stateChip, type Field } from "./Accounts";
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
  return ((a.publishes_to ?? []).length ? 4 : 0) + (a.connect ? 2 : 0) + (a.sources.length ? 1 : 0) - (a.paused ? 8 : 0);
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

/** The abilities of a service that pick something once through the account (a GA4 property). */
function measuring(s: Service): Ability[] {
  return s.installed.filter((a) => Object.keys(a.settings ?? {}).length > 0);
}

/** The steps along the top, the current one lit. */
function Steps({ names, at }: { names: string[]; at: number }) {
  return (
    <div className="cw-steps" aria-label="Steps">
      {names.map((t, i) => (
        <span key={t} className={at === i ? "on" : at > i ? "done" : ""}>
          {at > i ? <Check size={11} /> : <b className="mono">{i + 1}</b>} {t}
        </span>
      ))}
    </div>
  );
}

export function ConnectPage({ service: givenService, group: givenGroup }: { service?: string; group?: string }) {
  const { slug = "" } = useParams();
  const [params] = useSearchParams();
  const navigate = useNavigate();
  const data = useAsync(() => api.abilities(slug), [slug]);
  const accounts = useAsync(() => api.accounts(slug).catch(() => ({ accounts: [] as Account[] })), [slug]);
  useEvents(null, (e) => ["extension_installed", "extension_removed", "marketplace_refreshed"].includes(e.type) && data.reload(), { demo: false });
  const wantService = givenService ?? params.get("service") ?? "";
  const [group, setGroup] = useState(givenGroup ?? params.get("group") ?? "all");
  const [picked, setPicked] = useState<string | null>(wantService || null);
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
  const ofService = (id: string) => (accounts.data?.accounts ?? []).filter((c) => accountService(c).id === id);
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
  const open = !!service && service.installed.length > 0 && !installing && !!accounts.data;

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
      {!open && <Steps names={["Service", "Sign in", "What Seemo may do", "Done"]} at={0} />}
      {error && <p className="err">{error}</p>}

      {!open && installing && <Installing names={installing.names} done={installing.done} />}
      {!open && !installing && (
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
                    <Tile key={s.id} s={s} count={ofService(s.id).length} os={os} onPick={() => void choose(s)} />
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

      {open && service && (
        <Flow
          key={service.id}
          slug={slug}
          service={service}
          existing={ofService(service.id)}
          withService
          onBack={() => (setPicked(null), setError(null))}
          onPrograms={data.reload}
          onFinish={(id) => navigate(`/p/${slug}/accounts#account=${encodeURIComponent(id)}`)}
        />
      )}
    </div>
  );
}

type Stage = "again" | "signin" | "session" | "measures" | "allow" | "done";

/**
 * From a picked service to Done: sign one of its accounts in again or add
 * another, sign in, the program's sign-in when it has one, what its
 * abilities read through it, what Seemo may do, and what joined the Audience.
 */
function Flow({ slug, service, existing, withService, backLabel, onBack, onPrograms, onFinish }: { slug: string; service: Service; existing: Account[]; withService?: boolean; backLabel?: string; onBack: () => void; onPrograms?: () => void; onFinish: (id: string) => void }) {
  const measured = measuring(service);
  const [stage, setStage] = useState<Stage>(existing.length ? "again" : "signin");
  const [again, setAgain] = useState<Account | null>(null);
  const [id, setId] = useState<string | null>(null);
  const [startUrl, setStartUrl] = useState<string | undefined>(undefined);
  const names = [...(withService ? ["Service"] : []), "Sign in", ...(measured.length ? ["Measures"] : []), "What Seemo may do", "Done"];
  const cur = stage === "measures" ? "Measures" : stage === "allow" ? "What Seemo may do" : stage === "done" ? "Done" : "Sign in";
  const signedIn = (acc: string, url?: string) => (setId(acc), setStartUrl(url), setStage("session"));
  return (
    <>
      <Steps names={names} at={names.indexOf(cur)} />
      {stage === "again" && (
        <Again
          service={service}
          existing={existing}
          onAgain={(a) => (a.has_session ? signedIn(a.id) : (setAgain(a), setStage("signin")))}
          onNew={() => (setAgain(null), setStage("signin"))}
          backLabel={backLabel ?? "Another service"}
          onBack={onBack}
        />
      )}
      {stage === "signin" && (
        <SignIn
          key={again?.id ?? "new"}
          slug={slug}
          service={service}
          again={again}
          backLabel={existing.length ? "Back" : backLabel}
          onBack={existing.length ? () => setStage("again") : onBack}
          onSignedIn={signedIn}
          onPrograms={onPrograms}
        />
      )}
      {stage === "session" && id && <SessionStep slug={slug} id={id} startUrl={startUrl} onDone={() => setStage(measured.length ? "measures" : "allow")} />}
      {stage === "measures" && id && <Measures slug={slug} abilities={measured} account={id} onDone={() => setStage("allow")} />}
      {stage === "allow" && id && <Permissions slug={slug} id={id} onDone={() => setStage("done")} />}
      {stage === "done" && id && <Done slug={slug} id={id} onFinish={() => onFinish(id)} />}
    </>
  );
}

/** "You already have u/acme on Reddit. Sign it in again, or add another?" */
function Again({ service, existing, onAgain, onNew, backLabel, onBack }: { service: Service; existing: Account[]; onAgain: (a: Account) => void; onNew: () => void; backLabel: string; onBack: () => void }) {
  const one = existing.length === 1;
  return (
    <motion.div className="pcard cw-panel" initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }}>
      <div className="cw-head">
        <div>
          <h2>{one ? `You already have ${accountIdentity(existing[0]) || "an account"} on ${service.name}` : `You already have ${existing.length} ${service.name} accounts`}</h2>
          <p className="muted sm">{one ? "Sign it in again, or add another?" : "Sign one of them in again, or add another?"}</p>
        </div>
      </div>
      <div className="cw-choice">
        {existing.map((a) => (
          <button key={a.id} onClick={() => onAgain(a)}>
            <b>
              <LogIn size={13} /> Sign {one ? "it" : accountIdentity(a) || a.label} in again
            </b>
            <small>{stateChip(a.state).text}. Its permissions and Audience stay as they are.</small>
          </button>
        ))}
        <button onClick={onNew}>
          <b>
            <Plus size={13} /> Add another
          </b>
          <small>A second {service.name} account, signed in on its own.</small>
        </button>
      </div>
      <div className="acts">
        <button className="btn t" onClick={onBack}>
          <ArrowLeft size={14} /> {backLabel}
        </button>
      </div>
    </motion.div>
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

/**
 * Signing in: the service's button when it has one, else as few fields as
 * possible. A new account through the button exists only once the sign-in
 * came back; through typed keys it is made with them. Signing one in again
 * goes to that account.
 */
function SignIn({ slug, service, again, backLabel = "Another service", onBack, onSignedIn, onPrograms }: { slug: string; service: Service; again?: Account | null; backLabel?: string; onBack: () => void; onSignedIn: (id: string, startUrl?: string) => void; onPrograms?: () => void }) {
  // The one-button sign-in is always the way in when any ability of the service has one; typed keys are the last resort.
  const ab = service.installed.filter((a) => a.connect).sort((a, b) => score(b) - score(a))[0] ?? primary(service.installed);
  const Icon = serviceIcon(service.icon, service.group);
  const [mode, setMode] = useState<"connect" | "keys">(ab?.connect ? "connect" : "keys");
  const [values, setValues] = useState<Record<string, string>>({});
  const [label, setLabel] = useState("");
  const [moreKeys, setMoreKeys] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // The values the service showed for pasting, when it would not send them back to a plain-http KIQR.
  const [paste, setPaste] = useState<string[] | null>(null);
  // The account signed in again, kept fresh while its keys are typed.
  const [acc, setAcc] = useState<Account | null>(again ?? null);
  useEvents(slug, (e) => again && ACCOUNT_EVENTS.includes(e.type) && void api.account(slug, again.id).then(setAcc, () => undefined), { demo: false });
  // The programs the service runs (a browser) are installed when its abilities are; until they are here, that comes first.
  const [programsReady, setProgramsReady] = useState(false);
  const needPrograms = service.installed.filter((a) => (a.programs ?? []).some((p) => !p.found));
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
  const fields = asked.map(keyField);
  const ok = fields.every((f) => f.optional || (values[f.key] ?? "").trim());
  const name = label.trim() || hostOf(Object.values(values));
  const pasteFields = (paste ?? []).map((n) => {
    const k = ab.auth.find((x) => x.env === n);
    return k ? keyField(k) : { key: n, label: n, secret: true };
  });
  const run = async (f: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    try {
      await f();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  const typed = (): Record<string, string> => Object.fromEntries(Object.entries(values).filter(([k, v]) => v.trim() && (ab.auth.some((x) => x.env === k) || (paste ?? []).includes(k))));
  // Typed keys, or what the service showed: into the account signed in again, or a new one made with them.
  const save = () =>
    run(async () => {
      if (again) {
        for (const [k, v] of Object.entries(typed())) await api.setAccountSecret(slug, again.id, k, v);
        onSignedIn(again.id, startUrlOf(...Object.values(values)));
        return;
      }
      const c = await api.createAccount(slug, { service: ab.service ?? service.id, label: name || undefined, secrets: typed() });
      onSignedIn(c.id, startUrlOf(...Object.values(values)));
    });
  const head = (
    <div className="cw-head">
      <span className="ic">{service.avatar ? <img src={service.avatar} alt="" /> : <Icon size={19} />}</span>
      <div>
        <h2>{again ? `Sign ${accountIdentity(again) || service.name} in again` : service.name}</h2>
        <p className="muted sm">{serviceSummary(service)}</p>
      </div>
    </div>
  );
  // Signing in again with keys only: the account's own keys, saved as they are typed.
  if (again && mode === "keys" && acc)
    return (
      <div className="pcard cw-panel">
        {head}
        <AccountKeys slug={slug} a={acc} onChange={() => void api.account(slug, again.id).then(setAcc, () => undefined)} />
        <div className="acts">
          <button className={`btn ${acc.state.ready ? "p" : "g"}`} onClick={() => onSignedIn(again.id)}>
            {acc.state.ready ? "Next" : "Next, finish this later"}
          </button>
          <button className="btn t" onClick={onBack}>
            <ArrowLeft size={14} /> {backLabel}
          </button>
        </div>
      </div>
    );
  return (
    <div className="pcard cw-panel">
      {head}
      {paste ? (
        <>
          <p className="muted sm">Approve in the tab that opened. It shows what KIQR needs once, because it only sends it back over https: paste it here.</p>
          <Fields fields={pasteFields} values={values} onChange={(k, v) => setValues({ ...values, [k]: v })} />
          {error && <p className="err sm">{error}</p>}
          <div className="acts">
            <button className="btn p" disabled={busy || pasteFields.some((f) => !f.optional && !(values[f.key] ?? "").trim())} onClick={() => void save()}>
              {busy ? <Loader2 size={14} className="spin" /> : <Check size={14} />} Connect
            </button>
            <button className="btn t" onClick={() => setPaste(null)}>
              <ArrowLeft size={14} /> Back
            </button>
          </div>
        </>
      ) : (
        <>
          <Fields fields={fields} values={values} onChange={(k, v) => setValues({ ...values, [k]: v })} />
          {moreKeys && (
            <>
              {!again && (
                <label className="acc-field">
                  <span className="nm">
                    Name<small className="dim"> optional</small>
                  </span>
                  <input className="input" value={label} placeholder={hostOf(Object.values(values)) || service.name} onChange={(e) => setLabel(e.target.value)} aria-label="Name" />
                </label>
              )}
              <Fields fields={extra.map(keyField)} values={values} onChange={(k, v) => setValues({ ...values, [k]: v })} />
            </>
          )}
          {error && <p className="err sm">{error}</p>}
          <div className="acts">
            {mode === "connect" && ab.connect ? (
              <ConnectButton
                slug={slug}
                ability={ab.id}
                connect={ab.connect}
                auth={ab.auth.map((k) => ({ ...k, set: null }))}
                account={again?.id}
                label={again ? "Sign in again" : undefined}
                values={values}
                disabled={!ok}
                fresh={{ label: name || undefined }}
                onStarted={(_, p) => p && setPaste(p)}
                onDone={(id) => (id ? onSignedIn(id) : setError("The sign-in finished, but KIQR did not say which account it made. It is under Accounts."))}
              />
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
            {!moreKeys && (extra.length > 0 || !again) && (
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
 * The wizard in place, for one ability's service (the ability window, which
 * the welcome flow opens before the app is): sign in, what it reads through
 * the account, what Seemo may do, Done. One not installed yet is installed first.
 */
export function ConnectService({ slug, ability, onInstalled, onDone, onCancel }: { slug: string; ability: Ability; onInstalled: () => void; onDone: (id: string) => void; onCancel: () => void }) {
  const svc = serviceOf(ability);
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
  return <Flow slug={slug} service={{ id: svc.id, name: svc.name, icon: svc.icon, group: svc.group, avatar: ability.avatar ?? null, installed: [ability], market: [] }} existing={[]} backLabel="Cancel" onBack={onCancel} onPrograms={onInstalled} onFinish={onDone} />;
}

/** One account, loaded and kept fresh while a step shows it. */
function useAccount(slug: string, id: string, also: string[] = []): [Account | null, () => void, boolean] {
  const [a, setA] = useState<Account | null>(null);
  const [gone, setGone] = useState(false);
  const load = () => api.account(slug, id).then(setA, () => setGone(true));
  useEffect(() => {
    void load();
  }, [slug, id]);
  useEvents(slug, (e) => [...ACCOUNT_EVENTS, ...also].includes(e.type) && void load(), { demo: false });
  return [a, () => void load(), gone];
}

/**
 * What the service's abilities read through the account, one ability at a
 * time ("Which GA4 property?"), each picked from what the account has.
 * "Pick it later" leaves it for Performance or the ability window.
 */
function Measures({ slug, abilities, account, onDone }: { slug: string; abilities: Ability[]; account: string; onDone: () => void }) {
  const [acc, , gone] = useAccount(slug, account);
  const [i, setI] = useState(0);
  const a = abilities[i];
  const [values, setValues] = useState<Record<string, string>>(() => (a?.setting?.account === account ? settingValues(a) : {}));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [moving, setMoving] = useState(false);
  useEffect(() => {
    if (gone || !a) onDone();
  }, [gone, a]);
  if (!acc || !a) return <Loader2 size={16} className="spin" />;
  const fields = Object.entries(a.settings ?? {});
  const ok = fields.every(([k, f]) => f.optional || (values[k] ?? "").trim());
  const next = () => {
    const n = abilities[i + 1];
    setError(null);
    if (!n) return onDone();
    setValues(n.setting?.account === account ? settingValues(n) : {});
    setMoving(false);
    setI(i + 1);
  };
  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      await api.setAbilitySettings(slug, a.id, account, settingValues(a, values));
      next();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  const one = fields.length === 1 ? fields[0][1].label || fields[0][0] : null;
  const who = { id: acc.id, label: accountIdentity(acc) || acc.label };
  // Picked already, through another account of the service (a second Google login): keep that unless the user moves it here.
  const elsewhere = !moving && a.setting?.account && a.setting.account !== account && settingsPicked(a) ? (a.accounts ?? []).find((x) => x.id === a.setting!.account) : undefined;
  if (elsewhere)
    return (
      <motion.div key={`${a.id}-keep`} className="pcard cw-panel" initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }}>
        <div className="cw-head">
          <div>
            <h2>{a.name} is set up already</h2>
            <p className="muted sm">
              It reads {fields.map(([k, f]) => `${(f.label || k).toLowerCase()} ${String(a.setting?.[k] ?? "")}`).join(", ")} through {elsewhere.label}. Keep that, or pick one {who.label} sees instead.
              {abilities.length > 1 ? ` ${i + 1} of ${abilities.length}.` : ""}
            </p>
          </div>
        </div>
        <div className="acts">
          <button className="btn p" onClick={next}>
            <Check size={14} /> Keep it
          </button>
          <button className="btn g" onClick={() => setMoving(true)}>
            Use {who.label} instead
          </button>
        </div>
      </motion.div>
    );
  return (
    <motion.div key={a.id} className="pcard cw-panel" initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }}>
      <div className="cw-head">
        <div>
          <h2>{one ? `Which ${one}?` : `What ${a.name} reads`}</h2>
          <p className="muted sm">
            {a.sources.length ? `${a.name} reads the numbers of the one you pick through ${who.label}, once a day, into Performance.` : `${a.name} works with the one you pick, through ${who.label}.`}
            {abilities.length > 1 ? ` ${i + 1} of ${abilities.length}.` : ""}
          </p>
        </div>
      </div>
      {fields.map(([k, f]) => (
        <SettingField key={k} slug={slug} account={who} name={k} f={f} value={values[k] ?? ""} onChange={(v) => setValues((cur) => ({ ...cur, [k]: v }))} />
      ))}
      {error && <p className="err sm">{error}</p>}
      <div className="acts">
        <button className="btn p" disabled={busy || !ok} onClick={() => void save()}>
          {busy ? <Loader2 size={14} className="spin" /> : <Check size={14} />} Use it
        </button>
        <button className="btn t" disabled={busy} onClick={next}>
          Pick it later
        </button>
      </div>
    </motion.div>
  );
}

/** What Seemo may do there: one switch, and every permission folded under it. Saved as it is changed. */
function Permissions({ slug, id, onDone }: { slug: string; id: string; onDone: () => void }) {
  const [a, load] = useAccount(slug, id);
  const [all, setAll] = useState(false);
  return (
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
          <ActOnOwn slug={slug} a={a} onChange={load} />
          <button className="acx-more" onClick={() => setAll((v) => !v)} aria-expanded={all}>
            {all ? <ChevronDown size={13} /> : <ChevronRight size={13} />} Every permission
          </button>
          <AnimatePresence initial={false}>
            {all && (
              <motion.div initial={{ opacity: 0, height: 0 }} animate={{ opacity: 1, height: "auto" }} exit={{ opacity: 0, height: 0 }} transition={{ duration: 0.2 }} style={{ overflow: "hidden" }}>
                <PermissionMatrix slug={slug} a={a} onChange={load} />
              </motion.div>
            )}
          </AnimatePresence>
          <p className="muted sm">“Ask me first” puts a card in the chat and waits for your yes. “On its own” still puts the card there, as the record, and only ever applies in your Audience.</p>
          <div className="acts">
            <button className="btn p" onClick={onDone}>
              Next
            </button>
          </div>
        </>
      )}
    </motion.div>
  );
}

/**
 * Done: the account, how it stands, and what joined the Audience through it.
 * Only an account that reaches people (it publishes, or something joined)
 * speaks of the Audience; one that measures (Google) does not. While the
 * project's first build runs, the app is closed, so there is no link into it.
 */
function Done({ slug, id, onFinish }: { slug: string; id: string; onFinish: () => void }) {
  const [a] = useAccount(slug, id, AUDIENCE_EVENTS);
  const build = useAsync(() => api.build(slug), [slug]);
  if (!a) return <Loader2 size={16} className="spin" />;
  const joined = a.audience ?? [];
  const name = accountIdentity(a);
  const reaches = joined.length > 0 || !!a.can_publish;
  const open = reaches && build.data?.stage === "ready";
  return (
    <motion.div className="pcard cw-panel" initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }}>
      <div className="cw-head">
        <div>
          <h2>
            {accountService(a).name}
            {name ? ` (${name})` : ""} is connected
          </h2>
          <p className="sm">
            <StateChip s={a.state} />
          </p>
        </div>
      </div>
      {joined.length > 0 && (
        <div className="cw-joined">
          <p className="sm">
            <Users size={13} /> Joined your Audience:
          </p>
          <ul>
            {joined.map((x) => (
              <li key={x.id}>{x.label}</li>
            ))}
          </ul>
        </div>
      )}
      {joined.length === 0 && reaches && <p className="muted sm">Nothing joined your Audience through it yet. Add the pages, communities and lists it reaches there, or Seemo suggests them as it finds them.</p>}
      <div className="acts">
        {open && (
          <Link className="btn g" to={`/p/${slug}/audience`}>
            <Users size={14} /> Open your Audience
          </Link>
        )}
        <button className="btn p" onClick={onFinish}>
          <Check size={14} /> Done
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
  const [a, load, gone] = useAccount(slug, id);
  const [phase, setPhase] = useState<"where" | "kiqr" | "signin">("where");
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [hands, setHands] = useState(false);
  useEffect(() => {
    if (gone || (a && !a.has_session)) onDone();
  }, [gone, a?.has_session]);
  if (!a?.has_session) return <Loader2 size={16} className="spin" />;
  const byHand = signsInByHand(a);
  const name = accountIdentity(a) || accountService(a).name;
  const signIn = async () => {
    setBusy(true);
    setError(null);
    setNote(null);
    try {
      setNote(await signInNow(slug, a));
      load();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  const state = a.state.state;
  const hasLogin = !!a.has_login || (a.login_set ?? []).includes("LOGIN_PASSWORD");
  // A login kept in a password manager is read from there: nothing to ask.
  const ask = phase === "where" && !byHand && !hasLogin && (a.secrets?.in_kiqr ?? true);
  const settled = state === "ready" || state === "checking";
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
      {phase === "kiqr" && <LoginEditor slug={slug} a={a} onChange={load} onSaved={() => (setPhase("signin"), void signIn())} />}

      {!ask && phase !== "kiqr" && (
        <>
          <p className="sm">
            <StateChip s={a.state} />
          </p>
          {state === "needs_hand" && (
            <div className="acx-hand">
              <p>
                {a.session?.message ? `${sentence(a.session.message)}. ` : ""}Finish the sign-in in the live view.
              </p>
            </div>
          )}
          {note && <p className="muted sm">{note}</p>}
          {error && <p className="err sm">{error}</p>}
          <div className="acts">
            {!byHand && !settled && (
              <button className={`btn ${state === "needs_hand" || !hasLogin ? "g" : "p"}`} disabled={busy} onClick={() => void signIn()}>
                {busy ? <Loader2 size={14} className="spin" /> : <LogIn size={14} />} {state === "needs_hand" ? "Try again" : "Sign in"}
              </button>
            )}
            {a.live_view && !settled && (byHand || state === "needs_hand" || !hasLogin) && (
              <button className={`btn ${byHand || state === "needs_hand" || !hasLogin ? "p" : "g"}`} onClick={() => setHands(true)}>
                <Hand size={14} /> Take the controls
              </button>
            )}
            <button className={`btn ${state === "ready" ? "p" : "t"}`} onClick={onDone}>
              {state === "ready" ? "Next" : "Next, finish this later"}
            </button>
          </div>
        </>
      )}
      {hands && <Hands slug={slug} account={id} name={name} startUrl={startUrl ?? startUrlOf(a.start_url, a.identity)} onClose={() => (setHands(false), load())} />}
    </motion.div>
  );
}
