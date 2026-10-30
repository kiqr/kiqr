// The empty library: a getting-started guide, and a showcase of what Seemo will make for this
// business, written from its own profile so it already feels like theirs.
import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { motion } from "motion/react";
import {
  ArrowRight, AudioLines, BarChart3, Brain, Cable, CalendarClock, Check, Clapperboard, FileText, Film, Gift, Image, Laugh, LayoutTemplate, Mail,
  Loader2, Megaphone, MessageSquareText, MessagesSquare, Mic, Music, Newspaper, PenLine, Radar, Reply, ShoppingBag, SkipForward, Sparkles, Target, Undo2,
  type LucideIcon,
} from "lucide-react";
import { api, useAsync, useEvents, type Asset } from "../api";
import { openSeemo } from "../SeemoWidget";
import { DEMO } from "../demo";

type Idea = { icon: LucideIcon; name: string; example: string };
type Group = { label: string; tone: string; ideas: Idea[] };

function groups(name: string, offering: string, gap: string, audience: string): Group[] {
  return [
    {
      label: "Writing",
      tone: "#5eead4",
      ideas: [
        { icon: Newspaper, name: "Blog articles", example: `“${gap}”, written from the content gap, in ${name}'s voice` },
        { icon: Mail, name: "Newsletters", example: `A monthly letter to ${audience.toLowerCase()} about what's new` },
        { icon: LayoutTemplate, name: "Landing pages", example: `A page that sells ${offering}, with headline variants to test` },
        { icon: ShoppingBag, name: "Product descriptions", example: `Short and long copy for ${offering}` },
        { icon: FileText, name: "FAQ pages", example: "The questions customers ask, answered from your own pages" },
      ],
    },
    {
      label: "Social",
      tone: "#818cf8",
      ideas: [
        { icon: MessageSquareText, name: "X threads", example: `A five-post thread: “${gap}”` },
        { icon: PenLine, name: "LinkedIn posts", example: `A founder's take on why ${name} exists` },
        { icon: Reply, name: "Reddit replies", example: "Helpful answers in the communities your customers ask in" },
        { icon: Image, name: "Instagram posts", example: `Carousel and caption for ${offering}` },
      ],
    },
    {
      label: "Visual",
      tone: "#f472b6",
      ideas: [
        { icon: Laugh, name: "Memes", example: `The one about ${audience.toLowerCase()} everyone will recognise` },
        { icon: Sparkles, name: "Social images", example: "Quote cards and announcements in your brand colours" },
        { icon: Image, name: "Banners", example: "Hero images and ad creatives sized for every channel" },
        { icon: BarChart3, name: "Infographics", example: "Your numbers and process, drawn as one shareable image" },
      ],
    },
    {
      label: "Video and audio",
      tone: "#fbbf24",
      ideas: [
        { icon: Clapperboard, name: "Short videos", example: `A 20-second reel showing ${offering}` },
        { icon: Film, name: "YouTube scripts", example: `A how-to video script: “${gap}”` },
        { icon: Music, name: "Jingles and songs", example: `A Christmas song about ${name}` },
        { icon: Mic, name: "Voice-overs", example: "Narration for your videos and ads" },
        { icon: AudioLines, name: "Podcast clips", example: "Cut-downs with captions from a longer episode" },
      ],
    },
    {
      label: "Ads",
      tone: "#34d399",
      ideas: [
        { icon: Target, name: "Google Ads campaigns", example: `Ad groups, keywords and responsive ads for ${offering}` },
        { icon: Megaphone, name: "Meta ads", example: `Creative and copy aimed at ${audience.toLowerCase()}` },
      ],
    },
    {
      label: "On a schedule",
      tone: "#60a5fa",
      ideas: [
        { icon: CalendarClock, name: "Morning report", example: "Every day at 07:00: what was published, what worked, what's next" },
        { icon: Gift, name: "Christmas calendar", example: "Every day in December until the 24th, today's post" },
        { icon: Radar, name: "Community watch", example: "Weekdays at 10:00: find posts to reply to on Reddit" },
      ],
    },
  ];
}

