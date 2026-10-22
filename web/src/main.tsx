import { StrictMode, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { BrowserRouter, MemoryRouter, Navigate, Route, Routes } from "react-router-dom";
import { DEMO } from "./demo";
import "./index.css";
import { Shell } from "./Shell";
import { Onboarding } from "./pages/Onboarding";
import { ProjectPage } from "./pages/Project";
import { CrawlPage } from "./pages/Crawl";
import { Connections, Placeholder, ProjectSettings, Settings } from "./pages/Settings";

// Demo mode (previews, the kiqr.dev site) keeps routing in memory so the single
// file works anywhere, including sandboxed preview frames with no real URL.
const start = (typeof window !== "undefined" && window.__KIQR_DEMO_START__) || "/p/unblockingapi-com/crawl";
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
          <Route path="/p/:slug/competitors" element={<Placeholder title="Competitors" text="Competitor profiles arrive in a later release: Seemo will crawl the sites you name and position you against them." />} />
          <Route path="/p/:slug/tasks" element={<Placeholder title="Tasks" text="Marketing tasks arrive once the profile is solid: content, social posts, email, ads." />} />
          <Route path="/p/:slug/chats" element={<Placeholder title="Chats with Seemo" text="Conversations are saved per project and will be listed here." />} />
          <Route path="/p/:slug/settings" element={<ProjectSettings />} />
          <Route path="/connections" element={<Connections />} />
          <Route path="/settings" element={<Settings />} />
        </Route>
      </Routes>
    </Router>
  </StrictMode>,
);
