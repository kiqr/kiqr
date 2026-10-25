import { StrictMode, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { BrowserRouter, MemoryRouter, Navigate, Route, Routes } from "react-router-dom";
import { DEMO, DEMO_FRESH } from "./demo";
import "./index.css";
import { Shell } from "./Shell";
import { Onboarding } from "./pages/Onboarding";
import { ProjectPage } from "./pages/Project";
import { CrawlPage } from "./pages/Crawl";
import { Placeholder, ProjectSettings } from "./pages/Settings";
import { Settings } from "./pages/GlobalSettings";
import { ContentPage, Redirect } from "./pages/Content";
import { AbilitiesPage } from "./pages/AbilitiesPage";
import { FirstProjectAbilities } from "./pages/Settings";
import { AssetPage, LibraryPage } from "./pages/Library";
import { ChatsPage } from "./pages/Chats";
import { CalendarPage, TasksPage } from "./pages/Tasks";
import { TodoPage } from "./pages/Todo";
import { ChannelsPage } from "./pages/Channels";

// Demo mode (previews, the kiqr.dev site) keeps routing in memory so the single
// file works anywhere, including sandboxed preview frames with no real URL.
const startParam = typeof location !== "undefined" ? new URLSearchParams(location.search).get("start") : null;
const start = (typeof window !== "undefined" && window.__KIQR_DEMO_START__) || startParam || (DEMO_FRESH ? "/projects" : "/p/unblockingapi-com/crawl");
const Router = DEMO ? ({ children }: { children: ReactNode }) => <MemoryRouter initialEntries={[start]}>{children}</MemoryRouter> : BrowserRouter;

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <Router>
      <Routes>
        <Route element={<Shell />}>
          <Route index element={<Navigate to="/projects" replace />} />
          <Route path="/projects" element={<Onboarding />} />
          <Route path="/new" element={<Onboarding forceNew />} />
          <Route path="/p/:slug" element={<ProjectPage />} />
          <Route path="/p/:slug/crawl" element={<CrawlPage />} />
          <Route path="/p/:slug/content" element={<ContentPage />} />
          <Route path="/p/:slug/map" element={<Redirect to="content" />} />
          <Route path="/p/:slug/gaps" element={<Redirect to="content#gaps" />} />
          <Route path="/p/:slug/library" element={<LibraryPage />} />
          <Route path="/p/:slug/library/:type" element={<LibraryPage />} />
          <Route path="/p/:slug/library/:type/:id" element={<AssetPage />} />
          {/* Older links: the overview and per-format pages became the library. */}
          <Route path="/p/:slug/overview" element={<Redirect to="library" />} />
          <Route path="/p/:slug/articles" element={<Redirect to="library/article" />} />
          <Route path="/p/:slug/posts" element={<Redirect to="library/post" />} />
          <Route path="/p/:slug/competitors" element={<Placeholder title="Competitors" text="Competitor profiles arrive in a later release: Seemo will crawl the sites you name and position you against them." />} />
          <Route path="/p/:slug/chats" element={<ChatsPage />} />
          <Route path="/p/:slug/chats/:id" element={<ChatsPage />} />
          <Route path="/p/:slug/calendar" element={<CalendarPage />} />
          <Route path="/p/:slug/tasks" element={<TasksPage />} />
          <Route path="/p/:slug/todo" element={<TodoPage />} />
          <Route path="/p/:slug/skills" element={<Redirect to="abilities" />} />
          <Route path="/p/:slug/abilities" element={<AbilitiesPage />} />
          <Route path="/p/:slug/channels" element={<ChannelsPage />} />
          <Route path="/p/:slug/connectors" element={<Redirect to="channels" />} />
          <Route path="/p/:slug/settings" element={<ProjectSettings />} />
          {/* Abilities moved under each project; these older links follow. */}
          <Route path="/connections" element={<FirstProjectAbilities />} />
          <Route path="/settings/connections" element={<FirstProjectAbilities />} />
          <Route path="/settings/abilities" element={<FirstProjectAbilities />} />
          <Route path="/settings" element={<Settings />} />
          <Route path="/settings/:tab" element={<Settings />} />
        </Route>
      </Routes>
    </Router>
  </StrictMode>,
);
