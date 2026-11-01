// Connect an account: pick the service (grouped as on the Accounts page,
// from what is installed and what the marketplace has; one tile per service,
// however many abilities it has), which installs every ability of it not
// installed yet, sign in once (or give the few keys it needs), pick the
// places it needs (the GA4 property, the Search Console property), then
// choose what Seemo may do there. Done lands on the Accounts page with
// the new card lit. Opened from "Connect an account" and from every place
// that used to send the user to install or add something for a service;
// `?service=` or `?group=` (or the props) narrow it.
import { useEffect, useMemo, useRef, useState } from "react";
import { Link, useNavigate, useParams, useSearchParams } from "react-router-dom";
import { AnimatePresence, motion } from "motion/react";
import { ArrowLeft, Check, Loader2, Plus, Terminal } from "lucide-react";
import { api, useAsync, useEvents, SERVICE_GROUPS, type Ability, type Account, type KindInfo, type PlaceKind } from "../api";
import { ConnectButton } from "./ConnectButton";
import { AccountKeys, ACCOUNT_EVENTS, accountIdentity, accountService, byGroup, chosenLabel, Fields, ownLabel, permissionSummary, PermissionMatrix, placeFields, PlaceFieldsForm, placeKinds, placeNoun, placeReady, placesOf, serviceOf, type Field } from "./Accounts";
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
  const step = created ? (placed ? 4 : 3) : service?.installed.length && !installing ? 2 : 1;
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
                    <Tile key={s.id} s={s} count={have(s.id)} onPick={() => void choose(s)} />
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

      {step === 2 && service && (
        <SignIn
          key={service.id}
          slug={slug}
          service={service}
          onBack={() => (setPicked(null), setError(null))}
          onCreated={setCreated}
        />
      )}
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

function Tile({ s, count, onPick }: { s: Service; count: number; onPick: () => void }) {
  const Icon = serviceIcon(s.icon, s.group);
  const [broken, setBroken] = useState(false);
  const ab = primary(s.installed.length ? s.installed : s.market);
  // A program it runs that is not on this computer: it can be installed, but not run until it is.
  const note = ab?.program_found === false ? "Needs a program this computer does not have yet" : !s.installed.length && ab?.runs_program ? "Runs a program on this computer" : null;
  return (
    <button className={`cw-tile ${count ? "have" : ""}`} onClick={onPick}>
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
function SignIn({ slug, service, backLabel = "Another service", onBack, onCreated }: { slug: string; service: Service; backLabel?: string; onBack: () => void; onCreated: (id: string) => void }) {
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
      onCreated(c.id);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="pcard cw-panel">
      <div className="cw-head">
        <span className="ic">
          <Icon size={19} />
        </span>
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
  if (created && !placed) return <PlaceSteps slug={slug} id={created} onDone={() => setPlaced(true)} />;
  if (created) return <Permissions slug={slug} id={created} onDone={() => onDone(created)} />;
  return <SignIn slug={slug} service={{ id: svc.id, name: svc.name, icon: svc.icon, group: svc.group, avatar: ability.avatar ?? null, installed: [ability], market: [] }} backLabel="Cancel" onBack={onCancel} onCreated={setCreated} />;
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