export function EmptyLibrary({ slug }: { slug: string }) {
  const project = useAsync(() => api.project(slug), [slug]);
  const brief = useAsync(() => api.brief(slug).catch(() => null), [slug]);
  const b = brief.data;
  const name = b?.business_name || project.data?.name || "your business";
  const offering = b?.offerings[0]?.name || "your best seller";
  const gap = b?.content_gaps[0]?.topic || "the question customers ask most";
  const audience = b?.audiences[0]?.name || "your customers";

  return (
    <div className="lib-empty">
      <div className="lib-can-head">
        <div>
          <p className="cap">What Seemo can make</p>
          <h2>Everything {name} needs to be seen</h2>
          <p className="muted">Ideas written from your business profile. They become real assets in this library as Seemo learns each one.</p>
        </div>
      </div>
      {groups(name, offering, gap, audience).map((g, gi) => (
        <section key={g.label} className="lib-can-grp" style={{ "--tone": g.tone } as React.CSSProperties}>
          <p className="lib-can-label">
            <i /> {g.label}
          </p>
          <div className="lib-can-grid">
            {g.ideas.map((it, i) => (
              <motion.div
                key={it.name}
                className="lib-can-it"
                initial={{ opacity: 0, y: 10 }}
                whileInView={{ opacity: 1, y: 0 }}
                viewport={{ once: true, margin: "-40px" }}
                transition={{ delay: (gi === 0 ? 0.25 : 0) + i * 0.05, duration: 0.35, ease: [0.2, 0.8, 0.2, 1] }}
              >
                <span className="ic">
                  <it.icon size={16} />
                </span>
                <div className="min0">
                  <p className="nm">
                    {it.name}
                    <span className="soon">Coming soon</span>
                  </p>
                  <p className="ds">{it.example}</p>
                </div>
              </motion.div>
            ))}
          </div>
        </section>
      ))}
    </div>
  );
}

type StepState = "done" | "now" | "next" | "soon" | "skipped";
type Step = {
  id: string;
  icon: LucideIcon;
  title: string;
  text: string;
  state: StepState;
  /** Optional steps can be skipped; the rest are needed for everything else. */
  optional?: boolean;
  to?: string;
  action?: string;
  run?: () => void;
  busy?: boolean;
};

/** Skipped steps are a per-viewer convenience, kept in the browser. */
function useSkipped(slug: string) {
  const key = `kiqr.guide.skipped.${slug}`;
  const read = () => {
    try {
      return JSON.parse(localStorage.getItem(key) ?? "[]") as string[];
    } catch {
      return [];
    }
  };
  const [skipped, set] = useState<string[]>(read);
  useEffect(() => set(read()), [key]); // eslint-disable-line react-hooks/exhaustive-deps
  const toggle = (id: string) =>
    set((cur) => {
      const next = cur.includes(id) ? cur.filter((x) => x !== id) : [...cur, id];
      try {
        localStorage.setItem(key, JSON.stringify(next));
      } catch {
        /* private window: skipping lasts until reload */
      }
      return next;
    });
  return { skipped, toggle };
}

/**
 * Six steps from a crawled website to marketing that runs itself. Shown at the top of To do
 * until every step is done or skipped (`hideWhenComplete`); `onComplete` tells the page.
 */
