import { NavLink, Outlet, useParams } from "react-router-dom";
import { api, useAsync } from "./api";
import { SeemoWidget } from "./SeemoWidget";

export function Shell() {
  const { slug } = useParams();
  const projects = useAsync(() => api.projects(), []);
  const list = projects.data ?? [];
  return (
    <div className="shell">
      <aside className="sidebar">
        <div className="wordmark">KIQR</div>
        <nav aria-label="Projects">
          {list.length > 0 && <span className="section-label">Projects</span>}
          {list.map((p) => (
            <NavLink key={p.slug} to={`/p/${p.slug}`} className={({ isActive }) => (isActive || slug === p.slug ? "active" : "")}>
              {p.name}
            </NavLink>
          ))}
          <NavLink to="/new">Add a project</NavLink>
        </nav>
        <div className="grow" />
        <nav>
          <NavLink to="/settings">Settings</NavLink>
        </nav>
      </aside>
      <main className="content">
        <Outlet context={{ reloadProjects: projects.reload }} />
      </main>
      <SeemoWidget project={slug ?? null} />
    </div>
  );
}
