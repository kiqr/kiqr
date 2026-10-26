// Small pieces shared by the Abilities page, its modal and the marketplace:
// the icon, the vendor, the state, what an ability includes.
import { useState } from "react";
import { BarChart3, BookOpen, Zap, Brain, Globe, Image as ImageIcon, KeyRound, ListChecks, Mail, MessageSquareText, Music, PenTool, Plug, Send, ShieldCheck, Sparkles, Terminal, Video, Wrench, type LucideIcon } from "lucide-react";
import type { Ability } from "../api";
import "../signin.css";
import { assetIcon } from "../assetIcons";

const ABILITY_ICON: Record<string, LucideIcon> = { image: ImageIcon, video: Video, music: Music, email: Mail, publish: Send, web: Globe };
const NAMED_ICON: Record<string, LucideIcon> = { chart: BarChart3, search: Globe, globe: Globe, "shield-check": ShieldCheck, brain: Brain, "list-checks": ListChecks, "pen-tool": PenTool, "message-square-text": MessageSquareText, image: ImageIcon, mail: Mail, "book-open": BookOpen };

export function iconFor(a: Ability): LucideIcon {
  if (a.icon && NAMED_ICON[a.icon]) return NAMED_ICON[a.icon];
  if (a.icon) return assetIcon(a.icon);
  const first = a.provides[0]?.ability;
  if (first && ABILITY_ICON[first]) return ABILITY_ICON[first];
  if (a.channels.length) return Send;
  if (a.runs_program) return Terminal;
  if (a.transport === "instructions") return BookOpen;
  return Sparkles;
}

/** A channel kind's icon, by the lucide name its ability gave it. */
export function kindIcon(icon?: string): LucideIcon {
  const named = icon ? NAMED_ICON[icon] ?? assetIcon(icon) : undefined;
  return named && named !== assetIcon(undefined) ? named : Send;
}

/** The ability's mark: its avatar when it has one, else its icon. */
export function AbilityIcon({ a, size }: { a: Ability; size: number }) {
  const [broken, setBroken] = useState(false);
  const Icon = iconFor(a);
  if (a.avatar && !broken) return <img className="ab-avatar" src={a.avatar} alt="" width={size} height={size} onError={() => setBroken(true)} />;
  return <Icon size={size} />;
}

/** The vendor, linked to its GitHub page when it has one. */
export function Author({ a }: { a: Ability }) {
  // The product name is always KIQR, whatever an index file wrote.
  const label = a.author === "you" ? "you" : a.author.toLowerCase() === "kiqr" ? "KIQR" : a.author;
  return a.author_url ? (
    <a className="mk-author" href={a.author_url} target="_blank" rel="noreferrer" onClick={(e) => e.stopPropagation()}>
      {label}
    </a>
  ) : (
    <span className="mk-author">{label}</span>
  );
}

export type State = { id: "on" | "own" | "machine" | "off-here" | "off-all" | "missing" | "noprogram" | "setup" | "available"; text: string; tone: "ok" | "warn" | "" };

/** One phrase for where an ability stands in this project. */
export function stateOf(a: Ability): State {
  if (!a.installed) return { id: "available", text: "not installed", tone: "" };
  if (a.machine_off) return { id: "off-all", text: "off everywhere", tone: "" };
  if (a.paused) return { id: "off-here", text: "off here", tone: "" };
  if (a.program_found === false) return { id: "noprogram", text: "program missing", tone: "warn" };
  if (!a.ready) return { id: a.missing.length ? "missing" : "setup", text: a.missing.length ? (a.kinds?.length ? "no channel yet" : "not connected") : "needs setup", tone: "warn" };
  const n = a.accounts?.length ?? 0;
  if (n > 0) return { id: "own", text: a.kinds?.length ? (n === 1 ? "1 channel" : `${n} channels`) : n === 1 ? "connected" : `${n} accounts`, tone: "ok" };
  if (a.signed_in) return { id: "own", text: a.signed_in === "project" ? "signed in here" : "signed in", tone: "ok" };
  if (a.auth.length) return { id: "machine", text: "machine key", tone: "ok" };
  return { id: "on", text: "on", tone: "ok" };
}

export function isOn(a: Ability) {
  return a.installed && !a.machine_off && !a.paused && a.ready;
}

