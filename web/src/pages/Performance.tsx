// Your business → Performance: the numbers against the plan. Each objective's
// KPI with its value and target, the metrics per source with the last seven
// days against the seven before and a 30-day line, the top queries and pages,
// and where each number came from. Read from daily snapshots; Refresh reads
// the sources again.
import { useState } from "react";
import { Link, useParams } from "react-router-dom";
import { AlertTriangle, ArrowDownRight, ArrowUpRight, BarChart3, Loader2, Minus, RefreshCw, Sparkles, Target } from "lucide-react";
import { api, useAsync, useEvents, type PerfKpi, type PerfMetric } from "../api";
import { ago } from "./Project";
import "../profile.css";

export const PERF_EVENTS = ["metrics_snapshot", "metrics_failed", "plan_saved", "extension_installed", "extension_removed", "secret_set", "config_changed"];

export function PerformancePage() {
  const { slug = "" } = useParams();
  const data = useAsync(() => api.performance(slug), [slug]);
  useEvents(slug, (e) => PERF_EVENTS.includes(e.type) && data.reload(), { demo: false });
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState<{ ok: boolean; text: string }[] | null>(null);
  const d = data.data;
  const refresh = async () => {
    setBusy(true);
    setNote(null);
    try {
      const r = await api.refreshPerformance(slug);
      setNote((r.refreshed ?? []).map((x) => ({ ok: x.ok, text: x.note })));
      data.reload();
    } catch (e) {
      setNote([{ ok: false, text: (e as Error).message }]);
    } finally {
      setBusy(false);
    }
  };
  const connected = (d?.sources ?? []).filter((s) => s.provider && s.id !== "kiqr");
  const bySource = (id: string) => (d?.metrics ?? []).filter((m) => m.source === id);
  return (
    <div className="prof perf">
      <div className="cm-head">
        <div>
          <p className="cap">Marketing</p>
          <h1 className="d2">Performance</h1>
          <p className="muted">The numbers against the plan. Read once a day from the sources you connect; nothing here waits on them. Seemo reads the same snapshots when you ask how it is going.</p>
        </div>
        <div className="cm-acts">
          <button className="btn g" disabled={busy || !d} onClick={() => void refresh()}>
            {busy ? <Loader2 size={15} className="spin" /> : <RefreshCw size={15} />} Refresh
          </button>
        </div>
      </div>
      {note && (
        <ul className="perf-notes">
          {note.map((n, i) => (
            <li key={i} className={n.ok ? "ok" : "err"}>
              {n.ok ? <Sparkles size={12} /> : <AlertTriangle size={12} />} {n.text}
            </li>
          ))}
        </ul>
      )}

      {d && connected.length === 0 && (
        <div className="pcard none">
          <BarChart3 size={22} className="ac2" />
          <p className="t">No source connected yet</p>
          <p className="muted">Install an analytics ability from the marketplace (Setup → Abilities, under Analytics) and connect it with this project's keys. What KIQR publishes is counted either way.</p>
          <Link className="btn g" to={`/p/${slug}/abilities#marketplace`}>
            <Sparkles size={14} /> Open the marketplace
          </Link>
        </div>
      )}

      {d && d.kpis.length > 0 && (
        <section className="perf-sec">
          <h2>
            Plan KPIs <span className="n">{d.plan_period ?? ""}</span>
            <Link className="lnk sm" to={`/p/${slug}/plan`}>
              The plan
            </Link>
          </h2>
          <div className="perf-tiles">
            {d.kpis.map((k) => (
              <KpiTile key={k.objective} k={k} />
            ))}
          </div>
        </section>
      )}

      {d &&
        d.sources
          .filter((s) => bySource(s.id).length > 0)
          .map((s) => (
            <section className="perf-sec" key={s.id}>
              <h2>
                {s.label} <span className="n">{s.provider && s.provider !== "kiqr" ? `via ${s.provider}` : ""}</span>
                {s.taken_at && <small className="muted">taken {ago(s.taken_at)}</small>}
              </h2>
              <div className="perf-tiles">
                {bySource(s.id).map((m) => (
                  <MetricTile key={m.metric} m={m} />
                ))}
              </div>
              <div className="perf-tops">
                {Object.entries(d.tops)
                  .filter(([k]) => k.startsWith(`${s.id}.`))
                  .map(([k, rows]) => {
                    const [, metric, dim] = k.split(".");
                    return (
                      <div className="pcard perf-top" key={k}>
                        <p className="cap">
                          {dim === "query" ? "Top queries" : dim === "page" ? "Top pages" : dim === "event" ? "Key events" : dim === "channel" ? "By channel" : dim === "source" ? "Sources" : dim} · {metric.replace("_", " ")}
                        </p>
                        <table>
                          <tbody>
                            {rows.slice(0, 12).map((r) => (
                              <tr key={r.key}>
                                <td className="k">{r.key}</td>
                                <td className="v mono">{fmt(r.value)}</td>
                              </tr>
                            ))}
                          </tbody>
                        </table>
                      </div>
                    );
                  })}
              </div>
            </section>
          ))}

      {d && (
        <section className="perf-sec">
          <h2>Sources</h2>
          <ul className="perf-sources">
            {d.sources
              .filter((s) => s.id !== "kiqr")
              .map((s) => (
                <li key={s.id} className={s.provider ? "on" : ""}>
                  <b>{s.label}</b>
                  {s.provider ? (
                    <span>
                      via {s.candidates.find((c) => c.id === s.provider)?.name ?? s.provider}
                      {s.taken_at ? ` · taken ${ago(s.taken_at)}` : " · not read yet"}
                    </span>
                  ) : s.candidates.length ? (
                    <span className="warn">
                      {s.candidates.map((c) => c.name).join(", ")} installed but not connected. <Link to={`/p/${slug}/abilities#ability=${s.candidates[0].id}`}>Connect</Link>
                    </span>
                  ) : (
                    <span className="muted">
                      nothing reads it yet. <Link to={`/p/${slug}/abilities#marketplace`}>Marketplace</Link>
                    </span>
                  )}
                </li>
              ))}
          </ul>
        </section>
      )}
    </div>
  );
}

