// The first win: Seemo drafts the marketing plan (approved in its card, in the
// corner) and writes three posts in the business's own brand, right here.
import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { ArrowRight, Check, Loader2, MessageSquare, Target } from "lucide-react";
import { api, useEvents, type Asset, type Brand, type Plan } from "../api";
import { ASK_FOR_PLAN } from "../pages/Plan";
import { PostCard } from "../pages/PostCard";
import { openSeemo } from "../SeemoWidget";

const POSTS = 3;
const started = new Set<string>();

export function FirstWeek({ slug, brand }: { slug: string; brand: Brand | null }) {
  const reduced = useReducedMotion();
  const navigate = useNavigate();
  const [chat, setChat] = useState<string | null>(null);
  const [planState, setPlanState] = useState<"asking" | "card" | "saved" | "failed">("asking");
  const [plan, setPlan] = useState<Plan | null>(null);
  const [posts, setPosts] = useState<Asset[]>([]);
  const [postError, setPostError] = useState<string | null>(null);
  const [postsDone, setPostsDone] = useState(false);
  const [leaving, setLeaving] = useState(false);

  // Both start once, even if this screen is shown again.
  useEffect(() => {
    if (started.has(slug)) return;
    started.add(slug);
    api
      .createChat(slug, "Your first marketing plan")
      .then(async (c) => {
        setChat(c.id);
        openSeemo(c.id, { expanded: true });
        await api.sendMessage(slug, c.id, `${ASK_FOR_PLAN} This is our first plan together: keep it to what fits the time I have.`);
      })
      .catch(() => setPlanState("failed"));
    api.generateAssets(slug, "post", POSTS).catch((e: Error) => setPostError(e.message));
  }, [slug]);

  // The newest posts (a new project has no others), and the plan once it is approved.
  const loadPosts = () =>
    api.assets(slug, "post").then((all) => setPosts([...all].sort((a, b) => b.created.at.localeCompare(a.created.at)).slice(0, POSTS)), () => {});
  useEffect(() => {
    void loadPosts();
    api.plan(slug).then((v) => v.plan && (setPlan(v.plan), setPlanState("saved")), () => {});
  }, [slug]);
  useEvents(slug, (e) => {
    if (e.type === "asset_saved" && e.asset_type === "post") void loadPosts();
    if (e.type === "job_finished" && e.job === "posts") setPostsDone(true);
    if (e.type === "approval_requested" && e.kind === "plan") setPlanState((s) => (s === "saved" ? s : "card"));
    if (e.type === "plan_saved") api.plan(slug).then((v) => (setPlan(v.plan), setPlanState("saved")), () => {});
    if (e.type === "chat_turn_finished" && e.chat === chat && e.error) setPlanState((s) => (s === "saved" ? s : "failed"));
  });

  const open = async () => {
    setLeaving(true);
    await new Promise((r) => setTimeout(r, reduced ? 0 : 380));
    navigate(`/p/${slug}/todo`, { state: { welcome: true } });
  };

  return (
    <div className={`wl-week ${leaving ? "leaving" : ""}`}>
      <div className="wl-reveal-h">
        <p className="cap">Your first week</p>
        <h1 className="d1">
          Let's get <em>to work.</em>
        </h1>
        <p className="lead">I'm drafting your plan and writing your first posts, in your brand. You approve what goes out.</p>
      </div>

      <section className="wl-plan pcard">
        <header>
          <span className="ic">
            <Target size={16} />
          </span>
          <h3>Your marketing plan</h3>
          {planState === "saved" && (
            <span className="chip ok sm">
              <Check size={11} strokeWidth={3} /> Approved
            </span>
          )}
        </header>
        {planState === "asking" && (
          <p className="muted">
            <Loader2 size={13} className="spin" /> Reading what you told me and writing a plan for {brand?.site_name || "your business"}…
          </p>
        )}
        {planState === "card" && (
          <div className="row">
            <p className="muted">It's ready. Look it over in the chat and approve it, or ask me to change something.</p>
            {chat && (
              <button className="btn p sm" onClick={() => openSeemo(chat, { expanded: true })}>
                <MessageSquare size={13} /> Open the plan
              </button>
            )}
          </div>
        )}
        {planState === "failed" && (
          <div className="row">
            <p className="muted">I couldn't write the plan just now.</p>
            {chat && (
              <button className="btn g sm" onClick={() => openSeemo(chat, { expanded: true })}>
                See why
              </button>
            )}
          </div>
        )}
        {planState === "saved" && plan && (
          <ol className="wl-objectives">
            {plan.objectives
              .filter((o) => o.status === "active")
              .map((o, i) => (
                <motion.li key={o.id} initial={reduced ? false : { opacity: 0, x: -8 }} animate={{ opacity: 1, x: 0 }} transition={{ delay: i * 0.08 }}>
                  <b>{o.title}</b>
                  <span className="mono dim sm">{o.kpi}</span>
                </motion.li>
              ))}
          </ol>
        )}
      </section>

      <section className="wl-posts">
        <header>
          <h3>Your first posts</h3>
          <span className="dim sm mono">
            {posts.length}/{POSTS}
          </span>
        </header>
        {postError ? (
          <p className="muted">{postError}</p>
        ) : (
          <div className="wl-post-grid">
            <AnimatePresence>
              {posts.map((a) => (
                <motion.div key={a.id} layout initial={reduced ? false : { opacity: 0, y: 16, scale: 0.98 }} animate={{ opacity: 1, y: 0, scale: 1 }} transition={{ duration: 0.35 }}>
                  <PostCard slug={slug} a={a} brand={brand} accent={brand?.colors[0]} />
                </motion.div>
              ))}
            </AnimatePresence>
            {!postsDone &&
              Array.from({ length: Math.max(0, POSTS - posts.length) }).map((_, i) => (
                <div key={`ph${i}`} className="wl-post-ph shimmer">
                  <Loader2 size={14} className="spin" /> Writing…
                </div>
              ))}
          </div>
        )}
        {posts.length > 0 && <p className="dim sm">They wait as drafts in your Library until you approve them for a channel.</p>}
      </section>

      <div className="wl-cta">
        <button className="btn a wl-big" onClick={() => void open()}>
          Open your marketing department <ArrowRight size={16} />
        </button>
        {planState !== "saved" && <p className="dim sm">Your plan keeps going in the chat while you look around.</p>}
      </div>
    </div>
  );
}