export type Part = { icon: LucideIcon; label: string; detail?: string; tone?: "warn" | "" };

/** What an ability includes: what Seemo gets from it. Its requirements are `needsOf`. */
export function partsOf(a: Ability, full = false): Part[] {
  const out: Part[] = [];
  if (a.instructions.length) out.push({ icon: BookOpen, label: a.instructions.length === 1 ? "Instructions" : `Instructions, ${a.instructions.length} files`, detail: full ? a.instructions.map((f) => f.label).join(" · ") : undefined });
  if (a.transport === "mcp") out.push({ icon: Plug, label: "An MCP server", detail: full ? (a.command ?? "") : undefined });
  else if (a.runs_program) out.push({ icon: Terminal, label: "A program", detail: full ? (a.command ?? "") : undefined });
  if (a.tools > 0) out.push({ icon: Wrench, label: `${a.tools} ${a.tools === 1 ? "tool" : "tools"} for Seemo` });
  if (a.channels.length) out.push({ icon: Send, label: `Publishes to ${a.channels.join(", ")}` });
  if (a.sources?.length) out.push({ icon: BarChart3, label: `Reads ${a.sources.join(", ")} into Performance` });
  if (a.provides.length) out.push({ icon: Sparkles, label: a.provides.map((p) => p.label).join(", "), detail: full && a.active_for.length ? `preferred for ${a.active_for.join(", ")}` : undefined });
  if (a.core && !out.length) out.push({ icon: Sparkles, label: "Part of KIQR" });
  return out;
}

/** What an ability needs before it works: keys and where each is set, a program on this computer, the hosts it calls. */
export function needsOf(a: Ability): Part[] {
  const out: Part[] = [];
  if (a.auth.length && a.core) {
    // Signed in to the service (`signed_in`), its key is not needed: the sign-in stands in for it.
    const signedIn = a.signed_in ? `not needed: signed in for ${a.signed_in === "project" ? "this project" : "every project"}` : null;
    for (const k of a.auth) {
      out.push({ icon: KeyRound, label: `${k.label || k.env}${k.optional ? " (optional)" : ""}`, detail: k.set ? `set for ${k.set === "project" ? "this project" : "the machine"}` : signedIn ?? "not set", tone: !k.set && !k.optional && !signedIn ? "warn" : "" });
    }
  } else if (a.auth.length) {
    // Each account (each channel, for one that publishes) has its own keys.
    const n = a.accounts?.length ?? 0;
    const ready = (a.accounts ?? []).filter((x) => x.missing.length === 0).length;
    const noun = a.kinds?.length ? "channel" : "account";
    out.push({
      icon: KeyRound,
      label: `${a.auth.map((k) => k.label || k.env).join(", ")}, per ${noun}`,
      detail: !a.installed ? undefined : n === 0 ? `no ${noun} in this project yet` : `${ready} of ${n} ${noun}${n === 1 ? "" : "s"} connected`,
      tone: a.installed && ready === 0 ? "warn" : "",
    });
  }
  if (a.transport === "mcp" || a.runs_program) out.push({ icon: Terminal, label: `${a.command ?? "a program"} on this computer`, detail: a.program_found === false ? "not found; install it first" : a.program_found ? "found" : undefined, tone: a.program_found === false ? "warn" : "" });
  if (a.hosts.length) out.push({ icon: Globe, label: a.hosts.length === 1 ? "Access to one host" : `Access to ${a.hosts.length} hosts`, detail: a.hosts.join(", ") });
  return out;
}

export const RELOAD_ON = ["secret_set", "connect_finished", "sign_in_changed", "connection_saved", "connection_deleted", "extension_installed", "extension_removed", "extension_paused", "ability_saved", "config_changed", "marketplace_refreshed"];

/** Connects with a button instead of keys: its manifest has a sign-in, or KIQR signs in to it. */
export function oneClick(a: Ability): boolean {
  return !!a.connect || ONE_CLICK_CORE.includes(a.id);
}
const ONE_CLICK_CORE = ["unblockingapi"];

/** The label on abilities that connect with one click. */
export function OneClick() {
  return (
    <span className="oneclick" title="Connects with a button: sign in and approve, no keys to copy">
      <Zap size={10} /> 1-click
    </span>
  );
}