export function Guide({ slug, hideWhenComplete, onComplete }: { slug: string; hideWhenComplete?: boolean; onComplete?: (done: boolean) => void }) {
  const project = useAsync(() => api.project(slug), [slug]);
  const brief = useAsync(() => api.brief(slug).catch(() => null), [slug]);
  const posts = useAsync(() => api.assets(slug, "post").catch(() => [] as Asset[]), [slug]);
  const { skipped, toggle } = useSkipped(slug);
  const [writing, setWriting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // The chat the guide started, so this step follows it (and survives a reload).
  const chatKey = `kiqr.guide.chat.${slug}`;
  const [chatId, setChatId] = useState<string | null>(() => {
    try {
      return localStorage.getItem(chatKey);
    } catch {
      return null;
    }
  });
  useEvents(
    slug,
    (e) => {
      if (e.type === "asset_saved" && e.asset_type === "post") posts.reload();
      if (e.type === "job_finished" && e.job === "posts") setWriting(false);
      if (e.type === "seemo_says" && typeof e.text === "string" && e.text.startsWith("I couldn't write the posts")) setError(e.text.replace(/^I couldn't write the posts: /, ""));
      if (chatId && e.chat === chatId) {
        if (e.type === "chat_turn_finished" && e.error) {
          setError(String(e.error));
          setWriting(false);
        }
        if (e.type === "chat_message") {
          const m = e.message as { role: string; card?: { status: string } };
          if (m.card && (m.card.status === "declined" || m.card.status === "expired")) setWriting(false);
        }
      }
      if (e.type === "task_run_finished" && e.status === "failed" && writing) {
        setError(String(e.summary || "the task failed"));
        setWriting(false);
      }
    },
    { demo: false },
  );

  /** Ask Seemo in a new chat; it makes a task, the task writes the posts. */
  const generate = async () => {
    setError(null);
    setWriting(true);
    try {
      const c = await api.createChat(slug, "Three social media posts to start with");
      setChatId(c.id);
      try {
        localStorage.setItem(chatKey, c.id);
      } catch {
        /* private window */
      }
      openSeemo(c.id, { expanded: true });
      await api.sendMessage(slug, c.id, "Write three social media posts for the library as draft post assets, with an on-brand image on most of them, following the instructions for social media posts and images, and tell me when they are in.");
      if (DEMO) {
        window.setTimeout(() => {
          posts.reload();
          setWriting(false);
        }, 4000);
      }
    } catch (e) {
      setError((e as Error).message);
      setWriting(false);
    }
  };

  const planView = useAsync(() => api.plan(slug).catch(() => null), [slug]);
  useEvents(slug, (e) => e.type === "plan_saved" && planView.reload(), { demo: false });
  // The build reaching a new stage changes the first two steps.
  useEvents(slug, (e) => e.type === "build_stage" && (project.reload(), brief.reload()), { demo: false });
  const hasPlan = !!planView.data?.plan;
  const p = project.data;
  const hasBrief = !!brief.data;
  // The build says where it is; without it (older daemons), a profile means a crawl happened.
  const stage = p?.build?.stage;
  const crawled = stage ? stage === "writing" || stage === "ready" || hasBrief : !!p?.last_crawl || hasBrief;
  const postCount = posts.data?.length ?? 0;
  const optional = (id: string, state: StepState): StepState => (state !== "done" && skipped.includes(id) ? "skipped" : state);
  const steps: Step[] = [
    {
      id: "crawl",
      icon: Radar,
      title: "Crawl your website",
      text: p?.last_crawl ? `${p.last_crawl.pages} pages read.` : crawled ? "Done. Rescan any time from Website crawls." : "Seemo reads your site to learn what you sell and how you sound.",
      state: crawled ? "done" : "now",
      to: `/p/${slug}/crawl`,
      action: "Open crawls",
    },
    { id: "profile", icon: Brain, title: "Check your business profile", text: "Fix anything Seemo got wrong. Everything it makes starts here.", state: hasBrief ? "done" : crawled ? "now" : "next", to: `/p/${slug}`, action: "Open profile" },
    {
      id: "plan",
      icon: Target,
      title: "Let Seemo propose a plan",
      text: hasPlan ? "The plan is in. Everything Seemo makes is tied to one of its objectives." : "A few objectives for the period, each with a number to reach. Seemo proposes it from the brief; you approve it in the chat.",
      state: optional("plan", hasPlan ? "done" : hasBrief ? "now" : "next"),
      optional: true,
      to: `/p/${slug}/plan`,
      action: hasPlan ? "See the plan" : "Open the plan page",
    },
    {
      id: "posts",
      icon: MessageSquareText,
      title: "Generate 3 social media posts",
      text: postCount
        ? `${postCount} post${postCount === 1 ? "" : "s"} in the library.`
        : writing
          ? "Seemo is on it in a chat: it proposes a task, you approve it, and the posts land here. Follow along in the corner or in Chats."
          : error
            ? `That didn't work: ${error}`
            : "Seemo writes three posts from your profile, in your tone of voice. You review them before anything goes out.",
      state: optional("posts", postCount ? "done" : hasBrief ? "now" : "next"),
      optional: true,
      to: postCount ? `/p/${slug}/library/post` : writing && chatId ? `/p/${slug}/chats/${chatId}` : undefined,
      action: postCount ? "See posts" : writing ? "Open the chat" : undefined,
      run: !postCount && hasBrief && !writing ? generate : undefined,
      busy: writing,
    },
    { id: "connect", icon: Cable, title: "Connect where you publish", text: "Your blog, X, Reddit, LinkedIn, Google Ads, and image and video tools.", state: optional("connect", "soon"), optional: true },
    {
      id: "schedule",
      icon: MessagesSquare,
      title: "Ask Seemo, or put it on the calendar",
      text: "“Every weekday at 10:00, write a LinkedIn post.” Seemo proposes a task, you confirm once; it runs on its own.",
      state: optional("schedule", hasBrief ? "now" : "next"),
      optional: true,
      to: `/p/${slug}/chats`,
      action: "Open chats",
    },
  ];
  const settled = steps.filter((s) => s.state === "done" || s.state === "skipped").length;
  const done = !project.loading && settled === steps.length;
  useEffect(() => onComplete?.(done), [done, onComplete]);
  if (hideWhenComplete && (project.loading || done)) return null;

  return (
    <div className="pcard lib-guide">
      <div className="lib-guide-head">
        <div>
          <p className="cap">Step by step</p>
          <h2>From your website to marketing that runs itself</h2>
        </div>
        <div className="lib-guide-prog" aria-label={`${settled} of ${steps.length} steps done or skipped`}>
          <span>
            {settled}/{steps.length}
          </span>
          <i>
            <motion.b initial={{ width: 0 }} animate={{ width: `${(settled / steps.length) * 100}%` }} transition={{ duration: 0.9, ease: [0.2, 0.8, 0.2, 1], delay: 0.2 }} />
          </i>
        </div>
      </div>
      <ol className="lib-steps">
        {steps.map((s, i) => (
          <motion.li key={s.id} className={`st-${s.state}`} initial={{ opacity: 0, x: -8 }} animate={{ opacity: 1, x: 0 }} transition={{ delay: 0.1 + i * 0.07, duration: 0.3 }}>
            <span className="mk">{s.state === "done" ? <Check size={14} /> : s.state === "skipped" ? <SkipForward size={13} /> : s.busy ? <Loader2 size={14} className="spin" /> : <s.icon size={14} />}</span>
            <div className="min0">
              <p className="nm">
                {s.title}
                {s.optional && s.state !== "done" && s.state !== "skipped" && <span className="opt">Optional</span>}
                {s.state === "soon" && <span className="soon">Coming soon</span>}
              </p>
              <p className="ds">{s.state === "skipped" ? "Skipped. Come back to it any time." : s.text}</p>
            </div>
            <div className="acts">
              {s.state === "skipped" ? (
                <button className="btn t" onClick={() => toggle(s.id)}>
                  <Undo2 size={14} /> Undo
                </button>
              ) : (
                <>
                  {s.optional && s.state !== "done" && !s.busy && (
                    <button className="btn t" onClick={() => toggle(s.id)}>
                      Skip
                    </button>
                  )}
                  {s.run && s.state === "now" ? (
                    <button className="btn p" disabled={s.busy} onClick={s.run}>
                      {s.busy ? "Writing…" : error ? "Try again" : "Generate posts"} {!s.busy && <Sparkles size={14} />}
                    </button>
                  ) : (
                    s.to &&
                    s.state !== "soon" && (
                      <Link className={`btn ${s.state === "now" ? "p" : "t"}`} to={s.to}>
                        {s.action} {s.state === "now" && <ArrowRight size={14} />}
                      </Link>
                    )
                  )}
                </>
              )}
            </div>
          </motion.li>
        ))}
      </ol>
    </div>
  );
}
