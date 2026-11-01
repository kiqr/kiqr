// The programs an ability runs that KIQR installs on this machine (a browser,
// a virtual display). Most are downloads KIQR makes itself; a system package
// is the fallback: shown as the one command for this machine's package
// manager with Copy, and "Install it for me" when the desktop can ask for
// the password (pkexec). Used by the connect wizard and the ability window.
import { useEffect, useState } from "react";
import { AlertCircle, Check, Copy, Loader2, RefreshCw, Terminal } from "lucide-react";
import { api, PROGRAM_EVENTS, useEvents, type Ability, type Health, type ProgramStatus } from "../api";
import "../accounts.css";

let machine: Promise<Health | null> | null = null;
/** The daemon's machine (its system, package manager, pkexec), read once per page load. */
export function useMachine(): Health | null {
  const [h, setH] = useState<Health | null>(null);
  useEffect(() => {
    machine ??= api.health().catch(() => null);
    let gone = false;
    void machine.then((x) => !gone && setH(x));
    return () => {
      gone = true;
    };
  }, []);
  return h;
}

/** A system package that is still missing: what to run, Copy, and "Install it for me" when KIQR can. */
export function SystemProgram({ ability, p, onDone }: { ability: string; p: ProgramStatus; onDone: (programs: ProgramStatus[]) => void }) {
  const [copied, setCopied] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<string | null>(null);
  const copy = () => {
    if (!p.command) return;
    void navigator.clipboard?.writeText(p.command).then(
      () => (setCopied(true), window.setTimeout(() => setCopied(false), 1500)),
      () => setCopied(false),
    );
  };
  const install = async () => {
    setBusy(true);
    setError(null);
    setResult(null);
    try {
      const r = await api.installSystemPrograms(ability);
      const still = (r.programs ?? []).find((x) => x.program === p.program);
      setResult(still && !still.found ? `It ran, but ${p.program} is still not found.` : r.installed.length ? `Installed ${r.installed.join(", ")}.` : "Nothing was left to install.");
      onDone(r.programs ?? []);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  const recheck = async () => {
    setBusy(true);
    setError(null);
    try {
      const r = await api.installPrograms(ability);
      const still = (r.programs ?? []).find((x) => x.program === p.program);
      setResult(still && !still.found ? `${p.program} is still not found.` : `${p.program} is there now.`);
      onDone(r.programs ?? []);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  const entries = Object.entries(p.system ?? {});
  return (
    <div className="pg-sys">
      {p.command ? (
        <>
          <p className="sm">
            KIQR needs one program from your system: <b>{p.program}</b>
          </p>
          <div className="pg-cmd">
            <code className="mono">{p.command}</code>
            <button className="btn t sm" onClick={copy} aria-label="Copy the command">
              {copied ? <Check size={13} /> : <Copy size={13} />} {copied ? "Copied" : "Copy"}
            </button>
          </div>
        </>
      ) : (
        <>
          <p className="sm">
            Install <b>{p.program}</b> with your system's package manager.
          </p>
          {entries.length > 0 && (
            <ul className="pg-list">
              {entries.map(([mgr, pkg]) => (
                <li key={mgr}>
                  <span className="dim">{mgr}:</span> <code className="mono">{pkg}</code>
                </li>
              ))}
            </ul>
          )}
        </>
      )}
      <div className="acts">
        {p.pkexec && (
          <button className="btn p sm" disabled={busy} onClick={() => void install()} title="Your desktop asks for your password; KIQR never sees it">
            {busy ? <Loader2 size={13} className="spin" /> : <Terminal size={13} />} {busy ? "Installing… answer the password dialog" : "Install it for me"}
          </button>
        )}
        <button className="btn t sm" disabled={busy} onClick={() => void recheck()}>
          <RefreshCw size={13} /> I installed it, check again
        </button>
      </div>
      {result && <p className="muted sm">{result}</p>}
      {error && <p className="err sm">{error}</p>}
    </div>
  );
}

/** The ability window's list: each program, found or not, with what to do about a missing one. */
export function ProgramList({ a, onChange }: { a: Ability; onChange: () => void }) {
  const [programs, setPrograms] = useState<ProgramStatus[]>(a.programs ?? []);
  const [notes, setNotes] = useState<Record<string, { state: "installing" | "failed"; note: string }>>({});
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => setPrograms(a.programs ?? []), [a.programs]);
  useEvents(
    null,
    (e) => {
      if (!PROGRAM_EVENTS.includes(e.type)) return;
      const program = String(e.program ?? "");
      if (!programs.some((p) => p.program === program)) return;
      if (e.type === "program_installed") {
        setPrograms((cur) => cur.map((p) => (p.program === program ? { ...p, found: true } : p)));
        setNotes(({ [program]: _gone, ...rest }) => rest);
        onChange();
      } else setNotes((cur) => ({ ...cur, [program]: { state: e.type === "program_failed" ? "failed" : "installing", note: String((e.type === "program_failed" ? e.message : e.note) ?? "") } }));
    },
    { demo: false },
  );
  if (!programs.length) return null;
  const done = (list: ProgramStatus[]) => (list.length && setPrograms(list), onChange());
  const retry = async () => {
    setBusy(true);
    setError(null);
    try {
      done((await api.installPrograms(a.id)).programs ?? []);
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  const ownMissing = programs.some((p) => !p.found && p.how !== "system");
  return (
    <div className="pg">
      <ul className="cw-prog">
        {programs.map((p) => {
          const n = notes[p.program];
          return (
            <li key={p.program} className={p.found ? "done" : n?.state === "failed" && p.how !== "system" ? "bad" : ""}>
              {p.found ? <Check size={12} /> : n?.state === "installing" ? <Loader2 size={12} className="spin" /> : <AlertCircle size={12} />}
              <span>
                {p.program} <span className="mono dim">{p.version}</span>
                <span className="note"> · {p.found ? "on this machine" : n?.state === "installing" ? n.note || "installing" : p.how === "system" ? "from your system" : p.how === "none" ? "no way to install it here" : n?.note || "not installed yet"}</span>
              </span>
            </li>
          );
        })}
      </ul>
      {programs
        .filter((p) => !p.found && p.how === "system")
        .map((p) => (
          <SystemProgram key={p.program} ability={a.id} p={p} onDone={done} />
        ))}
      {ownMissing && (
        <div className="acts">
          <button className="btn t sm" disabled={busy} onClick={() => void retry()}>
            {busy ? <Loader2 size={13} className="spin" /> : <RefreshCw size={13} />} Install again
          </button>
        </div>
      )}
      {error && <p className="err sm">{error}</p>}
    </div>
  );
}
