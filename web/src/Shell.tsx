import { useEffect, useRef, useState } from "react";
import { NavLink, Outlet, useNavigate, useParams } from "react-router-dom";
import { api, useAsync, type Project } from "./api";
import { SeemoWidget } from "./SeemoWidget";

export function Shell() {
  const { slug } = useParams();
  const projects = useAsync(() => api.projects(), []);
  const list = projects.data ?? [];
  const current = list.find((p) => p.slug === slug) ?? null;

  return (
    <div className="shell">
      <aside className="side">
        <div className="mark">
          <i />
          KIQR
        </div>
        <ProjectSelector projects={list} current={current} />
        {current && (
          <nav className="nav" aria-label="Project">
            <NavLink to={`/p/${current.slug}`} end>
              Profile
            </NavLink>
            <NavLink to={`/p/${current.slug}/crawl`}>Crawl</NavLink>
            <NavLink to={`/p/${current.slug}/competitors`}>Competitors</NavLink>
            <NavLink to={`/p/${current.slug}/tasks`}>Tasks</NavLink>
            <NavLink to={`/p/${current.slug}/chats`}>Chats with Seemo</NavLink>
            <NavLink to={`/p/${current.slug}/settings`}>Project settings</NavLink>
          </nav>
        )}
        <nav className="nav bot" aria-label="Global">
          <NavLink to="/connections">Connections</NavLink>
          <NavLink to="/settings">Settings</NavLink>
        </nav>
      </aside>
      <main className="main">
        <Outlet context={{ reloadProjects: projects.reload }} />
      </main>
      <SeemoWidget project={slug ?? null} />
    </div>
  );
}

function ProjectSelector({ projects, current }: { projects: Project[]; current: Project | null }) {
  const [open, setOpen] = useState(false);
  const navigate = useNavigate();
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const onDoc = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    document.addEventListener("mousedown", onDoc);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDoc);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  const initial = (p: Project) => p.name.replace(/^www\./, "")[0]?.toUpperCase() ?? "?";

  if (projects.length === 0) {
    return (
      <button className="sel" onClick={() => navigate("/new")}>
        <span className="sw">+</span>
        <span>
          <b>No project yet</b>
          <small>Add a website to begin</small>
        </span>
      </button>
    );
  }

  return (
    <div className="dd" ref={ref}>
      <button className="sel" aria-haspopup="listbox" aria-expanded={open} onClick={() => setOpen((o) => !o)}>
        <span className="sw">{current ? initial(current) : "·"}</span>
        <span>
          <b>{current ? current.name : "Choose a project"}</b>
          <small>{current ? hostOf(current.url) : `${projects.length} projects`}</small>
        </span>
        <span className="car">{open ? "▴" : "▾"}</span>
      </button>
      {open && (
        <div className="menu" role="listbox">
          {projects.map((p) => (
            <a
              key={p.slug}
              href={`/p/${p.slug}`}
              className={p.slug === current?.slug ? "on" : ""}
              onClick={(e) => {
                e.preventDefault();
                setOpen(false);
                navigate(`/p/${p.slug}`);
              }}
            >
              <span className="sw">{initial(p)}</span>
              {p.name}
            </a>
          ))}
          <a
            href="/new"
            className="add"
            onClick={(e) => {
              e.preventDefault();
              setOpen(false);
              navigate("/new");
            }}
          >
            + Add a project
          </a>
        </div>
      )}
    </div>
  );
}

function hostOf(u: string) {
  try {
    return new URL(u).host.replace(/^www\./, "");
  } catch {
    return u;
  }
}
