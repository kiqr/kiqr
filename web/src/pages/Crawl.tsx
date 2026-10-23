import { useLocation, useParams } from "react-router-dom";
import { CrawlLive } from "../crawl/CrawlLive";

/** /p/:slug/crawl — the live crawl screen. Components live in src/crawl/. */
export function CrawlPage() {
  const { slug = "" } = useParams();
  const fresh = (useLocation().state as { fresh?: boolean } | null)?.fresh === true;
  return <CrawlLive key={slug} slug={slug} fresh={fresh} />;
}
