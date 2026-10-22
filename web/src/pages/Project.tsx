import { useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { api, useAsync, useEvents, type Field, type Section } from "../api";

export function ProjectPage() {
  const { slug = "" } = useParams();
  const navigate = useNavigate();
  const project = useAsync(() => api.project(slug), [slug]);
  const profile = useAsync(() => api.profile(slug), [slug]);
  useEvents(slug, (e) => {
    if (e.type === "profile_updated" || e.type === "crawl_finished") profile.reload();
  });

  if (project.error) return <p className="empty">{project.error}</p>;
  const p = project.data;
  const sections = (profile.data ?? []).filter((s) => s.fields.length > 0);

  const recrawl = async () => {
    await api.crawl(slug);
    navigate(`/p/${slug}/crawl`);
  };

  return (
    <div>
      <div className="page-head">
        <h1 className="display">Business profile</h1>
        <span className="spacer" />
        {p && (
          <>
            <button className="btn secondary" onClick={recrawl}>
              Re-crawl site
            </button>
            <Link to={`/p/${slug}/crawl`} className="btn text">
              Crawl log
            </Link>
          </>
        )}
      </div>
      {p && (
        <p className="body-sm" style={{ color: "var(--ink-muted)", marginTop: "calc(-1 * var(--space-8))", marginBottom: "var(--space-12)" }}>
          <code>{p.url}</code>
          {p.last_crawl && ` — ${p.last_crawl.pages} pages read ${new Date(p.last_crawl.finished_at).toLocaleString()}`}
        </p>
      )}
      {sections.length === 0 && (
        <p className="empty">
          No profile yet. {p?.last_crawl ? "The crawl finished but no facts were extracted; check the model in settings." : "The crawl is running; facts will appear as pages are read."}
        </p>
      )}
      {sections.map((s) => (
        <SectionView key={s.id} slug={slug} section={s} onSaved={profile.reload} />
      ))}
    </div>
  );
}

function SectionView({ slug, section, onSaved }: { slug: string; section: Section; onSaved: () => void }) {
  const save = async (fields: Field[]) => {
    await api.saveSection(slug, { ...section, fields });
    onSaved();
  };
  return (
    <section className="section">
      <h2 className="title">{section.title}</h2>
      <p className="section-desc">{section.description}</p>
      {section.fields.map((f, i) => (
        <FieldView
          key={f.name}
          field={f}
          onChange={(nf) => save(section.fields.map((x, j) => (j === i ? nf : x)))}
          onRemove={() => save(section.fields.filter((_, j) => j !== i))}
        />
      ))}
    </section>
  );
}

function FieldView({ field, onChange, onRemove }: { field: Field; onChange: (f: Field) => void; onRemove: () => void }) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(field.value);
  const low = field.confidence > 0 && field.confidence < 0.6;
  const confirmRemove = () => {
    if (window.confirm(`Remove "${field.name}" from the profile?`)) onRemove();
  };
  return (
    <div className="pf">
      <div className="pf-name">
        {field.name}
        {field.locked && <span className="lock">Locked by you</span>}
        <span className="pf-actions">
          <button className="btn text" onClick={() => setEditing(true)}>
            Edit
          </button>
          <button className="btn text" onClick={() => onChange({ ...field, locked: !field.locked })}>
            {field.locked ? "Unlock" : "Lock"}
          </button>
          <button className="btn text" onClick={confirmRemove}>
            Remove
          </button>
        </span>
      </div>
      {editing ? (
        <div className="pf-edit">
          <textarea className="input" value={draft} onChange={(e) => setDraft(e.target.value)} autoFocus />
          <div className="actions">
            <button
              className="btn primary"
              onClick={() => {
                onChange({ ...field, value: draft, locked: true, edited_at: new Date().toISOString() });
                setEditing(false);
              }}
            >
              Save changes
            </button>
            <button className="btn text" onClick={() => setEditing(false)}>
              Cancel
            </button>
          </div>
          <p className="help">Saving locks the field so re-crawls keep your value.</p>
        </div>
      ) : (
        <div className="pf-value body">{field.value}</div>
      )}
      {field.evidence.slice(0, 3).map((e, i) => (
        <div key={i} className="pf-evidence">
          From <code>{pathOf(e.url)}</code> — “{e.quote}”
        </div>
      ))}
      {field.locked && field.edited_at && <div className="pf-evidence">Edited {new Date(field.edited_at).toLocaleDateString()}. Survives re-crawls.</div>}
      {!field.locked && field.confidence > 0 && (
        <div className={`pf-conf ${low ? "low" : ""}`}>
          <span className="bar">
            <i style={{ width: `${Math.round(field.confidence * 100)}%` }} />
          </span>
          Confidence {field.confidence.toFixed(2)}
          {low && ", needs review"}
        </div>
      )}
    </div>
  );
}

function pathOf(u: string) {
  try {
    const x = new URL(u);
    return x.pathname || "/";
  } catch {
    return u;
  }
}
