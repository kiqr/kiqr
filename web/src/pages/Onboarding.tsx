import { useState } from "react";
import { Navigate, useNavigate, useOutletContext } from "react-router-dom";
import { api, useAsync } from "../api";

export function Onboarding({ forceNew = false }: { forceNew?: boolean }) {
  const projects = useAsync(() => api.projects(), []);
  const navigate = useNavigate();
  const { reloadProjects } = useOutletContext<{ reloadProjects: () => void }>();
  const [url, setUrl] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  if (projects.loading) return null;
  if (!forceNew && projects.data && projects.data.length > 0) {
    return <Navigate to={`/p/${projects.data[0].slug}`} replace />;
  }
  const first = (projects.data?.length ?? 0) === 0;

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
      reloadProjects();
      navigate(`/p/${p.slug}/crawl`);
    } catch (err) {
      setError(String((err as Error).message));
      setBusy(false);
    }
  };

  return (
    <div className="onb">
      <h1 className="d1">{first ? "Give Seemo your website." : "Add another website."}</h1>
      <p className="lead">
        {first
          ? "You're hiring your first AI marketer. Seemo reads up to 200 pages and builds a business profile with evidence for every fact. You can edit anything."
          : "Each project gets its own profile, crawl history and conversations with Seemo."}
      </p>
      <form onSubmit={submit}>
        <div className={`field ${error ? "bad" : ""}`}>
          <span>https://</span>
          <input placeholder="your-site.com" value={url} onChange={(e) => setUrl(e.target.value)} aria-label="Website" autoFocus />
          <button className="btn a" disabled={busy}>
            {busy ? "Starting…" : "Start crawl"}
          </button>
        </div>
      </form>
      <p className={`help ${error ? "bad" : ""}`}>{error ?? "Runs on your machine with your own Claude or Codex login. No account needed."}</p>
    </div>
  );
}
