// The getting-started steps at the top of To do, built from what the owner told
// Seemo while it first read the site (the intake): the goal picks the first real
// piece of work, the networks the channels to connect, the time per week the
// rhythm to schedule. Each step that needs Seemo starts a chat in the corner
// widget, so the steps and the conversation stay one thread.
import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { motion } from "motion/react";
import { ArrowRight, BarChart3, CalendarClock, Cable, Check, Loader2, Mail, MessageSquare, MessageSquareText, Newspaper, Plug, Reply, SkipForward, Sparkles, Target, Undo2, type LucideIcon } from "lucide-react";
import { api, useAsync, useEvents, type Asset, type Intake } from "../api";
import { useNetworks } from "../networks";
import { openSeemo } from "../SeemoWidget";
import { ASK_FOR_PLAN } from "./Plan";

type StepState = "done" | "now" | "skipped";
type Step = {
  id: string;
  icon: LucideIcon;
  title: string;
  text: string;
  /** What the owner said that this step comes from, quoted back. */
  why?: string;
  state: StepState;
  optional?: boolean;
  to?: string;
  action?: string;
  /** Asks Seemo in a new chat: the chat's title and the message. */
  ask?: { title: string; message: string; label: string };
  /** Something to do here instead of a link. */
  run?: { label: string; go: () => void };
};

const GOAL_SAID: Record<string, string> = {
  customers: "More new customers",
  repeat: "Sell more to the customers I have",
  known: "Get known",
  launch: "Launch something new",
};
const GOAL_WORDS: Record<string, string> = {
  customers: "want more new customers",
  repeat: "want to sell more to the customers you have",
  known: "want to get known",
  launch: "are launching something new",
};
const TIME_SAID: Record<string, string> = { lt1: "Less than an hour a week", "1to3": "One to three hours a week", "3plus": "More than three hours a week" };
const TIME_WORDS: Record<string, string> = { lt1: "have less than an hour a week", "1to3": "have one to three hours a week", "3plus": "have more than three hours a week" };

/** What to put on the calendar, sized to the time the owner has. */
const RHYTHM: Record<string, { name: string; text: string; schedule: string }> = {
  lt1: { name: "one post a week", text: "Every Tuesday at 09:00, as a draft. Your part is a yes: a few minutes a week.", schedule: "one social media post every Tuesday at 09:00" },
  "1to3": { name: "three posts a week", text: "Monday, Wednesday and Friday at 09:00, as drafts for you to approve.", schedule: "one social media post every Monday, Wednesday and Friday at 09:00" },
  "3plus": { name: "a post every weekday", text: "Every weekday at 09:00, and an article every Thursday, as drafts for you to approve.", schedule: "one social media post every weekday at 09:00 and one blog article every Thursday at 09:00" },
  "": { name: "two posts a week", text: "Tuesday and Thursday at 09:00, as drafts for you to approve.", schedule: "one social media post every Tuesday and Thursday at 09:00" },
};

const list = (xs: string[]) => (xs.length < 2 ? (xs[0] ?? "") : `${xs.slice(0, -1).join(", ")} and ${xs[xs.length - 1]}`);

/** Skipped steps are a per-viewer convenience, kept in the browser. */
function useStored<T>(key: string, empty: T) {
  const read = (): T => {
    try {
      return (JSON.parse(localStorage.getItem(key) ?? "null") as T) ?? empty;
    } catch {
      return empty;
    }
  };
  const [value, set] = useState<T>(read);
  useEffect(() => set(read()), [key]); // eslint-disable-line react-hooks/exhaustive-deps
  const save = (f: (cur: T) => T) =>
    set((cur) => {
      const next = f(cur);
      try {
        localStorage.setItem(key, JSON.stringify(next));
      } catch {
        /* private window: lasts until reload */
      }
      return next;
    });
  return [value, save] as const;
}

