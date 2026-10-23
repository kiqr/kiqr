import { AnimatePresence, motion } from "motion/react";
import { pathOf, type CrawlState, type FeedItem } from "./store";
import { fmt } from "./tickers";

function StatusPill({ it }: { it: FeedItem }) {
  const label = it.kind === "fetched" ? String(it.status) : it.kind === "blocked" ? "BLOCK" : it.kind === "skipped" ? "OUT" : "ERR";
  const tone = it.kind === "failed" || (it.status ?? 0) >= 400 ? "bad" : it.kind === "blocked" ? "warn" : it.kind === "skipped" ? "skip" : "ok";
  return <span className={`cr-pill ${tone}`}>{label}</span>;
}

/** Live list of processed URLs; newest slides in at the top. */
export function UrlFeed({ state, reducedMotion }: { state: CrawlState; reducedMotion: boolean }) {
  const items = state.feed.slice(0, 60);
  return (
    <div className="cr-feed">
      <ul>
        <AnimatePresence initial={false} mode="popLayout">
          {items.map((it) => (
            <motion.li
              key={it.key}
              layout={!reducedMotion}
              initial={reducedMotion ? false : { opacity: 0, x: -24, backgroundColor: "rgba(94,234,212,0.18)" }}
              animate={{ opacity: 1, x: 0, backgroundColor: "rgba(94,234,212,0)" }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.35, ease: [0.2, 0.8, 0.2, 1] }}
            >
              <StatusPill it={it} />
              <span className="cr-feed-main">
                <span className="cr-feed-path">
                  {pathOf(it.url)}
                  {it.via === "unblockingapi" && <i className="cr-via">UnblockingAPI</i>}
                </span>
                {(it.title || it.reason) && <span className="cr-feed-sub">{it.title || it.reason}</span>}
              </span>
              {it.kind === "fetched" && (
                <span className="cr-feed-meta">
                  {it.words !== undefined && (
                    <>
                      {fmt.format(it.words)}w<br />
                    </>
                  )}
                  {it.ms !== undefined && `${it.ms}ms`}
                </span>
              )}
            </motion.li>
          ))}
        </AnimatePresence>
      </ul>
      <div className="cr-feed-fade" />
    </div>
  );
}

/** Terminal-style line showing what's being fetched right now. */
export function NowFetching({ state }: { state: CrawlState }) {
  const urls = [...state.fetching].slice(-4);
  return (
    <div className="cr-now">
      {urls.length === 0 ? (
        <p className="idle">
          {state.phase === "learning"
            ? "▸ writing the business profile…"
            : state.phase === "done"
              ? "▸ done"
              : state.analysis.waiting > 0
                ? `▸ reading ${state.analysis.waiting} ${state.analysis.waiting === 1 ? "page" : "pages"} with AI…`
                : "▸ waiting for work…"}
          <span className="cr-cursor" />
        </p>
      ) : (
        urls.map((u) => {
          const n = state.nodes.get(u);
          return (
            <p key={u}>
              <span className="dim">{n?.status === "rendering" ? "unblock" : "GET"} </span>
              {pathOf(u)}
              <span className="cr-cursor" />
            </p>
          );
        })
      )}
    </div>
  );
}
