import { useState } from "react";
import { Navigate, useNavigate, useOutletContext } from "react-router-dom";
import { motion } from "motion/react";
import { Brain, RefreshCw } from "lucide-react";
import { api, useAsync, type Health } from "../api";
import { DEMO } from "../demo";
import { SecretField } from "./SecretField";

const reduced = () => window.matchMedia("(prefers-reduced-motion: reduce)").matches;

export function Onboarding({ forceNew = false }: { forceNew?: boolean }) {
  const projects = useAsync(() => api.projects(), []);
  const health = useAsync(() => api.health(), []);
  const navigate = useNavigate();
  const { reloadProjects } = useOutletContext<{ reloadProjects: () => void }>();
  const [url, setUrl] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [leaving, setLeaving] = useState(false);

  if (projects.loading) return null;
  if (!forceNew && projects.data && projects.data.length > 0) {
    const p = projects.data[0];
    return <Navigate to={!DEMO && p.build && p.build.stage !== "ready" ? `/welcome/${p.slug}` : `/p/${p.slug}`} replace />;
  }
  // Seemo can't read a site without a model; say so before the crawl, not after it.
  const brainless = !!health.data && !health.data.providers.some((x) => x.task === "extract" && x.available);
  const first = (projects.data?.length ?? 0) === 0;
  const host = url.trim().replace(/^https?:\/\//i, "").replace(/\/.*$/, "");

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    const raw = url.trim().replace(/^https?:\/\//i, "");
    if (!/^[a-z0-9.-]+\.[a-z]{2,}(\/.*)?$/i.test(raw)) {
      setError("Enter a website address, like acme.com.");
      return;
    }
    setBusy(true);
    try {
      const p = await api.createProject(`https://${raw}`);
      // Let the form fly out before the crawl screen takes over.
      setLeaving(true);
      await new Promise((r) => setTimeout(r, reduced() ? 0 : 420));
      reloadProjects();
      navigate(`/welcome/${p.slug}`, { state: { fresh: true } });
    } catch (err) {
      setError(String((err as Error).message));
      setBusy(false);
    }
  };

  return (
    <div className={`onb ${leaving ? "leaving" : ""}`}>
      <span className="av onb-av" />
      <h1 className="d1">{first ? (
          <>
            Give Seemo <em>your website.</em>
          </>
        ) : (
          <>
            Add <em>another website.</em>
          </>
        )}</h1>
      <p className="lead">
        {first
          ? "You're hiring your first AI marketer. Seemo reads up to 200 pages and builds a business profile with evidence for every fact. You can edit anything."
          : "Each project gets its own profile, crawl history and conversations with Seemo."}
      </p>
      {brainless && <NeedsBrain h={health.data!} recheck={health.reload} />}
      <form onSubmit={submit} hidden={brainless}>
        <div className={`field ${error ? "bad" : ""} ${busy ? "busy" : ""}`}>
          <span>https://</span>
          <input placeholder="your-site.com" value={url} onChange={(e) => setUrl(e.target.value)} aria-label="Website" autoFocus disabled={busy} />
          <button className="btn a" disabled={busy}>
            {busy ? `Opening ${host}…` : "Start crawl"}
          </button>
        </div>
      </form>
      <p key={error ?? ""} className={`help ${error ? "bad shake" : ""}`} hidden={brainless}>
        {error ?? "Runs on your machine with your own Claude or Codex login. No account needed."}
      </p>
    </div>
  );
}

/** Before the first crawl: Seemo needs a model it can use, from a CLI login or an API key. */
function NeedsBrain({ h, recheck }: { h: Health; recheck: () => void }) {
  const model = h.providers.find((x) => x.task === "extract")?.model ?? "";
  const keys = (h.secrets ?? []).filter((s) => /^(ANTHROPIC|OPENAI)_API_KEY$/.test(s.name));
  const cli = model.startsWith("claude-cli/") ? "claude" : model.startsWith("codex-cli/") ? "codex" : null;
  const installed = cli ? h.tools?.find((t) => t.name === cli)?.installed : undefined;
  return (
    <motion.div initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }} className="pcard onb-brain">
      <div className="hd">
        <span className="ic">
          <Brain size={16} />
        </span>
        <div>
          <h3>First, give Seemo a brain</h3>
          <p className="muted sm">Seemo thinks with the AI you already use. It is set to <code>{model}</code>, which this computer can't reach yet.</p>
        </div>
      </div>
      {cli && (
        <p className="sm">
          {installed ? (
            <>
              Sign in to it: run <code>{cli}</code> in a terminal and log in, then check again.
            </>
          ) : (
            <>
              Install the <code>{cli}</code> command-line tool and sign in with your {cli === "claude" ? "Claude" : "ChatGPT"} plan, or use an API key below.
            </>
          )}
        </p>
      )}
      {keys.length > 0 && (
        <div className="set-keys">
          {keys.map((k) => (
            <SecretField key={k.name} name={k.name} label={k.name === "ANTHROPIC_API_KEY" ? "Anthropic API key" : "OpenAI API key"} set={k.set} optional onSaved={recheck} />
          ))}
        </div>
      )}
      <div className="row">
        <button className="btn g sm" onClick={recheck}>
          <RefreshCw size={13} /> Check again
        </button>
        <a className="btn t sm" href="/settings/models">
          Choose the models
        </a>
      </div>
    </motion.div>
  );
}
