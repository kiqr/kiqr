import { useState } from "react";
import { useParams } from "react-router-dom";
import { api, useAsync } from "../api";

export function ProjectSettings() {
  const { slug = "" } = useParams();
  const project = useAsync(() => api.project(slug), [slug]);
  const [maxPages, setMaxPages] = useState<string>("");
  const [conc, setConc] = useState<string>("");
  const [ai, setAi] = useState<string>("");
  const [saved, setSaved] = useState(false);
  const p = project.data;
  const save = async (e: React.FormEvent) => {
    e.preventDefault();
    await api.crawlSettings(slug, {
      max_pages: maxPages ? Number(maxPages) : undefined,
      concurrency: conc ? Number(conc) : undefined,
      ai_concurrency: ai ? Number(ai) : undefined,
    });
    setSaved(true);
    project.reload();
  };
  return (
    <div style={{ maxWidth: 720 }}>
      <div className="topbar">
        <h1 className="d2">Project settings</h1>
      </div>
      <form onSubmit={save}>
        <section className="sect">
          <header>
            <h2 className="d3">Crawl</h2>
            <p>Applies to the next crawl. Leave a field empty to use the global default.</p>
          </header>
          <div className="kv">
            <div>
              <div className="k">
                Max pages<small>Default 200</small>
              </div>
              <input className="input" type="number" min={1} max={5000} placeholder={String(p?.crawl.max_pages ?? "")} value={maxPages} onChange={(e) => setMaxPages(e.target.value)} />
            </div>
            <div>
              <div className="k">
                Pages fetched in parallel<small>1–10, default 5. Fetching is done by KIQR, never by the model.</small>
              </div>
              <input className="input" type="number" min={1} max={10} placeholder={String(p?.crawl.concurrency ?? "")} value={conc} onChange={(e) => setConc(e.target.value)} />
            </div>
            <div>
              <div className="k">
                Pages Seemo reads in parallel<small>1–10, default 3. Reading is queued and never slows fetching.</small>
              </div>
              <input className="input" type="number" min={1} max={10} placeholder={String(p?.crawl.ai_concurrency ?? "")} value={ai} onChange={(e) => setAi(e.target.value)} />
            </div>
          </div>
          <div style={{ display: "flex", gap: 8, marginTop: 14, alignItems: "center" }}>
            <button className="btn p">Save</button>
            {saved && <span className="chip ok">saved</span>}
          </div>
        </section>
      </form>
      <section className="sect">
        <header>
          <h2 className="d3">Project</h2>
        </header>
        <div className="kv">
          <div>
            <div className="k">Website</div>
            <div>
              <code>{p?.url}</code>
            </div>
          </div>
          <div>
            <div className="k">
              Remove project<small>Deletes the profile, facts and history for this site.</small>
            </div>
            <div>
              <button
                className="btn g bad"
                onClick={async () => {
                  if (window.confirm(`Remove ${p?.name} and everything under it?`)) {
                    await api.deleteProject(slug);
                    location.href = "/";
                  }
                }}
              >
                Remove
              </button>
            </div>
          </div>
        </div>
      </section>
    </div>
  );
}

export function Placeholder({ title, text }: { title: string; text: string }) {
  return (
    <div>
      <div className="topbar">
        <h1 className="d2">{title}</h1>
      </div>
      <p className="empty">{text}</p>
    </div>
  );
}
