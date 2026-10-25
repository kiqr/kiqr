// "Teach Seemo something": a new ability in plain words, or an edit of one
// of yours. Name it as an action, say when it applies, write the steps.
import { useState } from "react";
import { Check, Globe, Loader2 } from "lucide-react";
import { api } from "../api";

export type TeachDraft = { name: string; description: string; text: string; everywhere: boolean; editing: string | null };

export const STARTERS: { label: string; text: string }[] = [
  { label: "How we write something", text: "When asked for a product update post:\n\n1. Lead with what changed for the customer, in one sentence.\n2. One concrete example or number.\n3. End with where to try it. No hype words.\n\nTone: plain, first person plural." },
  { label: "How to publish somewhere", text: "To publish an article to our blog:\n\n1. Make sure the asset is `ready` and has a cover image.\n2. Use the blog's publish tool with the title, the body as HTML and the category \"Guides\".\n3. Report the URL back and set the asset to published." },
  { label: "A recurring job", text: "The Monday report:\n\n1. List what was added to the library last week (type, title).\n2. Note which tasks ran and whether any failed.\n3. Suggest one thing to write next, from the content gaps.\n\nKeep it under 200 words." },
];

/** `Write product update posts` → `write-product-update-posts`. */
export function slugify(s: string) {
  return s
    .toLowerCase()
    .normalize("NFKD")
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 60);
}

/** Frontmatter in, fields out. */
export function parseInstructions(text: string): { name: string; description: string; body: string; front: string } {
  const m = /^---\n([\s\S]*?)\n---\n?([\s\S]*)$/.exec(text);
  if (!m) return { name: "", description: "", body: text.trim(), front: "" };
  const field = (k: string) => (new RegExp(`^${k}:\\s*(.*)$`, "m").exec(m[1])?.[1] ?? "").trim().replace(/^["']|["']$/g, "");
  return { name: field("name"), description: field("description"), body: m[2].trim(), front: m[1] };
}

function yamlStr(s: string) {
  const t = s.replace(/\n/g, " ").trim();
  return /[:#"'[\]{}&*!|>%@`]/.test(t) ? JSON.stringify(t) : t;
}

export function TeachForm({ slug, draft, setDraft, onDone }: { slug: string; draft: TeachDraft; setDraft: (d: TeachDraft) => void; onDone: () => void }) {
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const id = draft.editing ?? slugify(draft.name);
  const ok = id.length > 1 && draft.description.trim().length > 3 && draft.text.trim().length > 10;
  const save = async () => {
    setSaving(true);
    setError(null);
    try {
      const body = `---\nname: ${yamlStr(draft.name)}\ndescription: ${yamlStr(draft.description)}\n---\n\n${draft.text.trim()}\n`;
      await api.saveInstructions(draft.everywhere ? null : slug, id, body);
      onDone();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setSaving(false);
    }
  };
  return (
    <div className="pcard teach-form">
      <p className="cap">{draft.editing ? "Edit the ability" : "Teach Seemo something"}</p>
      <div className="grid">
        <label>
          <span className="nm">What should Seemo be able to do?</span>
          <input className="input" value={draft.name} placeholder="Write product update posts" onChange={(e) => setDraft({ ...draft, name: e.target.value })} />
          {id && <span className="ds mono">{id}</span>}
        </label>
        <label>
          <span className="nm">When should Seemo use it?</span>
          <input className="input" value={draft.description} placeholder="When asked for a post about something we shipped" onChange={(e) => setDraft({ ...draft, description: e.target.value })} />
          <span className="ds">One line. This is how Seemo knows it fits a request.</span>
        </label>
      </div>
      <label className="full">
        <span className="nm">Instructions</span>
        {!draft.text && (
          <div className="starters">
            {STARTERS.map((s) => (
              <button key={s.label} className="chip" onClick={() => setDraft({ ...draft, text: s.text })}>
                {s.label}
              </button>
            ))}
          </div>
        )}
        <textarea className="input" rows={10} value={draft.text} placeholder="Write it as you would to a new colleague: the steps, the tone, what to avoid, what to report." onChange={(e) => setDraft({ ...draft, text: e.target.value })} />
      </label>
      <div className="scope">
        <span className="nm">Where</span>
        <button className={`chip ${!draft.everywhere ? "acc" : ""}`} disabled={!!draft.editing} onClick={() => setDraft({ ...draft, everywhere: false })}>
          This project
        </button>
        <button className={`chip ${draft.everywhere ? "acc" : ""}`} disabled={!!draft.editing} onClick={() => setDraft({ ...draft, everywhere: true })}>
          <Globe size={12} /> Every project
        </button>
      </div>
      {error && <p className="err">{error}</p>}
      <div className="acts">
        <button className="btn p" disabled={!ok || saving} onClick={() => void save()}>
          {saving ? <Loader2 size={14} className="spin" /> : <Check size={14} />} {draft.editing ? "Save" : "Teach Seemo"}
        </button>
        <button className="btn t" onClick={onDone}>
          Cancel
        </button>
      </div>
    </div>
  );
}
