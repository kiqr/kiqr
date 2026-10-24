// Skills: how Seemo does things. KIQR's own, the ones you write here in plain
// words, and packs you install (a skill folder that also brings a connection).
import { useEffect, useState } from "react";
import { Link, useParams } from "react-router-dom";
import { BookOpen, Check, ChevronDown, ChevronRight, Copy, FolderOpen, Globe, Loader2, Pencil, Plus, Trash2, X } from "lucide-react";
import { api, useAsync, useEvents, type Skill } from "../api";
import { Markdown } from "../markdown";
import "../profile.css";
import "../chat.css";

type Draft = { name: string; description: string; text: string; everywhere: boolean; editing: string | null };

const STARTERS: { label: string; text: string }[] = [
  { label: "How we write something", text: "When asked for a product update post:\n\n1. Lead with what changed for the customer, in one sentence.\n2. One concrete example or number.\n3. End with where to try it. No hype words.\n\nTone: plain, first person plural." },
  { label: "How to publish somewhere", text: "To publish an article to our blog:\n\n1. Make sure the asset is `ready` and has a cover image.\n2. Use the blog connection's publish tool with the title, the body as HTML and the category \"Guides\".\n3. Report the URL back and set the asset to published." },
  { label: "A recurring job", text: "The Monday report:\n\n1. List what was added to the library last week (type, title).\n2. Note which tasks ran and whether any failed.\n3. Suggest one thing to write next, from the content gaps.\n\nKeep it under 200 words." },
];

/** `Publish to WordPress` → `publish-to-wordpress`. */
function slugify(s: string) {
  return s
    .toLowerCase()
    .normalize("NFKD")
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 60);
}

