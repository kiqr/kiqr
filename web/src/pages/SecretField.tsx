// A secret by name: typed once, saved to .env (one account's, or KIQR's own keys on the machine), never shown again.
import { useEffect, useState } from "react";
import { Check, ExternalLink, Loader2 } from "lucide-react";
import { api } from "../api";
import { FieldInput } from "./FieldInput";

/** A help line: a bare URL becomes "where to get it"; a sentence keeps its words, with any URL in it linked. */
export function Hint({ text }: { text: string }) {
  if (/^https?:\/\/\S+$/.test(text.trim()))
    return (
      <a href={text.trim()} target="_blank" rel="noreferrer" className="lnk">
        where to get it <ExternalLink size={10} />
      </a>
    );
  const parts = text.split(/(https?:\/\/\S+)/);
  return (
    <span className="hint">
      {parts.map((p, i) =>
        /^https?:\/\//.test(p) ? (
          <a key={i} href={p} target="_blank" rel="noreferrer" className="lnk">
            {p.replace(/^https?:\/\//, "")} <ExternalLink size={10} />
          </a>
        ) : (
          <span key={i}>{p}</span>
        ),
      )}
    </span>
  );
}

export function SecretField({ name, label, help, placeholder, set, optional, options, slug, account, onSaved }: { name: string; label: string; help?: string; placeholder?: string; set: boolean | string | null; optional?: boolean; options?: string[]; slug?: string; account?: string; onSaved?: () => void }) {
  const [value, setValue] = useState("");
  const [busy, setBusy] = useState(false);
  const [done, setDone] = useState(!!set);
  const [error, setError] = useState<string | null>(null);
  const [editing, setEditing] = useState(!set);
  useEffect(() => {
    setDone(!!set);
    setEditing(!set);
  }, [set]);
  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      const r = slug && account ? await api.setAccountSecret(slug, account, name, value.trim()) : slug ? await api.setProjectSecret(slug, name, value.trim()) : await api.setSecret(name, value.trim());
      setDone(r.set);
      setEditing(!r.set);
      setValue("");
      onSaved?.();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="set-key">
      <span className="lb">
        {label}
        {optional && <small> optional</small>}
        {help && <Hint text={help} />}
      </span>
      {editing ? (
        <span className="in">
          <FieldInput mono secret options={options} placeholder={done ? "new value" : placeholder || "paste it here"} value={value} onChange={setValue} onEnter={() => value.trim() && void save()} ariaLabel={label} />
          <button className="btn g sm" disabled={busy || !value.trim()} onClick={() => void save()}>
            {busy ? <Loader2 size={13} className="spin" /> : <Check size={13} />} Save
          </button>
          {done && (
            <button className="btn t sm" onClick={() => setEditing(false)}>
              Cancel
            </button>
          )}
        </span>
      ) : (
        <span className="in">
          <span className="chip ok">
            <b /> {typeof set === "string" && set === "project" ? "set for this project" : "set"}
          </span>
          <button className="btn t sm" onClick={() => setEditing(true)}>
            Change
          </button>
        </span>
      )}
      {error && <span className="err">{error}</span>}
    </div>
  );
}
