import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { BrowserRouter, Navigate, Route, Routes } from "react-router-dom";
import "./index.css";
import { Shell } from "./Shell";
import { Onboarding } from "./pages/Onboarding";
import { ProjectPage } from "./pages/Project";
import { CrawlPage } from "./pages/Crawl";
import { Settings } from "./pages/Settings";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <BrowserRouter>
      <Routes>
        <Route element={<Shell />}>
          <Route index element={<Navigate to="/projects" replace />} />
          <Route path="/projects" element={<Onboarding />} />
          <Route path="/new" element={<Onboarding forceNew />} />
          <Route path="/p/:slug" element={<ProjectPage />} />
          <Route path="/p/:slug/crawl" element={<CrawlPage />} />
          <Route path="/settings" element={<Settings />} />
        </Route>
      </Routes>
    </BrowserRouter>
  </StrictMode>,
);
