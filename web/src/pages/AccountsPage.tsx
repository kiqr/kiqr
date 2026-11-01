// Setup → Accounts: everything Seemo signs in to for this project, grouped
// by kind of service (Social, Website and blog, Email, Numbers…). Each card
// says what Seemo may do there in one line with one switch, where inside it
// content goes, and keeps the rest under Details. "Connect an account" opens
// the wizard.
import { useMemo } from "react";
import { Link, useLocation, useParams } from "react-router-dom";
import { Plug, Plus } from "lucide-react";
import { api, useAsync, useEvents, type MailList } from "../api";
import { AccountCard, accountService, ACCOUNT_EVENTS, byGroup, connectPath, LIST_EVENTS } from "./Accounts";
import "../profile.css";
import "../accounts.css";

export function AccountsPage() {
  const { slug = "" } = useParams();
  const location = useLocation();
  const data = useAsync(() => api.accounts(slug), [slug]);
  const lists = useAsync(() => api.lists(slug).catch(() => ({ lists: [] as MailList[] })), [slug]);
  useEvents(slug, (e) => {
    if ([...ACCOUNT_EVENTS, "asset_published"].includes(e.type)) data.reload();
    if (LIST_EVENTS.includes(e.type)) lists.reload();
  }, { demo: false });
  useEvents(null, (e) => ["extension_installed", "extension_removed"].includes(e.type) && data.reload(), { demo: false });
  // The wizard lands here with the new account: `#account=<id>` lights its card.
  const highlight = /#account=([^&]+)/.exec(location.hash)?.[1] ?? null;
  const all = data.data?.connections ?? [];
  const groups = useMemo(() => byGroup(all, (a) => accountService(a).group), [all]);
  const several = (id: string) => all.filter((x) => accountService(x).id === id).length > 1;

  return (
    <div className="prof">
      <div className="cm-head">
        <div>
          <p className="cap">Setup</p>
          <h1 className="d2">Accounts</h1>
          <p className="muted">Everything Seemo signs in to for this project: your social accounts, your mail account, your site, your analytics. On each you choose what Seemo may do there, and where inside it things go.</p>
        </div>
        <Link className="btn p" to={connectPath(slug)}>
          <Plus size={15} /> Connect an account
        </Link>
      </div>

      {data.error && <p className="err">{data.error}</p>}

      {data.data && all.length === 0 && (
        <div className="pcard none">
          <Plug size={22} className="ac2" />
          <p className="t">No accounts yet</p>
          <p className="muted">Connect where you post, the address you send from, your site or your analytics. Seemo asks before it acts on any of them until you say otherwise.</p>
          <Link className="btn g" to={connectPath(slug)}>
            <Plus size={14} /> Connect an account
          </Link>
        </div>
      )}

      <div className="acx-groups">
        {groups.map((g) => (
          <section key={g.id} className="acx-group">
            <h2>
              {g.label} <span className="n">{g.items.length}</span>
            </h2>
            <div className="acx-grid">
              {g.items.map((a) => (
                <AccountCard key={a.id} slug={slug} a={a} lists={lists.data?.lists ?? []} several={several(accountService(a).id)} highlight={a.id === highlight} onChange={data.reload} />
              ))}
            </div>
          </section>
        ))}
      </div>
    </div>
  );
}
