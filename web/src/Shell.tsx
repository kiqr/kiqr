import { useEffect, useRef, useState } from "react";
import { NavLink, Outlet, useLocation, useNavigate, useParams } from "react-router-dom";
import { motion } from "motion/react";
import {
  Brain,
  CalendarDays,
  Check,
  ChevronsUpDown,
  Globe,
  Inbox,
  Library,
  ListChecks,
  MessagesSquare,
  Network,
  Plus,
  Rocket,
  Send,
  Sparkles,
  Settings,
  SlidersHorizontal,
  Swords,
  type LucideIcon,
} from "lucide-react";
import { api, useAsync, useEvents, type AssetType, type Project, type TypeCount } from "./api";
import { assetIcon } from "./assetIcons";
import { SeemoWidget } from "./SeemoWidget";
import { TODO_EVENTS } from "./pages/Todo";

type NavItem = { sub: string; label: string; icon: LucideIcon; count?: number; hot?: boolean };

/**
 * Project-scoped navigation. Content is the library plus one item per asset type (types are
 * data, so the group grows when Seemo makes a new kind of thing); what we know about the
 * business; Seemo, which is the automation: chats, the calendar of tasks, abilities, and the channels
 * it publishes through; settings last.
 */
function navGroups(types: AssetType[], counts: TypeCount[] | undefined, waiting: number): { label: string; items: NavItem[] }[] {
  const n = (id: string) => counts?.find((c) => c.type === id)?.count ?? 0;
  // A type appears once the project has something of it; nothing is pinned.
  const shown = types.filter((t) => n(t.id) > 0);
  const total = (counts ?? []).reduce((s, c) => s + c.count, 0);
  // Until the first asset exists, the library is where you get started.
  const fresh = counts !== undefined && total === 0;
  return [
    {
      label: "Library",
      items: [
        { sub: "library", label: fresh ? "Getting started" : "All", icon: fresh ? Rocket : Library, count: total || undefined },
        ...shown.map((t) => ({ sub: `library/${t.id}`, label: t.plural, icon: assetIcon(t.icon), count: n(t.id) || undefined })),
      ],
    },
    {
      label: "Your business",
      items: [
        { sub: "", label: "Business profile", icon: Brain },
        { sub: "channels", label: "Channels", icon: Send },
        { sub: "competitors", label: "Competitors", icon: Swords },
        { sub: "content", label: "Content", icon: Network },
      ],
    },
    {
      label: "Seemo",
      items: [
        { sub: "todo", label: "To do", icon: Inbox, count: waiting || undefined, hot: true },
        { sub: "abilities", label: "Abilities", icon: Sparkles },
        { sub: "calendar", label: "Calendar", icon: CalendarDays },
        { sub: "chats", label: "Chats", icon: MessagesSquare },
        { sub: "tasks", label: "Tasks", icon: ListChecks },
      ],
    },
    {
      label: "Settings",
      items: [{ sub: "settings", label: "Project settings", icon: SlidersHorizontal }],
    },
  ];
}

/** Asset types and this project's counts, kept current from asset events. */
function useAssetNav(slug: string | undefined) {
  const types = useAsync(() => api.assetTypes().catch(() => [] as AssetType[]), []);
  const counts = useAsync(() => (slug ? api.assetSummary(slug).catch(() => [] as TypeCount[]) : Promise.resolve([] as TypeCount[])), [slug]);
  // What waits for the user (approvals, drafts, failed publishes), for the To do count.
  const waiting = useAsync(() => (slug ? api.todo(slug).then((t) => t.count).catch(() => 0) : Promise.resolve(0)), [slug]);
  useEvents(slug ?? null, (e) => {
    if (e.type === "asset_saved" || e.type === "asset_deleted") counts.reload();
    if (e.type === "asset_type_saved") types.reload();
    if (TODO_EVENTS.includes(e.type)) waiting.reload();
  }, { demo: false });
  return navGroups(types.data ?? [], counts.data, waiting.data ?? 0);
}

export function Shell() {
  const { slug } = useParams();
  const projects = useAsync(() => api.projects(), []);
  const list = projects.data ?? [];
  const current = list.find((p) => p.slug === slug) ?? null;
  const { pathname } = useLocation();
  // First run: the onboarding stands alone, centred; the sidebar arrives with the first project.
  const bare = (projects.loading || list.length === 0) && (pathname === "/projects" || pathname === "/new");
  const nav = useAssetNav(current?.slug);

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
            {nav.map((g) => (
              <div className="grp" key={g.label}>
                <p>{g.label}</p>
                {g.items.map((it) => {
                  const to = it.sub ? `/p/${current.slug}/${it.sub}` : `/p/${current.slug}`;
                  return (
                    <NavLink key={it.sub} to={to} end={!it.sub.startsWith("library/")}>
                      {({ isActive }) => (
                        <>
                          {isActive && <motion.span layoutId="nav-active" className="hl" transition={{ type: "spring", stiffness: 500, damping: 40 }} />}
                          <it.icon size={16} />
                          <span>{it.label}</span>
                          {it.count !== undefined && <span className={`ct ${it.hot ? "hot" : ""}`}>{it.count}</span>}
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
