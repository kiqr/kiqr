// The first win: Seemo drafts the marketing plan (approved in its card, in the
// corner). Posts come once an account is connected, written for that place:
// the screen points at the connect grid.
import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { motion, useReducedMotion } from "motion/react";
import { ArrowRight, Cable, Check, Loader2, MessageSquare, Target } from "lucide-react";
import { api, useEvents, type Brand, type Plan } from "../api";
import { connectPath } from "../pages/Accounts";
import { ASK_FOR_PLAN } from "../pages/Plan";
import { openSeemo } from "../SeemoWidget";

const started = new Set<string>();

export function FirstWeek({ slug, brand }: { slug: string; brand: Brand | null }) {
  const reduced = useReducedMotion();
  const navigate = useNavigate();
  const [chat, setChat] = useState<string | null>(null);
  const [planState, setPlanState] = useState<"asking" | "card" | "saved" | "failed">("asking");
  const [plan, setPlan] = useState<Plan | null>(null);
  const [leaving, setLeaving] = useState(false);

  // Starts once, even if this screen is shown again.
  useEffect(() => {
    if (started.has(slug)) return;
    started.add(slug);
    api
      .createChat(slug, "Your first marketing plan")
      .then(async (c) => {
        setChat(c.id);
        openSeemo(c.id);
        await api.sendMessage(slug, c.id, `${ASK_FOR_PLAN} This is our first plan together: keep it to what fits the time I have.`);
      })
      .catch(() => setPlanState("failed"));
  }, [slug]);

  // The plan once it is approved.
  useEffect(() => {
    api.plan(slug).then((v) => v.plan && (setPlan(v.plan), setPlanState("saved")), () => {});
  }, [slug]);
  useEvents(slug, (e) => {
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
        <p className="lead">I'm drafting your plan, in your brand. Connect where you publish and I write your first posts for each place. You approve what goes out.</p>
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
              <button className="btn p sm" onClick={() => openSeemo(chat)}>
                <MessageSquare size={13} /> Open the plan
              </button>
            )}
          </div>
        )}
        {planState === "failed" && (
          <div className="row">
            <p className="muted">I couldn't write the plan just now.</p>
            {chat && (
              <button className="btn g sm" onClick={() => openSeemo(chat)}>
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

      <section className="wl-posts pcard">
        <header>
          <span className="ic">
            <Cable size={16} />
          </span>
          <h3>Where you publish</h3>
        </header>
        <p className="muted">Connect X, Reddit, your blog or your mail account, and I write the first posts for each of them, the way that place reads. Until then I write nothing for nowhere.</p>
        <div className="row">
          <button className="btn g sm" onClick={() => navigate(connectPath(slug, { group: "social" }))}>
            <Cable size={13} /> Connect an account
          </button>
        </div>
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
