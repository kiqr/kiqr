import { useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { api, useAsync, useEvents, type Field, type Section } from "../api";

export function ProjectPage() {
  const { slug = "" } = useParams();
  const navigate = useNavigate();
  const project = useAsync(() => api.project(slug), [slug]);
  const profile = useAsync(() => api.profile(slug), [slug]);
  useEvents(slug, (e) => {
    if (e.type === "profile_updated" || e.type === "crawl_finished") {
      profile.reload();
      project.reload();
    }
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
      <div className="topbar">
        <h1 className="d2">Business profile</h1>
        {p?.last_crawl && (
          <span className="chip ok">
            <b />
            {p.last_crawl.pages} pages · {new Date(p.last_crawl.finished_at).toLocaleDateString()}
          </span>
        )}
        <span className="sp" />
        {p && (
          <>
            <button className="btn g" onClick={recrawl}>
              Re-crawl
            </button>
            <a className="btn p" href={`/api/projects/${slug}/profile.md`} download={`${slug}-profile.md`}>
              Export
            </a>
          </>
        )}
      </div>
      {sections.length > 0 && (
        <div className="tabs">
          {sections.map((s) => (
            <button key={s.id} className="chip" onClick={() => document.getElementById(`s-${s.id}`)?.scrollIntoView({ behavior: "smooth" })}>
              {s.title}
            </button>
          ))}
        </div>
      )}
      {sections.length === 0 && (
        <div className="empty">
          {p?.last_crawl ? (
            <>
              The crawl finished but no facts were extracted. Check the models in <Link to="/settings">settings</Link>, then{" "}
              <button className="btn t" onClick={() => api.synthesize(slug)}>
                build the profile
              </button>
              .
            </>
          ) : (
            <>
              No profile yet. <Link to={`/p/${slug}/crawl`}>Watch the crawl</Link>; sections appear as Seemo finishes them.
            </>
          )}
        </div>
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
    <section className="sect" id={`s-${section.id}`}>
      <header>
        <h2 className="d3">{section.title}</h2>
        <p>{section.description}</p>
      </header>
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
  return (
    <div className="pf">
      <div className="k">
        {field.name.replace(/_/g, " ")}
        {field.locked && <span className="chip">locked by you</span>}
        {!field.locked && low && (
          <span className="chip warn">
            <b />
            needs review
          </span>
        )}
      </div>
      <div className="acts">
        <button className="btn t" onClick={() => setEditing(true)}>
          Edit
        </button>
        <button className="btn t" onClick={() => onChange({ ...field, locked: !field.locked })}>
          {field.locked ? "Unlock" : "Lock"}
        </button>
        <button className="btn t" onClick={() => window.confirm(`Remove "${field.name}" from the profile?`) && onRemove()}>
          Remove
        </button>
      </div>
      {editing ? (
        <div className="edit">
          <textarea className="input" value={draft} onChange={(e) => setDraft(e.target.value)} autoFocus />
          <div className="acts2">
            <button
              className="btn p"
              onClick={() => {
                onChange({ ...field, value: draft, locked: true, edited_at: new Date().toISOString() });
                setEditing(false);
              }}
            >
              Save and lock
            </button>
            <button className="btn g" onClick={() => setEditing(false)}>
              Cancel
            </button>
          </div>
          <p className="help">Locked fields keep your value through every re-crawl.</p>
        </div>
      ) : (
        <div className="v">{field.value}</div>
      )}
      <div className="e">
        {field.evidence.slice(0, 3).map((e, i) => (
          <span key={i}>
            <code>{pathOf(e.url)}</code> {e.quote && `“${e.quote}”`}
          </span>
        ))}
        {field.locked && field.edited_at && <span>Edited {new Date(field.edited_at).toLocaleDateString()}</span>}
        {!field.locked && field.confidence > 0 && (
          <span className={`conf ${low ? "low" : ""}`}>
            <i style={{ "--w": `${Math.round(field.confidence * 100)}%` } as React.CSSProperties} />
            {field.confidence.toFixed(2)}
          </span>
        )}
      </div>
    </div>
  );
}

function pathOf(u: string) {
  try {
    return new URL(u).pathname || "/";
  } catch {
    return u;
  }
}
