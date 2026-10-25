// One button instead of typed keys, when an ability's manifest says how its
// service signs in. What is asked first (the site) is typed here; the rest
// comes back from the service, or is shown there for the user to paste when
// the service will not send it back to a plain-http KIQR.
import { useState } from "react";
import { ExternalLink, Loader2, LogIn } from "lucide-react";
import { api, useEvents, type ConnectInfo, type MarketAuth } from "../api";

export function ConnectButton({ slug, ability, connect, auth, scope, onDone }: { slug: string; ability: string; connect: ConnectInfo; auth: MarketAuth[]; scope: "project" | "global"; onDone?: () => void }) {
  const ask = connect.needs.map((n) => auth.find((k) => k.env === n) ?? { env: n, label: n, help: "", optional: false, set: null });
  const [values, setValues] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);
  const [waiting, setWaiting] = useState<"callback" | "paste" | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEvents(null, (e) => {
    if (e.type !== "connect_finished" || e.ability !== ability) return;
    setWaiting(null);
    if (e.ok) onDone?.();
    else setError(String(e.error ?? "the sign-in failed"));
  });
  const missing = ask.filter((k) => !k.set && !values[k.env]?.trim());
  const start = async () => {
    setBusy(true);
    setError(null);
    try {
      const r = await api.startConnect(slug, ability, { scope, origin: window.location.origin, values });
      window.open(r.url, "_blank", "noopener");
      setWaiting(r.callback ? "callback" : "paste");
      if (Object.keys(values).length) onDone?.();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  const pasteLabels = auth.filter((k) => connect.returns.includes(k.env) && !connect.needs.includes(k.env)).map((k) => k.label || k.env);
  return (
    <div className="ab-sign">
      {ask
        .filter((k) => !k.set)
        .map((k) => (
          <label key={k.env} className="set-key">
            <span className="lb">{k.label || k.env}</span>
            <span className="in">
              <input className="input" autoComplete="off" spellCheck={false} placeholder={k.help || ""} value={values[k.env] ?? ""} onChange={(e) => setValues({ ...values, [k.env]: e.target.value })} />
            </span>
          </label>
        ))}
      <div className="row">
        <button className="btn p sm" disabled={busy || missing.length > 0} onClick={() => void start()}>
          {busy ? <Loader2 size={13} className="spin" /> : <LogIn size={13} />} {connect.label}
        </button>
        {waiting === "callback" && (
          <span className="muted sm">
            <Loader2 size={12} className="spin" /> Waiting for you to approve in the tab that opened <ExternalLink size={11} />
          </span>
        )}
      </div>
      {waiting === "paste" && <p className="muted sm">Approve in the tab that opened. It shows {pasteLabels.join(" and ") || "the key"} once, because it only sends them back to KIQR over https: paste them below.</p>}
      {error && <p className="err sm">{error}</p>}
    </div>
  );
}
