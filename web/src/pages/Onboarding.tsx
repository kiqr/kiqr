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

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    if (!/^https?:\/\//i.test(url.trim())) {
      setError("Enter a full address, starting with https://.");
      return;
    }
    setBusy(true);
    try {
      const p = await api.createProject(url.trim());
      reloadProjects();
      navigate(`/p/${p.slug}/crawl`);
    } catch (err) {
      setError(String((err as Error).message));
      setBusy(false);
    }
  };

  return (
    <div className="onboarding">
      <h1 className="display-xl">
        Tell us your website.
        <br />
        Seemo does the rest.
      </h1>
      <form onSubmit={submit}>
        <input
          className={`input ${error ? "error" : ""}`}
          placeholder="https://"
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          aria-label="Website"
          autoFocus
        />
        <button className="btn primary" disabled={busy}>
          {busy ? "Starting" : "Start crawl"}
        </button>
      </form>
      <p className={`help ${error ? "error" : ""}`}>{error ?? "We'll read up to 200 pages and build a business profile you can edit. Change the limit in settings."}</p>
    </div>
  );
}
