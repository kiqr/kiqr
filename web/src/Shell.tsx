import { useEffect, useRef, useState } from "react";
import { Navigate, NavLink, Outlet, useLocation, useNavigate, useParams } from "react-router-dom";
import { motion } from "motion/react";
import {
  BarChart3,
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
  Send,
  Sparkles,
  Settings,
  SlidersHorizontal,
  Swords,
  Target,
  type LucideIcon,
} from "lucide-react";
import { DEMO } from "./demo";
import { api, useAsync, useEvents, type AssetType, type Project, type TypeCount } from "./api";
import { assetIcon } from "./assetIcons";
import { SeemoWidget } from "./SeemoWidget";
import { CloudStatus } from "./CloudStatus";
import { TODO_EVENTS } from "./pages/Todo";
import { Moving, addressUrl } from "./pages/GlobalSettings";
import { Logo } from "./Logo";

type NavItem = { sub: string; label: string; icon: LucideIcon; count?: number; hot?: boolean };

/**
 * Project-scoped navigation, grouped by what the user is doing: To do (what needs them) on
 * its own; Marketing (plan, performance, calendar); the Library plus one item per asset type
 * (types are data, so the group grows when Seemo makes a new kind of thing); Business (what
 * KIQR knows); Seemo (chats and tasks); Setup (abilities, channels, project settings) last.
 */
function navGroups(types: AssetType[], counts: TypeCount[] | undefined, waiting: number): { label: string; items: NavItem[] }[] {
  const n = (id: string) => counts?.find((c) => c.type === id)?.count ?? 0;
  // A type appears once the project has something of it; nothing is pinned.
  const shown = types.filter((t) => n(t.id) > 0);
  const total = (counts ?? []).reduce((s, c) => s + c.count, 0);
  return [
    // What needs you, first and on its own.
    { label: "", items: [{ sub: "todo", label: "To do", icon: Inbox, count: waiting || undefined, hot: true }] },
    // Direction, results, and when things happen.
    {
      label: "Marketing",
      items: [
        { sub: "plan", label: "Plan", icon: Target },
        { sub: "performance", label: "Performance", icon: BarChart3 },
        { sub: "calendar", label: "Calendar", icon: CalendarDays },
      ],
    },
    // What has been made.
    {
      label: "Library",
      items: [
        { sub: "library", label: "All", icon: Library, count: total || undefined },
        ...shown.map((t) => ({ sub: `library/${t.id}`, label: t.plural, icon: assetIcon(t.icon), count: n(t.id) || undefined })),
      ],
    },
    // What KIQR knows about the business and its site.
    {
      label: "Business",
      items: [
        { sub: "", label: "Business profile", icon: Brain },
        { sub: "competitors", label: "Competitors", icon: Swords },
        { sub: "content", label: "Content", icon: Network },
      ],
    },
    // Talking to Seemo and what it runs on its own.
    {
      label: "Seemo",
      items: [
        { sub: "chats", label: "Chats", icon: MessagesSquare },
        { sub: "tasks", label: "Tasks", icon: ListChecks },
      ],
    },
    // Installed once, configured per project, rarely visited.
    {
      label: "Setup",
      items: [
        { sub: "abilities", label: "Abilities", icon: Sparkles },
        { sub: "channels", label: "Channels", icon: Send },
        { sub: "settings", label: "Project settings", icon: SlidersHorizontal },
      ],
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
  // Opening the app reads the marketplace index again, in the background; pages listen for the event.
  useEffect(() => {
    if (!DEMO) api.refreshMarketplace().catch(() => {});
  }, []);
  const projects = useAsync(() => api.projects(), []);
  // A project's first build moving on decides whether its pages are open yet.
  useEvents(null, (e) => e.type === "build_stage" && projects.reload(), { demo: false });
  const updateAvailable = useUpdateAvailable();
  const moving = useAddressMove();
  const list = projects.data ?? [];
  const current = list.find((p) => p.slug === slug) ?? null;
  const { pathname, state } = useLocation();
  // First run: the onboarding stands alone, centred; so does a project's first build (the welcome flow).
  const welcome = pathname.startsWith("/welcome/");
  const bare = welcome || ((projects.loading || list.length === 0) && (pathname === "/projects" || pathname === "/new"));
  // Fresh from the welcome flow: the sidebar arrives with a little ceremony.
  const arriving = (state as { welcome?: boolean } | null)?.welcome === true;
  const nav = useAssetNav(current?.slug);

  // Until its first build is done a project is the welcome flow, whatever page was asked for.
  if (!DEMO && current?.build && current.build.stage !== "ready" && pathname.startsWith(`/p/${current.slug}`) && !arriving) {
    return <Navigate to={`/welcome/${current.slug}`} replace />;
  }

  return (
    <div className={`shell ${bare ? "bare" : ""} ${arriving ? "arriving" : ""}`}>
      <aside className="side">
        <div className="mark">
          <Logo />
        </div>
        <ProjectSwitcher projects={list} current={current} />
        {current && (
          <nav className="nav" aria-label="Project">
            {nav.map((g) => (
              <div className="grp" key={g.label || g.items[0].sub}>
                {g.label && <p>{g.label}</p>}
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
          <CloudStatus />
          <NavLink to="/settings">
            <Settings size={16} /> Settings
            {updateAvailable && <span className="update-dot" title="A new version of KIQR is available" />}
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
      {moving && <Moving to={moving.to} pid={moving.pid} />}
    </div>
  );
}

/** When KIQR's address changes (in Settings or by Seemo), the daemon restarts there; follow it. */
function useAddressMove() {
  const [moving, setMoving] = useState<{ to: string; pid?: number } | null>(null);
  useEvents(
    null,
    (e) => {
      if (e.type !== "config_changed" || !e.restart) return;
      Promise.all([api.config(), api.health()]).then(([c, h]) => setMoving({ to: addressUrl(c.daemon), pid: h.pid })).catch(() => {});
    },
    { demo: false },
  );
  return moving;
}

/** Whether a newer KIQR is out, for the dot on Settings. */
function useUpdateAvailable() {
  const health = useAsync(() => api.health().catch(() => null), []);
  useEvents(null, (e) => e.type === "update_available" && health.reload(), { demo: false });
  return !!health.data?.update?.available;
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
