// One button instead of typed keys, when an ability's manifest says how its
// service signs in. It always signs in one account: an existing one again
// (`account`), or a new one it adds first (`fresh`: its name, and a channel's
// kind and destination). What is asked first (the site) is typed here; the
// rest comes back from the service, or is shown there for the user to paste
// into the account when the service will not send it back to a plain-http KIQR.
import { useState } from "react";
import { ExternalLink, Loader2, LogIn } from "lucide-react";
import { api, useEvents, type ConnectInfo } from "../api";
import { FieldInput } from "./FieldInput";

type Key = { env: string; label: string; help: string; optional: boolean; options?: string[]; placeholder?: string; set: boolean | string | null };

export function ConnectButton({ slug, ability, connect, auth, account, fresh, label, values: given, disabled, onStarted, onDone }: {
  slug: string;
  ability: string;
  connect: ConnectInfo;
  auth: Key[];
  /** The account to sign in again. */
  account?: string;
  /** For a new account: what to call it, and for a channel its kind and destination. */
  fresh?: { label?: string; kind?: string; destination?: Record<string, string> };
  /** The button's words, when not the manifest's ("Connect again"). */
  label?: string;
  /** What it asks first, when the form around it already asked (then no fields here). */
  values?: Record<string, string>;
  disabled?: boolean;
  /** The account the sign-in goes to, once it exists (a new one is added at the start). */
  onStarted?: (account: string, paste: boolean) => void;
  onDone?: () => void;
}) {
  const ask = connect.needs.map((n) => auth.find((k) => k.env === n) ?? { env: n, label: n, help: "", optional: false, set: null });
  const [own, setValues] = useState<Record<string, string>>({});
  const values = given ?? own;
  const [busy, setBusy] = useState(false);
  const [waiting, setWaiting] = useState<"callback" | "paste" | null>(null);
  const [target, setTarget] = useState<string | undefined>(account);
  const [error, setError] = useState<string | null>(null);
  useEvents(null, (e) => {
    if (e.type !== "connect_finished" || e.ability !== ability || (target && e.connection && e.connection !== target)) return;
    setWaiting(null);
    if (e.ok) onDone?.();
    else setError(String(e.error ?? "the sign-in failed"));
  }, { demo: false });
  const missing = ask.filter((k) => !k.set && !values[k.env]?.trim());
  const start = async () => {
    setBusy(true);
    setError(null);
    try {
      const r = await api.startConnect(slug, ability, { connection: target, ...(target ? {} : fresh), origin: window.location.origin, values });
      window.open(r.url, "_blank", "noopener");
      setTarget(r.connection);
      setWaiting(r.callback ? "callback" : "paste");
      onStarted?.(r.connection, !r.callback);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  const returned = auth.filter((k) => connect.returns.includes(k.env) && !connect.needs.includes(k.env)).map((k) => k.label || k.env);
  return (
    <div className="ab-sign">
      {!given && ask
        .filter((k) => !k.set)
        .map((k) => (
          <label key={k.env} className="set-key">
            <span className="lb">{k.label || k.env}</span>
            <span className="in">
              <FieldInput options={k.options} optional={k.optional} placeholder={k.placeholder ?? ""} value={values[k.env] ?? ""} onChange={(v) => setValues({ ...values, [k.env]: v })} ariaLabel={k.label || k.env} />
            </span>
          </label>
        ))}
      <div className="row">
        <button className="btn p sm" disabled={busy || disabled || missing.length > 0} onClick={() => void start()}>
          {busy ? <Loader2 size={13} className="spin" /> : <LogIn size={13} />} {label ?? connect.label}
        </button>
        {waiting === "callback" && (
          <span className="muted sm">
            <Loader2 size={12} className="spin" /> Waiting for you to approve in the tab that opened <ExternalLink size={11} />
          </span>
        )}
      </div>
      {waiting === "paste" && <p className="muted sm">Approve in the tab that opened. It shows {returned.join(" and ") || "the key"} once, because it only sends them back to KIQR over https: paste them into this account below.</p>}
      {error && <p className="err sm">{error}</p>}
    </div>
  );
}