export function fmt(v: number | null | undefined, unit = "") {
  if (v == null) return "–";
  const n = Math.abs(v) >= 1000 ? Math.round(v).toLocaleString() : Number.isInteger(v) ? String(v) : v.toFixed(1);
  return unit ? `${n} ${unit}` : n;
}

function Trend({ now, prev, lower }: { now: number | null; prev: number | null; lower?: boolean }) {
  if (now == null || prev == null || prev === 0) return <span className="trend flat">{prev === 0 && now ? "new" : ""}</span>;
  const pct = ((now - prev) / prev) * 100;
  const up = pct > 0.5;
  const down = pct < -0.5;
  const good = lower ? down : up;
  const bad = lower ? up : down;
  return (
    <span className={`trend ${good ? "good" : bad ? "bad" : "flat"}`}>
      {up ? <ArrowUpRight size={12} /> : down ? <ArrowDownRight size={12} /> : <Minus size={12} />} {Math.abs(pct).toFixed(0)}%
    </span>
  );
}

/** A 30-day line, no axes: the shape is the point. */
function Spark({ series, lower }: { series: { date: string; value: number }[]; lower?: boolean }) {
  if (series.length < 2) return null;
  const w = 160;
  const h = 36;
  const vals = series.map((p) => p.value);
  const min = Math.min(...vals);
  const max = Math.max(...vals);
  const span = max - min || 1;
  const pts = series.map((p, i) => `${((i / (series.length - 1)) * w).toFixed(1)},${(h - 2 - ((p.value - min) / span) * (h - 4)).toFixed(1)}`).join(" ");
  const rising = vals[vals.length - 1] >= vals[0];
  const good = lower ? !rising : rising;
  return (
    <svg className={`spark ${good ? "good" : "bad"}`} viewBox={`0 0 ${w} ${h}`} width={w} height={h} aria-hidden>
      <polyline points={pts} fill="none" strokeWidth="1.5" />
    </svg>
  );
}

function MetricTile({ m }: { m: PerfMetric }) {
  const unit = m.metric === "conversion_rate" || m.metric === "ctr" ? "%" : "";
  return (
    <div className="pcard perf-tile">
      <p className="cap">{m.label}</p>
      <p className="big">
        {fmt(m.last7, unit)} <Trend now={m.last7} prev={m.prev7} lower={m.lower_is_better} />
      </p>
      <p className="muted sm">last 7 days · {fmt(m.last30, unit)} in 30</p>
      <Spark series={m.series} lower={m.lower_is_better} />
    </div>
  );
}

function KpiTile({ k }: { k: PerfKpi }) {
  const pct = k.target && k.value != null ? Math.min(100, Math.round((k.value / k.target) * 100)) : null;
  return (
    <div className={`pcard perf-tile kpi ${k.measured ? "" : "dim"}`}>
      <p className="cap">
        <Target size={12} /> {k.label ?? k.kpi}
      </p>
      <p className="ttl">{k.title}</p>
      {k.measured ? (
        <>
          <p className="big">
            {fmt(k.value, k.unit)} <Trend now={k.value} prev={k.prev} lower={k.kpi === "rank_positions" || k.kpi === "cac"} />
          </p>
          {k.target != null && (
            <>
              <p className="muted sm">
                of {fmt(k.target, k.unit)} · {pct}%
              </p>
              <div className="bar">
                <i style={{ width: `${pct ?? 0}%` }} />
              </div>
            </>
          )}
          {k.target == null && <p className="muted sm">last {k.window_days} days · no target set</p>}
        </>
      ) : (
        <p className="muted sm">{k.source ? `Needs a ${k.source} source` : "Nothing measures this KPI yet"}</p>
      )}
    </div>
  );
}
