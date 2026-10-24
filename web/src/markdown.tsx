import type { ReactNode } from "react";

/** Enough Markdown for drafts and chat: headings, paragraphs, lists, quotes, bold, italics, links and code. */
export function Markdown({ text, className = "lib-md" }: { text: string; className?: string }) {
  const blocks = text.replace(/\r/g, "").split(/\n{2,}/);
  return (
    <div className={className}>
      {blocks.map((b, i) => {
        const h = /^(#{1,4})\s+(.*)$/.exec(b.trim());
        if (h) {
          const H = (["h1", "h2", "h3", "h4"] as const)[h[1].length - 1];
          return <H key={i}>{inline(h[2])}</H>;
        }
        const lines = b.split("\n").filter((l) => l.trim() !== "");
        if (lines.length && lines.every((l) => /^\s*([-*•]|\d+\.)\s+/.test(l))) {
          const L = /^\s*\d+\./.test(lines[0]) ? "ol" : "ul";
          return <L key={i}>{lines.map((l, k) => <li key={k}>{inline(l.replace(/^\s*([-*•]|\d+\.)\s+/, ""))}</li>)}</L>;
        }
        if (lines.length && lines.every((l) => l.startsWith(">"))) return <blockquote key={i}>{inline(lines.map((l) => l.replace(/^>\s?/, "")).join(" "))}</blockquote>;
        if (b.trim().startsWith("```")) return <pre key={i}>{b.trim().replace(/^```\w*\n?|```$/g, "")}</pre>;
        // Single newlines inside a paragraph stay line breaks: posts and lists Seemo writes depend on them.
        return (
          <p key={i}>
            {lines.map((l, k) => (
              <span key={k}>
                {k > 0 && <br />}
                {inline(l)}
              </span>
            ))}
          </p>
        );
      })}
    </div>
  );
}

export function inline(s: string): ReactNode[] {
  return s.split(/(\*\*[^*]+\*\*|`[^`]+`|\*[^*]+\*|\[[^\]]+\]\([^)]+\))/g).map((part, i) => {
    if (part.startsWith("**")) return <b key={i}>{part.slice(2, -2)}</b>;
    if (part.startsWith("`")) return <code key={i}>{part.slice(1, -1)}</code>;
    if (part.startsWith("[")) {
      const m = /^\[([^\]]+)\]\(([^)]+)\)$/.exec(part);
      if (m && /^(https?:|\/)/.test(m[2]))
        return (
          <a key={i} href={m[2]} target={m[2].startsWith("http") ? "_blank" : undefined} rel="noreferrer">
            {m[1]}
          </a>
        );
      return part;
    }
    if (part.startsWith("*") && part.length > 2) return <em key={i}>{part.slice(1, -1)}</em>;
    return part;
  });
}
