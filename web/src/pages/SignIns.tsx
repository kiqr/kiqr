// KIQR's own sign-ins: kiqr.cloud (connects this KIQR agent with third-party
// apps and finishes their sign-ins) and UnblockingAPI by account instead of
// an API key. On this computer the service's page opens and comes back here;
// elsewhere a code is shown to enter at the service.
import { useState } from "react";
import { Check, ExternalLink, Loader2, LogIn, LogOut } from "lucide-react";
import { api, useAsync, useEvents, type SignIn } from "../api";
import "../signin.css";

/**
 * One sign-in: who it is signed in as, or the button (and the code, in device
 * mode). With `slug`, a sign-in that can belong to one project asks which:
 * this project (the default) or every project.
 */
export function SignInRow({ s, onChange, label, slug }: { s: SignIn; onChange: () => void; label?: string; slug?: string }) {
  const [busy, setBusy] = useState(false);
  const choose = !!slug && !!s.per_project;
  const [forProject, setForProject] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [opened, setOpened] = useState(false);
  const start = async () => {
    setBusy(true);
    setError(null);
    try {
      const r = await api.startSignIn(s.id, window.location.origin, choose && forProject ? slug : undefined);
      if (r.mode === "redirect") {
        window.open(r.url, "_blank", "noopener");
        setOpened(true);
      }
      onChange();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  const out = async () => {
    setBusy(true);
    try {
      await api.signOut(s.id, s.scope === "project" ? slug : undefined);
      onChange();
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="signin">
      {s.signed_in ? (
        <span className="row">
          <span className="chip ok">
            <Check size={12} /> Signed in{s.account ? ` as ${s.account}` : ""}
            {choose && (s.scope === "project" ? " · this project" : " · every project")}
          </span>
          <button className="btn t sm" disabled={busy} onClick={() => void out()}>
            <LogOut size={13} /> Sign out
          </button>
        </span>
      ) : s.waiting ? (
        <span className="code-box">
          <span className="muted sm">Enter this code at </span>
          <a className="lnk" href={s.waiting.verification_uri_complete ?? s.waiting.verification_uri} target="_blank" rel="noreferrer">
            {s.waiting.verification_uri.replace(/^https?:\/\//, "")} <ExternalLink size={11} />
          </a>
          <b className="mono code">{s.waiting.user_code.replace(/^(.{4})(.{4})$/, "$1-$2")}</b>
          <span className="muted sm">
            <Loader2 size={12} className="spin" /> waiting for you to approve
          </span>
        </span>
      ) : (
        <span className="row">
          {choose && (
            <span className="scope-pick" role="radiogroup" aria-label="For which projects">
              <button className={`chip ${forProject ? "acc" : ""}`} role="radio" aria-checked={forProject} onClick={() => setForProject(true)}>
                This project
              </button>
              <button className={`chip ${!forProject ? "acc" : ""}`} role="radio" aria-checked={!forProject} onClick={() => setForProject(false)}>
                Every project
              </button>
            </span>
          )}
          <button className="btn p sm" disabled={busy} onClick={() => void start()}>
            {busy ? <Loader2 size={13} className="spin" /> : <LogIn size={13} />} {label ?? `Sign in to ${s.name}`}
          </button>
          {opened && (
            <span className="muted sm">
              <Loader2 size={12} className="spin" /> Approve in the tab that opened
            </span>
          )}
        </span>
      )}
      {error && <p className="err sm">{error}</p>}
    </div>
  );
}

/** The sign-ins KIQR knows (as one project sees them), kept fresh as they change. */
export function useSignIns(slug?: string) {
  const data = useAsync(() => api.signIns(slug), [slug]);
  useEvents(null, (e) => e.type === "sign_in_changed" && data.reload(), { demo: false });
  return data;
}

/** One sign-in by id, for a page that shows just that one. */
export function SignInById({ id, label, slug }: { id: string; label?: string; slug?: string }) {
  const data = useSignIns(slug);
  const s = data.data?.signins.find((x) => x.id === id);
  return s ? <SignInRow s={s} onChange={data.reload} label={label} slug={slug} /> : null;
}
