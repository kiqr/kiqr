import { api, useAsync } from "../api";

export function Settings() {
  const health = useAsync(() => api.health(), []);
  const h = health.data;
  return (
    <div style={{ maxWidth: 640 }}>
      <h1 className="display" style={{ marginBottom: "var(--space-12)" }}>
        Settings
      </h1>
      <section className="section">
        <h2 className="title">Models</h2>
        <p className="section-desc">Which model runs each task. Set in <code>config.yaml</code>; keys live in <code>.env</code>.</p>
        {h?.providers.map((p) => (
          <div key={p.task} className="pf">
            <div className="pf-name">{p.task}</div>
            <div className="pf-value">
              <code>{p.model}</code>
            </div>
            <div className="pf-evidence" style={{ color: p.available ? "var(--success)" : "var(--warning)" }}>
              {p.available ? "Available" : "Not available — add the key to .env or install the CLI"}
            </div>
          </div>
        ))}
      </section>
      <section className="section">
        <h2 className="title">Connections</h2>
        <p className="section-desc">Services KIQR can use on your behalf.</p>
        <div className="pf">
          <div className="pf-name">UnblockingAPI</div>
          <div className="pf-value">Reads pages that block crawlers or need JavaScript.</div>
          <div className="pf-evidence">
            {h?.unblockingapi ? "Connected." : (
              <>
                Not connected. Add <code>UNBLOCKINGAPI_KEY</code> to <code>.env</code> and restart the daemon.
              </>
            )}
          </div>
        </div>
      </section>
      <section className="section">
        <h2 className="title">Storage</h2>
        <p className="section-desc">Everything lives in plain files under a local git repository. Every change is a commit.</p>
        <div className="pf">
          <div className="pf-name">Home</div>
          <div className="pf-value">
            <code>{h?.home}</code>
          </div>
          <div className="pf-evidence">KIQR {h?.version}</div>
        </div>
      </section>
    </div>
  );
}