/** "You told me you want more new customers, have less than an hour a week and are on Instagram." */
function toldMe(i: Intake | null, names: string[]) {
  if (!i) return null;
  const parts = [i.goal ? GOAL_WORDS[i.goal] : i.goal_note.trim() ? `want ${i.goal_note.trim().replace(/[.]$/, "")}` : "", i.time ? TIME_WORDS[i.time] : "", names.length ? `are on ${list(names)}` : ""].filter(Boolean);
  return parts.length ? `You told me you ${list(parts)}. These steps start from that.` : null;
}

export function Guide({ slug, hideWhenComplete, onComplete }: { slug: string; hideWhenComplete?: boolean; onComplete?: (done: boolean) => void }) {
  const brief = useAsync(() => api.brief(slug).catch(() => null), [slug]);
  const intake = useAsync(() => api.intake(slug).catch(() => null), [slug]);
  const assets = useAsync(() => api.assets(slug).catch(() => [] as Asset[]), [slug]);
  const planView = useAsync(() => api.plan(slug).catch(() => null), [slug]);
  const channels = useAsync(() => api.channels(slug).catch(() => null), [slug]);
  const tasks = useAsync(() => api.tasks(slug).catch(() => []), [slug]);
  const perf = useAsync(() => api.performance(slug).catch(() => null), [slug]);
  const setup = useAsync(() => api.setup(slug).catch(() => null), [slug]);
  const networks = useNetworks(slug);
  const [skipped, setSkipped] = useStored<string[]>(`kiqr.guide.skipped.${slug}`, []);
  // The chat each step started, so the step follows it (and survives a reload).
  const [chats, setChats] = useStored<Record<string, string>>(`kiqr.guide.chats.${slug}`, {});
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [writing, setWriting] = useState(false);

  useEvents(
    slug,
    (e) => {
      if (["asset_saved", "asset_deleted", "asset_approved", "asset_published", "asset_unpublished"].includes(e.type)) assets.reload();
      if (e.type === "job_finished" && e.job === "posts") setWriting(false);
      if (e.type === "plan_saved") planView.reload();
      if (e.type === "task_saved" || e.type === "task_deleted") tasks.reload();
      if (e.type === "connection_saved" || e.type === "connection_deleted") channels.reload();
      if (e.type === "metrics_snapshot") perf.reload();
      if (e.type === "intake_saved") (intake.reload(), setup.reload());
      if (["extension_installed", "extension_removed", "extension_paused", "connection_saved", "connect_finished"].includes(e.type)) setup.reload();
      if (e.type === "chat_turn_finished" && e.error) {
        const step = Object.keys(chats).find((k) => chats[k] === e.chat);
        if (step) setErrors((x) => ({ ...x, [step]: String(e.error) }));
      }
    },
    { demo: false },
  );

  const ask = async (id: string, q: NonNullable<Step["ask"]>) => {
    setErrors((x) => ({ ...x, [id]: "" }));
    try {
      const c = await api.createChat(slug, q.title);
      setChats((cur) => ({ ...cur, [id]: c.id }));
      openSeemo(c.id);
      await api.sendMessage(slug, c.id, q.message);
    } catch (e) {
      setErrors((x) => ({ ...x, [id]: (e as Error).message }));
    }
  };
  const writePosts = () => {
    setErrors((x) => ({ ...x, posts: "" }));
    setWriting(true);
    api.generateAssets(slug, "post", 3).catch((e: Error) => {
      setErrors((x) => ({ ...x, posts: e.message }));
      setWriting(false);
    });
  };

  const b = brief.data;
  const i = intake.data;
  const all = assets.data ?? [];
  const has = (type: string) => all.some((a) => a.type === type && a.status !== "archived");
  const posts = all.filter((a) => a.type === "post");
  const drafts = posts.filter((a) => a.status === "draft").length;
  const approved = posts.some((a) => a.status === "ready" || a.status === "scheduled" || a.status === "published");
  const plan = planView.data?.plan;
  const objective = plan?.objectives.find((o) => o.status === "active");
  const kpi = objective && (planView.data?.kpis.find((k) => k.id === objective.kpi)?.label ?? objective.kpi);
  const name = b?.business_name || "your business";
  const gap = b?.content_gaps[0]?.topic;
  const who = i?.customers.trim() || b?.audiences[0]?.name || "your customers";
  // The networks named that something here can publish to; the rest wait for an ability (the welcome chat said so).
  const said = (i?.networks ?? []).flatMap((k) => networks.filter((n) => n.kind === k || n.name.toLowerCase() === k.toLowerCase()).map((n) => n.name).slice(0, 1));
  const sugs = setup.data?.suggestions ?? [];
  const toSetUp = sugs.filter((s) => !s.ready);
  const ch = channels.data?.channels ?? [];
  const connected = i?.networks.length ? ch.some((c) => i.networks.includes(c.kind)) : ch.length > 0;
  const measured = (perf.data?.sources ?? []).some((s) => s.provider && s.id !== "kiqr");
  const rhythm = RHYTHM[i?.time ?? ""] ?? RHYTHM[""];
  const goalSaid = i?.goal ? GOAL_SAID[i.goal] : i?.goal_note.trim() || undefined;

  // The first real piece of work, from the goal.
  const goal: Pick<Step, "icon" | "title" | "text" | "ask"> & { type: string } =
    i?.goal === "repeat"
      ? {
          type: "email",
          icon: Mail,
          title: "Write a letter to the customers you have",
          text: `One email that brings them back: what's new at ${name}, and what they could buy next.`,
          ask: { title: "A letter to your customers", label: "Write it", message: `Write an email to ${name}'s existing customers as a draft email asset in the library: what's new, and what they could buy next${b?.offerings[0] ? `, like ${b.offerings[0].name}` : ""}. Tell me when it's in.` },
        }
      : i?.goal === "known"
        ? {
            type: "reply",
            icon: Reply,
            title: "Find conversations to join",
            text: `Seemo looks for threads where ${who.toLowerCase()} ask about what you do, and drafts helpful replies. Nothing is posted until you say yes.`,
            ask: { title: "Conversations to join", label: "Find them", message: `Find three recent online conversations where ${who} ask about what ${name} does, and draft a helpful reply to each as a draft reply asset in the library. Tell me when they're in.` },
          }
        : i?.goal === "launch"
          ? {
              type: "article",
              icon: Newspaper,
              title: "Draft the launch announcement",
              text: "An article that says what's new and why it matters, ready to share everywhere on the day.",
              ask: { title: "The launch announcement", label: "Write it", message: `Write a launch announcement for ${name} as a draft article asset in the library${i.goal_note.trim() ? `; the launch: ${i.goal_note.trim()}` : ""}. Ask me what's launching if the profile doesn't say. Tell me when it's in.` },
            }
          : {
              type: "article",
              icon: Newspaper,
              title: gap ? `Answer “${gap}”` : "Write your first article",
              text: gap ? "New customers often start with a search. Your site doesn't answer this yet; Seemo writes the article that does, in your voice." : "An article that answers what your customers search for, in your voice.",
              ask: { title: "Your first article", label: "Write it", message: `Write a blog article for ${who} as a draft article asset in the library${gap ? ` on “${gap}”, from the content gaps in the profile` : ", on the biggest content gap in the profile"}. Tell me when it's in.` },
            };

  const st = (id: string, done: boolean): StepState => (done ? "done" : skipped.includes(id) ? "skipped" : "now");
  const steps: Step[] = [
    {
      id: "plan",
      icon: Target,
      title: plan ? "Your marketing plan" : "Approve your marketing plan",
      text: objective
        ? `${plan!.objectives.filter((o) => o.status === "active").length} objectives, starting with “${objective.title}”.`
        : "Two to four objectives, each with a number to reach, from your profile and what you told me. You approve it in the chat.",
      state: st("plan", !!plan),
      to: plan ? `/p/${slug}/plan` : undefined,
      action: "See the plan",
      ask: { title: "Your first marketing plan", label: "Propose it", message: `${ASK_FOR_PLAN} This is our first plan together: keep it to what fits the time I have.` },
    },
    {
      id: "posts",
      icon: MessageSquareText,
      title: approved ? "Your first posts" : "Approve your first posts",
      text: approved
        ? "Approved. They go out on the channel and at the time you picked."
        : drafts
          ? `${drafts} draft${drafts === 1 ? "" : "s"} wait below. Approve one for a channel, or ask Seemo to change it.`
          : writing
            ? "Seemo is writing three posts in your brand. They land below as drafts."
            : "Three posts from your profile, in your voice and your colours. Nothing goes out until you approve it.",
      state: st("posts", approved),
      optional: true,
      to: approved ? `/p/${slug}/library/post` : undefined,
      action: "See posts",
      run: approved ? undefined : drafts ? { label: "See the drafts", go: () => document.getElementById("todo-drafts")?.scrollIntoView({ behavior: "smooth", block: "start" }) } : { label: writing ? "Writing…" : "Write them", go: writePosts },
    },
    ...(sugs.length
      ? [
          {
            id: "setup",
            icon: Plug,
            title: toSetUp.length ? `${toSetUp.some((s) => s.installed) ? "Sign in to" : "Set up"} ${list(toSetUp.map((s) => s.name.replace(/^(Read|Publish \w+ to|Send \w+ from) /, "")))}` : "Your tools are set up",
            text: toSetUp.length
              ? "I suggested these from what you told me and what I saw on your site. Sign in once and they work on their own."
              : "Everything I suggested in our first conversation is signed in and on.",
            why: toSetUp[0]?.why[0]?.kind === "site" ? "Seen on your site" : undefined,
            state: st("setup", toSetUp.length === 0),
            optional: true,
            to: `/p/${slug}/abilities${toSetUp[0] ? `#ability=${toSetUp[0].id}` : ""}`,
            action: toSetUp.length ? "Set up" : "See abilities",
          } satisfies Step,
        ]
      : []),
    {
      id: "connect",
      icon: Cable,
      title: said.length ? `Connect ${list(said)}` : "Connect where you publish",
      text: connected ? "Connected. Approved posts go out there on their own." : said.length ? "You're already there. Once connected, an approved post goes out on its own at the time you pick." : "Your blog, social accounts and ad accounts. Once connected, an approved post goes out on its own.",
      why: said.length ? `You said: ${list(said)}` : undefined,
      state: st("connect", connected),
      optional: true,
      to: `/p/${slug}/channels`,
      action: connected ? "See channels" : "Connect",
    },
    {
      id: "goal",
      ...goal,
      state: st("goal", has(goal.type)),
      why: goalSaid ? `You said: ${goalSaid}` : undefined,
      optional: true,
      to: has(goal.type) ? `/p/${slug}/library/${goal.type}` : undefined,
      action: "See it",
      ask: has(goal.type) ? undefined : goal.ask,
    },
    {
      id: "numbers",
      icon: BarChart3,
      title: "Connect your numbers",
      text: measured ? "Connected. Seemo reads them once a day and tells you how the plan is going." : objective ? `Your plan counts ${kpi!.toLowerCase()}. Connect where that number lives, and Seemo tells you how it's going.` : "Connect your analytics, and Seemo tells you what works.",
      state: st("numbers", measured),
      optional: true,
      to: `/p/${slug}/performance`,
      action: measured ? "See performance" : "Connect",
    },
    {
      id: "rhythm",
      icon: CalendarClock,
      title: `Put ${rhythm.name} on the calendar`,
      text: rhythm.text,
      why: i?.time ? `You said: ${TIME_SAID[i.time]}` : undefined,
      state: st("rhythm", (tasks.data ?? []).some((t) => t.status === "active")),
      optional: true,
      to: `/p/${slug}/tasks`,
      action: "See tasks",
      ask: {
        title: "Your weekly rhythm",
        label: "Set it up",
        message: `Propose a recurring task: ${rhythm.schedule}, written as drafts in the library for me to approve${said.length ? `, for ${list(said)}` : ""}, tied to the plan.`,
      },
    },
  ];

  const loading = brief.loading || intake.loading || assets.loading || planView.loading;
  const settled = steps.filter((s) => s.state !== "now").length;
  const done = !loading && settled === steps.length;
  useEffect(() => onComplete?.(done), [done, onComplete]);
  if (hideWhenComplete && (loading || done)) return null;
  const first = steps.find((s) => s.state === "now")?.id;
  const told = toldMe(i ?? null, said);

  return (
    <div className="pcard lib-guide">
      <div className="lib-guide-head">
        <div>
          <p className="cap">Step by step</p>
          <h2>Your first week with Seemo</h2>
          <p className="lib-guide-told">
            <span className="av" aria-hidden />
            {told ?? "From your website to marketing that runs itself, one step at a time."}
          </p>
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
        {steps.map((s, n) => {
          const chat = chats[s.id];
          const asked = s.state === "now" && s.ask && chat;
          const busy = (s.id === "posts" && writing) || false;
          const err = errors[s.id];
          const primary = s.id === first ? "p" : "g";
          return (
            <motion.li key={s.id} className={`st-${s.state} ${s.id === first ? "first" : ""}`} initial={{ opacity: 0, x: -8 }} animate={{ opacity: 1, x: 0 }} transition={{ delay: 0.1 + n * 0.07, duration: 0.3 }}>
              <span className="mk">{s.state === "done" ? <Check size={14} /> : s.state === "skipped" ? <SkipForward size={13} /> : busy ? <Loader2 size={14} className="spin" /> : <s.icon size={14} />}</span>
              <div className="min0">
                <p className="nm">
                  {s.title}
                </p>
                <p className="ds">{s.state === "skipped" ? "Skipped. Come back to it any time." : err ? `That didn't work: ${err}` : asked ? "Seemo is on it in a chat. It asks you before it makes anything." : s.text}</p>
                {s.why && s.state === "now" && <p className="why">{s.why}</p>}
              </div>
              <div className="acts">
                {s.state === "skipped" ? (
                  <button className="btn t" onClick={() => setSkipped((cur) => cur.filter((x) => x !== s.id))}>
                    <Undo2 size={14} /> Undo
                  </button>
                ) : s.state === "done" ? (
                  s.to && (
                    <Link className="btn t" to={s.to}>
                      {s.action}
                    </Link>
                  )
                ) : (
                  <>
                    {s.optional && !busy && (
                      <button className="btn t" onClick={() => setSkipped((cur) => [...cur, s.id])}>
                        Skip
                      </button>
                    )}
                    {asked ? (
                      <button className={`btn ${err ? primary : "g"}`} onClick={() => (err ? void ask(s.id, s.ask!) : openSeemo(chat))}>
                        {err ? "Try again" : <><MessageSquare size={14} /> Open the chat</>}
                      </button>
                    ) : s.ask ? (
                      <button className={`btn ${primary}`} onClick={() => void ask(s.id, s.ask!)}>
                        {s.ask.label} <Sparkles size={14} />
                      </button>
                    ) : s.run ? (
                      <button className={`btn ${primary}`} disabled={busy} onClick={s.run.go}>
                        {err && !busy ? "Try again" : s.run.label} {!busy && <ArrowRight size={14} />}
                      </button>
                    ) : (
                      s.to && (
                        <Link className={`btn ${primary}`} to={s.to}>
                          {s.action} <ArrowRight size={14} />
                        </Link>
                      )
                    )}
                  </>
                )}
              </div>
            </motion.li>
          );
        })}
      </ol>
    </div>
  );
}
