// Credentials and abilities recommend each other, by host: a Google sign-in
// in the Vault suggests Google Analytics and Search Console; an x.com login,
// posting on X. On the Abilities page, a shelf of what the Vault's
// credentials could do that nothing does yet ("Recommended for your
// credentials"); on a credential, every ability that could use it, allowed
// here or not. Allow grants it; Install and allow installs the ability first.
import { useState } from "react";
import { Check, Loader2, Plus, Sparkles, X } from "lucide-react";
import { api, useAsync, useEvents, VAULT_EVENTS, type Ability, type Recommendation } from "../api";
import { Favicon } from "./Favicon";
import { RELOAD_ON } from "./abilityBits";

type Rec = Recommendation["abilities"][number];

function readDismissed(slug: string): string[] {
  try {
    return JSON.parse(localStorage.getItem(`kiqr.recommend.dismissed.${slug}`) ?? "[]");
  } catch {
    return [];
  }
}

/** Allow (or install and allow) one recommendation. */
async function take(slug: string, credential: string, r: Rec) {
  if (r.status === "available") await api.installAbility(r.id);
  await api.grant(slug, r.id, credential);
}

function useRecs(slug: string, credential?: string) {
  const data = useAsync(() => api.recommendations(slug, credential).catch(() => ({ recommendations: [] as Recommendation[] })), [slug, credential]);
  useEvents(null, (e) => (VAULT_EVENTS.includes(e.type) || RELOAD_ON.includes(e.type)) && data.reload(), { demo: false });
  return data;
}

/** The Abilities page's shelf: what the Vault's credentials could do here that nothing does yet. */
export function Recommended({ slug, installed, market, onOpen }: { slug: string; installed: Ability[]; market: Ability[]; onOpen: (a: Ability) => void }) {
  const data = useRecs(slug);
  const [dismissed, setDismissed] = useState<string[]>(() => readDismissed(slug));
  const dismiss = (key: string) => {
    const next = [...dismissed, key];
    setDismissed(next);
    try {
      localStorage.setItem(`kiqr.recommend.dismissed.${slug}`, JSON.stringify(next));
    } catch {
      /* the shelf just shows it again */
    }
  };
  const groups = (data.data?.recommendations ?? [])
    .map((g) => ({ ...g, abilities: g.abilities.filter((r) => r.status !== "allowed" && !dismissed.includes(`${g.credential}:${r.id}`)) }))
    .filter((g) => g.abilities.length > 0);
  if (!groups.length) return null;
  const find = (id: string) => [...installed, ...market].find((a) => a.id === id);
  return (
    <section className="mk-sec rc">
      <h2>
        <Sparkles size={15} className="ac2" /> Recommended for your credentials
      </h2>
      <p className="muted sm">What the logins and sign-ins in your Vault could do here.</p>
      <div className="rc-groups">
        {groups.map((g) => (
          <div key={g.credential} className="rc-group">
            <p className="rc-hd">
              <Favicon domain={g.domain ?? undefined} size={18} /> <b>{g.label}</b>
            </p>
            <div className="rc-tiles">
              {g.abilities.map((r) => (
                <RecTile key={r.id} slug={slug} credential={g.credential} r={r} onOpen={() => find(r.id) && onOpen(find(r.id)!)} onDismiss={() => dismiss(`${g.credential}:${r.id}`)} onDone={data.reload} />
              ))}
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}

function RecTile({ slug, credential, r, onOpen, onDismiss, onDone }: { slug: string; credential: string; r: Rec; onOpen: () => void; onDismiss?: () => void; onDone: () => void }) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const go = async () => {
    setBusy(true);
    setError(null);
    try {
      await take(slug, credential, r);
      onDone();
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="rc-tile">
      <button className="rc-name" onClick={onOpen}>
        <b>{r.name}</b>
        <small>{r.for || r.summary}</small>
        {r.more_access && <small className="warn">Sign in again to give it access</small>}
      </button>
      {r.status === "allowed" ? (
        <span className="chip ok sm">
          <Check size={11} /> Allowed
        </span>
      ) : (
        <button className="btn g sm" disabled={busy} onClick={() => void go()} title={r.status === "available" ? "Install it, and let it use this credential in this project" : "Let it use this credential in this project"}>
          {busy ? <Loader2 size={13} className="spin" /> : <Plus size={13} />} {r.status === "available" ? "Install and allow" : "Allow"}
        </button>
      )}
      {onDismiss && (
        <button className="x" aria-label="Not now" title="Not now" onClick={onDismiss}>
          <X size={13} />
        </button>
      )}
      {error && <p className="err sm">{error}</p>}
    </div>
  );
}

/** On a credential: every ability that could use it in this project, by host. */
export function CredentialAbilities({ slug, credential }: { slug: string; credential: string }) {
  const data = useRecs(slug, credential);
  const list = data.data?.recommendations[0]?.abilities ?? [];
  if (!data.data) return <Loader2 size={14} className="spin" />;
  if (!list.length) return <p className="muted sm">Nothing installed or in the marketplace works with it yet.</p>;
  const order = { allowed: 0, installed: 1, available: 2 } as const;
  return (
    <div className="rc-tiles col">
      {[...list]
        .sort((a, b) => order[a.status] - order[b.status])
        .map((r) => (
          <RecTile key={r.id} slug={slug} credential={credential} r={r} onOpen={() => {}} onDone={data.reload} />
        ))}
    </div>
  );
}
