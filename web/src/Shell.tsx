import { useEffect, useRef, useState } from "react";
import { NavLink, Outlet, useLocation, useNavigate, useParams } from "react-router-dom";
import { motion } from "motion/react";
import {
  Brain,
  CalendarDays,
  Check,
  ChevronsUpDown,
  Gauge,
  Globe,
  ListChecks,
  MessageSquareText,
  MessagesSquare,
  Network,
  Newspaper,
  Plug,
  Plus,
  Radar,
  SearchX,
  Settings,
  SlidersHorizontal,
  Swords,
  type LucideIcon,
} from "lucide-react";
import { api, useAsync, type Project } from "./api";
import { SeemoWidget } from "./SeemoWidget";

/** Project-scoped navigation. Content first; what we know about the business second; Seemo; settings last. */
const NAV: { label: string; items: { sub: string; label: string; icon: LucideIcon }[] }[] = [
  {
    label: "Content",
    items: [
      { sub: "overview", label: "Overview", icon: Gauge },
      { sub: "calendar", label: "Calendar", icon: CalendarDays },
      { sub: "articles", label: "Articles", icon: Newspaper },
      { sub: "posts", label: "Posts", icon: MessageSquareText },
      { sub: "map", label: "Content map", icon: Network },
      { sub: "gaps", label: "Content gaps", icon: SearchX },
    ],
  },
  {
    label: "Your business",
    items: [
      { sub: "", label: "Business profile", icon: Brain },
      { sub: "crawl", label: "Website crawls", icon: Radar },
      { sub: "competitors", label: "Competitors", icon: Swords },
    ],
  },
  {
    label: "Seemo",
    items: [
      { sub: "chats", label: "Chats with Seemo", icon: MessagesSquare },
      { sub: "tasks", label: "Tasks", icon: ListChecks },
    ],
  },
  {
    label: "Settings",
    items: [{ sub: "settings", label: "Project settings", icon: SlidersHorizontal }],
  },
];

export function Shell() {
  const { slug } = useParams();
  const projects = useAsync(() => api.projects(), []);
  const list = projects.data ?? [];
  const current = list.find((p) => p.slug === slug) ?? null;
  const { pathname } = useLocation();
  // First run: the onboarding stands alone, centred; the sidebar arrives with the first project.
  const bare = (projects.loading || list.length === 0) && (pathname === "/projects" || pathname === "/new");

  return (
    <div className={`shell ${bare ? "bare" : ""}`}>
      <aside className="side">
        <div className="mark">
          <i />
          KIQR
        </div>
        <ProjectSwitcher projects={list} current={current} />
        {current && (
          <nav className="nav" aria-label="Project">
            {NAV.map((g) => (
              <div className="grp" key={g.label}>
                <p>{g.label}</p>
                {g.items.map((it) => {
                  const to = it.sub ? `/p/${current.slug}/${it.sub}` : `/p/${current.slug}`;
                  return (
                    <NavLink key={it.sub} to={to} end>
                      {({ isActive }) => (
                        <>
                          {isActive && <motion.span layoutId="nav-active" className="hl" transition={{ type: "spring", stiffness: 500, damping: 40 }} />}
                          <it.icon size={16} />
                          <span>{it.label}</span>
                        </>
                      )}
                    </NavLink>
                  );
                })}
              </div>
            ))}
          </nav>
        )}
        <div className="side-foot">
          <NavLink to="/connections">
            <Plug size={16} /> Connections
          </NavLink>
          <NavLink to="/settings">
            <Settings size={16} /> Settings
          </NavLink>
          <div className="who">
            <span>Runs on this machine</span>
            <span>~/.kiqr</span>
          </div>
        </div>
      </aside>
      <main className="main">
        <div key={pathname} className="page">
          <Outlet context={{ reloadProjects: projects.reload }} />
        </div>
      </main>
      <SeemoWidget project={slug ?? null} />
    </div>
  );
}

function ProjectSwitcher({ projects, current }: { projects: Project[]; current: Project | null }) {
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

  const go = (to: string) => (e: React.MouseEvent) => {
    e.preventDefault();
    setOpen(false);
    navigate(to);
  };

  return (
    <div className="dd" ref={ref}>
      <button className="sel" aria-haspopup="listbox" aria-expanded={open} onClick={() => (projects.length ? setOpen((o) => !o) : navigate("/new"))}>
        <span className="sw">
          <Globe size={15} />
        </span>
        <span className="t">
          <small>Project</small>
          <b>{current ? current.name : projects.length ? "Select a project" : "Add a project"}</b>
        </span>
        <ChevronsUpDown size={14} className="car" />
      </button>
      {open && (
        <div className="menu" role="listbox">
          <ul>
            {projects.map((p) => (
              <li key={p.slug}>
                <a href={`/p/${p.slug}`} onClick={go(`/p/${p.slug}`)}>
                  {p.name}
                  {p.slug === current?.slug && <Check size={14} className="ck" />}
                </a>
              </li>
            ))}
          </ul>
          <a href="/new" className="add" onClick={go("/new")}>
            <Plus size={14} /> Add project
          </a>
        </div>
      )}
    </div>
  );
}