/** Frontmatter in, fields out. */
function parseSkill(text: string): { description: string; body: string } {
  const m = /^---\n([\s\S]*?)\n---\n?([\s\S]*)$/.exec(text);
  if (!m) return { description: "", body: text.trim() };
  const d = /^description:\s*(.*)$/m.exec(m[1]);
  return { description: (d?.[1] ?? "").trim().replace(/^["']|["']$/g, ""), body: m[2].trim() };
}

export function SkillsPage() {
  const { slug = "" } = useParams();
  const skills = useAsync(() => api.skills(slug), [slug]);
  const [draft, setDraft] = useState<Draft | null>(null);
  const [advanced, setAdvanced] = useState(false);
  useEvents(null, (e) => ["skill_saved", "skill_deleted", "extension_installed", "extension_removed", "secret_set"].includes(e.type) && skills.reload(), { demo: false });
  const list = skills.data ?? [];
  // Skills proper: folders with a SKILL.md Seemo reads in a chat. Pipeline prompts and
  // connection-only folders live under Advanced.
  const yours = list.filter((s) => s.has_skill && !s.builtin);
  const builtin = list.filter((s) => s.has_skill && s.builtin && !s.pipeline);

  const startNew = () => setDraft({ name: "", description: "", text: "", everywhere: false, editing: null });
  const edit = async (s: Skill) => {
    const text = await api.skillText(slug, s.name).catch(() => "");
    const parsed = parseSkill(text);
    setDraft({ name: s.name, description: parsed.description || s.description, text: parsed.body, everywhere: s.scope.scope === "global" && !s.builtin, editing: s.builtin ? null : s.name });
  };

  return (
    <div className="prof">
      <div className="cm-head">
        <div>
          <p className="cap">Seemo</p>
          <h1 className="d2">Skills</h1>
          <p className="muted">How Seemo does things, in plain words. It reads the skill that fits before it acts. Write your own here, or install a pack that also brings a connection.</p>
        </div>
        {!draft && (
          <button className="btn p" onClick={startNew}>
            <Plus size={15} /> Teach Seemo something
          </button>
        )}
      </div>

      {draft && <SkillForm slug={slug} draft={draft} setDraft={setDraft} onDone={() => (setDraft(null), skills.reload())} />}

      {skills.data && yours.length === 0 && !draft && (
        <div className="pcard none">
          <BookOpen size={22} className="ac2" />
          <p className="t">Nothing of your own yet</p>
          <p className="muted">Tell Seemo how you write a product update, how to publish to your blog, or what a weekly report should contain. It will follow it every time.</p>
          <button className="btn g" onClick={startNew}>
            <Plus size={14} /> Teach Seemo something
          </button>
        </div>
      )}

      {yours.length > 0 && (
        <>
          <p className="cap" style={{ margin: "4px 0 10px" }}>
            Yours
          </p>
          <div className="skills">
            {yours.map((s) => (
              <SkillRow key={`${s.scope.scope}-${s.name}`} slug={slug} s={s} onEdit={() => void edit(s)} onChange={skills.reload} />
            ))}
          </div>
        </>
      )}

      {builtin.length > 0 && (
        <>
          <p className="cap" style={{ margin: "24px 0 10px" }}>
            Built in
          </p>
          <div className="skills">
            {builtin.map((s) => (
              <SkillRow key={s.name} slug={slug} s={s} onEdit={() => void edit(s)} onChange={skills.reload} />
            ))}
          </div>
        </>
      )}

      <div className="pcard adv">
        <button className="adv-head" onClick={() => setAdvanced((a) => !a)} aria-expanded={advanced}>
          {advanced ? <ChevronDown size={14} /> : <ChevronRight size={14} />} Advanced: folders, packs and connections
        </button>
        {advanced && <Advanced slug={slug} skills={list} onChange={skills.reload} />}
      </div>
    </div>
  );
}

function SkillRow({ slug, s, onEdit, onChange }: { slug: string; s: Skill; onEdit: () => void; onChange: () => void }) {
  const [open, setOpen] = useState(false);
  const [text, setText] = useState<string | null>(null);
  const [confirm, setConfirm] = useState(false);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    if (open && text === null) api.skillText(slug, s.name).then((t) => setText(parseSkill(t).body), () => setText(""));
  }, [open, text, slug, s.name]);
  const remove = async () => {
    setBusy(true);
    try {
      await api.deleteSkill(s.scope.scope === "project" ? slug : null, s.name);
      onChange();
    } finally {
      setBusy(false);
      setConfirm(false);
    }
  };
  const project = s.scope.scope === "project";
  return (
    <div className={`pcard skill ${s.error ? "broken" : ""}`}>
      <div className="row">
        <span className="ic">
          <BookOpen size={16} />
        </span>
        <div className="min0">
          <h3>
            {s.title || s.name}
            {project ? <span className="chip acc">This project</span> : <span className="chip">Everywhere</span>}
            {s.connection && (
              <Link to="/settings/connections" className={`chip ${s.connection.ready ? "ok" : "warn"}`} title={s.connection.name}>
                <b /> {s.connection.ready ? s.connection.name : `${s.connection.name}: needs a key`}
              </Link>
            )}
          </h3>
          <p className="meta">
            {s.description ? <span>{s.description}</span> : <span className="dim">No description: Seemo will not know when to use it.</span>}
          </p>
          {s.error && <p className="err">{s.error}</p>}
        </div>
        <div className="acts">
          {s.has_skill && (
            <button className="btn t" onClick={() => setOpen((o) => !o)} aria-label={open ? "Hide" : "Read"} title={open ? "Hide" : "Read"}>
              {open ? <ChevronDown size={15} /> : <ChevronRight size={15} />}
            </button>
          )}
          {s.has_skill && (
            <button className="btn t" onClick={onEdit} aria-label={s.builtin ? "Edit a copy" : "Edit"} title={s.builtin ? "Edit a copy for this project" : "Edit"}>
              {s.builtin ? <Copy size={15} /> : <Pencil size={15} />}
            </button>
          )}
          {!s.builtin &&
            (confirm ? (
              <>
                <button className="btn t bad" disabled={busy} onClick={() => void remove()} aria-label="Yes, remove">
                  {busy ? <Loader2 size={15} className="spin" /> : <Check size={15} />}
                </button>
                <button className="btn t" onClick={() => setConfirm(false)} aria-label="Keep it">
                  <X size={15} />
                </button>
              </>
            ) : (
              <button className="btn t" onClick={() => setConfirm(true)} aria-label="Remove" title="Remove">
                <Trash2 size={15} />
              </button>
            ))}
        </div>
      </div>
      {open && <div className="skill-text">{text === null ? <Loader2 size={14} className="spin" /> : <Markdown text={text} />}</div>}
    </div>
  );
}

function SkillForm({ slug, draft, setDraft, onDone }: { slug: string; draft: Draft; setDraft: (d: Draft) => void; onDone: () => void }) {
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const name = slugify(draft.name);
  const ok = name.length > 1 && draft.description.trim().length > 3 && draft.text.trim().length > 10;
  const save = async () => {
    setSaving(true);
    setError(null);
    try {
      const body = `---\nname: ${name}\ndescription: ${draft.description.trim().replace(/\n/g, " ")}\n---\n\n${draft.text.trim()}\n`;
      await api.saveSkill(draft.everywhere ? null : slug, name, body);
      onDone();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setSaving(false);
    }
  };
  return (
    <div className="pcard skill-form">
      <p className="cap">{draft.editing ? "Edit the skill" : "Teach Seemo something"}</p>
      <div className="grid">
        <label>
          <span className="nm">What is it called?</span>
          <input className="input" value={draft.name} placeholder="Product update posts" onChange={(e) => setDraft({ ...draft, name: e.target.value })} disabled={!!draft.editing} />
          {name && <span className="ds mono">{name}</span>}
        </label>
        <label>
          <span className="nm">When should Seemo use it?</span>
          <input className="input" value={draft.description} placeholder="When asked for a post about something we shipped" onChange={(e) => setDraft({ ...draft, description: e.target.value })} />
          <span className="ds">One line. This is how Seemo knows the skill fits a request.</span>
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
        <button className={`chip ${!draft.everywhere ? "acc" : ""}`} onClick={() => setDraft({ ...draft, everywhere: false })}>
          This project
        </button>
        <button className={`chip ${draft.everywhere ? "acc" : ""}`} onClick={() => setDraft({ ...draft, everywhere: true })}>
          <Globe size={12} /> Every project
        </button>
      </div>
      {error && <p className="err">{error}</p>}
      <div className="acts">
        <button className="btn p" disabled={!ok || saving} onClick={() => void save()}>
          {saving ? <Loader2 size={14} className="spin" /> : <Check size={14} />} Save the skill
        </button>
        <button className="btn t" onClick={onDone}>
          Cancel
        </button>
      </div>
    </div>
  );
}

/** Folders, packs from a path or URL, and what each pack brings. */
function Advanced({ slug, skills, onChange }: { slug: string; skills: Skill[]; onChange: () => void }) {
  const [source, setSource] = useState("");
  const [here, setHere] = useState(true);
  const [busy, setBusy] = useState(false);
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);
  const install = async () => {
    if (!source.trim()) return;
    setBusy(true);
    setMsg(null);
    try {
      const s = await api.installSkill(source.trim(), here ? slug : undefined);
      setMsg({ ok: true, text: `Installed ${s.name}${s.connection ? ` with the connection ${s.connection.name}` : ""}.` });
      setSource("");
      onChange();
    } catch (e) {
      setMsg({ ok: false, text: (e as Error).message });
    } finally {
      setBusy(false);
    }
  };
  const global = skills.find((s) => s.scope.scope === "global");
  const globalDir = global ? global.folder.replace(/\/[^/]+$/, "") : "~/.kiqr/skills";
  return (
    <div className="adv-body">
      <p className="muted sm">
        A skill is a folder with a <code>SKILL.md</code>. It may also hold an <code>extension.yaml</code>, which gives Seemo a connection: an HTTP call, a program on this computer, or an MCP server. Packs install the same way.
      </p>
      <dl className="set-rows mono">
        <div>
          <dt>Every project</dt>
          <dd>
            <code>
              <FolderOpen size={12} /> {globalDir}
            </code>
          </dd>
        </div>
        <div>
          <dt>This project</dt>
          <dd>
            <code>
              <FolderOpen size={12} /> {globalDir.replace(/\/skills$/, "")}/projects/{slug}/skills
            </code>
          </dd>
        </div>
      </dl>
      <div className="install">
        <input className="input mono" value={source} placeholder="/path/to/folder or https://github.com/you/pack.git" onChange={(e) => setSource(e.target.value)} />
        <button className={`chip ${here ? "acc" : ""}`} onClick={() => setHere(true)}>
          This project
        </button>
        <button className={`chip ${!here ? "acc" : ""}`} onClick={() => setHere(false)}>
          Every project
        </button>
        <button className="btn g" disabled={busy || !source.trim()} onClick={() => void install()}>
          {busy ? <Loader2 size={14} className="spin" /> : <Plus size={14} />} Install
        </button>
      </div>
      {msg && <p className={msg.ok ? "ok sm" : "err sm"}>{msg.text}</p>}
      <ul className="packs">
        {skills
          .filter((s) => s.connection || s.error || s.pipeline)
          .map((s) => (
            <li key={`${s.scope.scope}-${s.name}`}>
              <code>{s.name}</code>
              {s.connection ? (
                <span>
                  brings <b>{s.connection.name}</b> {s.connection.ready ? "(connected)" : "(needs a key)"}
                  {s.has_skill ? " and a skill" : ""}
                </span>
              ) : s.error ? (
                <span className="bad">{s.error}</span>
              ) : (
                <span>one of KIQR's pipeline prompts (reading pages, building the profile); edit with care</span>
              )}
            </li>
          ))}
      </ul>
      <p className="muted sm">
        Keys live in <code>.env</code> and are set under <Link to="/settings/connections">Settings → Connections</Link>. They never reach the model.
      </p>
    </div>
  );
}
