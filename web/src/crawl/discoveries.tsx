import type { ReactNode } from "react";
import { AnimatePresence, motion } from "motion/react";
import { Gauge, MapIcon, ShieldCheck, Zap } from "lucide-react";
import type { CrawlState } from "./store";
import { fmt } from "./tickers";

/** Pop-in badges for what we learned before/while crawling. */
export function Discoveries({ state, reducedMotion }: { state: CrawlState; reducedMotion: boolean }) {
  const items: { key: string; icon: ReactNode; title: string; detail?: string }[] = [];
  if (state.phase !== "connecting") items.push({ key: "robots", icon: <ShieldCheck size={14} />, title: "robots.txt respected" });
  if (state.sitemapUrls > 0) items.push({ key: "sitemap", icon: <MapIcon size={14} />, title: `${fmt.format(state.sitemapUrls)} URLs from sitemaps` });
  if (state.concurrency)
    items.push({
      key: "parallel",
      icon: <Gauge size={14} />,
      title: `${state.concurrency} fetches in parallel`,
      detail: state.aiConcurrency ? `${state.aiConcurrency} pages read by AI at a time` : undefined,
    });
  if (state.unblocked > 0)
    items.push({
      key: "unblock",
      icon: <Zap size={14} />,
      title: "UnblockingAPI escalation",
      detail: `${fmt.format(state.unblocked)} ${state.unblocked === 1 ? "page" : "pages"} fetched through a real browser`,
    });
  if (!items.length) return null;
  return (
    <div className="cr-disc">
      <AnimatePresence>
        {items.map((it) => (
          <motion.div
            key={it.key}
            layout={!reducedMotion}
            initial={reducedMotion ? false : { opacity: 0, scale: 0.7, y: 8 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            transition={{ type: "spring", stiffness: 380, damping: 22 }}
            className="cr-badge"
            title={it.detail}
          >
            <span className="ic">{it.icon}</span>
            <span>{it.title}</span>
          </motion.div>
        ))}
      </AnimatePresence>
    </div>
  );
}
