// The empty library: a showcase of what Seemo will make for this business,
// written from its own profile so it already feels like theirs.
import { motion } from "motion/react";
import {
  AudioLines, BarChart3, CalendarClock, Clapperboard, FileText, Film, Gift, Image, Laugh, LayoutTemplate, Mail,
  Megaphone, MessageSquareText, Mic, Music, Newspaper, PenLine, Radar, Reply, ShoppingBag, Sparkles, Target,
  type LucideIcon,
} from "lucide-react";
import { api, useAsync } from "../api";

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
