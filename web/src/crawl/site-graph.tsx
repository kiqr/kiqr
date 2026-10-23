// Canvas + d3-force radial site map: radar beam, packets, glow sprites. Ported from ContentWriter.
import { useEffect, useRef, useState } from "react";
import { forceCollide, forceLink, forceManyBody, forceRadial, forceSimulation, type Simulation } from "d3-force";
import type { CrawlStore, GraphNode, NodeStatus } from "./store";

// ---------- palette ----------
const STATUS_COLOR: Record<NodeStatus, string> = {
  queued: "#56627c",
  fetching: "#5eead4",
  rendering: "#a78bfa",
  ok: "#34d399",
  blocked: "#fbbf24",
  error: "#f87171",
  skipped: "#475569",
};
const READING = "#818cf8";
const MONO = '"JetBrains Mono", ui-monospace, monospace';

const RING = 78; // world px between depth rings
const BEAM_SPEED = 0.0011; // rad per ms
const SPAWN_MS = 650;
const PACKET_MS = 700;

interface SimNode {
  id: string;
  depth: number;
  x: number;
  y: number;
  vx?: number;
  vy?: number;
  fx?: number | null;
  fy?: number | null;
  r: number;
}
interface SimLink {
  source: string | SimNode;
  target: string | SimNode;
}

const easeOutBack = (t: number) => {
  const c1 = 1.70158;
  const c3 = c1 + 1;
  return 1 + c3 * Math.pow(t - 1, 3) + c1 * Math.pow(t - 1, 2);
};
const clamp = (v: number, a: number, b: number) => Math.min(b, Math.max(a, v));

function nodeRadius(n: GraphNode, isRoot: boolean) {
  if (isRoot) return 9;
  const base = 3.2 + Math.min(Math.sqrt(n.inbound) * 1.1, 7);
  return base;
}

/** Pre-rendered radial glow sprites (much cheaper than shadowBlur). */
const glowCache = new Map<string, HTMLCanvasElement>();
function glowSprite(color: string): HTMLCanvasElement {
  let c = glowCache.get(color);
  if (c) return c;
  c = document.createElement("canvas");
  c.width = c.height = 64;
  const g = c.getContext("2d")!;
  const grad = g.createRadialGradient(32, 32, 0, 32, 32, 32);
  grad.addColorStop(0, color + "cc");
  grad.addColorStop(0.25, color + "55");
  grad.addColorStop(1, color + "00");
  g.fillStyle = grad;
  g.fillRect(0, 0, 64, 64);
  glowCache.set(color, c);
  return c;
}

export interface HoverInfo {
  node: GraphNode;
  x: number;
  y: number;
  /** Width of the map, for keeping the tooltip inside it. */
  w: number;
}

export function SiteGraph({
  store,
  reducedMotion,
  onHover,
}: {
  store: CrawlStore;
  reducedMotion: boolean;
  onHover?: (h: HoverInfo | null) => void;
}) {
  const wrapRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [userCamera, setUserCamera] = useState(false);
  const userCameraRef = useRef(false);
  userCameraRef.current = userCamera;
  const zoomRef = useRef<(factor: number) => void>(() => {});
  const onHoverRef = useRef(onHover);
  onHoverRef.current = onHover;

  useEffect(() => {
    const canvas = canvasRef.current!;
    const wrap = wrapRef.current!;
    const ctx = canvas.getContext("2d")!;
    let width = 0;
    let height = 0;
    let dpr = 1;

    const resize = () => {
      const rect = wrap.getBoundingClientRect();
      dpr = Math.min(window.devicePixelRatio || 1, 2);
      width = rect.width;
      height = rect.height;
      canvas.width = Math.round(width * dpr);
      canvas.height = Math.round(height * dpr);
      canvas.style.width = `${width}px`;
      canvas.style.height = `${height}px`;
    };
    resize();
    const ro = new ResizeObserver(resize);
    ro.observe(wrap);

    // ---------- simulation ----------
    const simNodes = new Map<string, SimNode>();
    const simLinks: SimLink[] = [];

    const linkForce = forceLink<SimNode, SimLink>(simLinks)
      .id((d) => d.id)
      .distance((l) => 26 + (l.target as SimNode).r * 2)
      .strength(0.5);
    const radial = forceRadial<SimNode>((d) => d.depth * RING, 0, 0).strength(0.55);
    const sim: Simulation<SimNode, SimLink> = forceSimulation<SimNode, SimLink>([])
      .force("charge", forceManyBody<SimNode>().strength(-26).distanceMax(260))
      .force("link", linkForce)
      .force("collide", forceCollide<SimNode>((d) => d.r + 2.5).iterations(2))
      .force("radial", radial)
      .alphaDecay(0.012)
      .velocityDecay(0.35)
      .stop();

    function syncSim() {
      const s = store.state;
      let added = false;
      let rootId: string | null = null;
      for (const n of s.nodes.values()) {
        rootId ??= n.id;
        const existing = simNodes.get(n.id);
        if (existing) {
          existing.r = nodeRadius(n, n.id === rootId);
          continue;
        }
        const parent = n.parent ? simNodes.get(n.parent) : undefined;
        const angle = Math.random() * Math.PI * 2;
        const sn: SimNode = {
          id: n.id,
          depth: n.depth,
          x: parent ? parent.x + Math.cos(angle) * 6 : 0,
          y: parent ? parent.y + Math.sin(angle) * 6 : 0,
          r: nodeRadius(n, n.id === rootId),
        };
        if (n.id === rootId) {
          sn.fx = 0;
          sn.fy = 0;
        }
        simNodes.set(n.id, sn);
        if (parent) simLinks.push({ source: n.parent!, target: n.id });
        added = true;
      }
      if (added) {
        sim.nodes([...simNodes.values()]);
        linkForce.links(simLinks);
        sim.alpha(Math.max(sim.alpha(), reducedMotion ? 0.3 : 0.6));
      }

    }

    // ---------- camera ----------
    const cam = { x: 0, y: 0, k: 1.4 };
    let dragging: { x: number; y: number; cx: number; cy: number } | null = null;
    let dragMoved = false;

    const toWorld = (sx: number, sy: number) => ({ x: (sx - width / 2) / cam.k + cam.x, y: (sy - height / 2) / cam.k + cam.y });

    function autoFit() {
      if (userCameraRef.current || simNodes.size === 0) return;
      let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
      for (const n of simNodes.values()) {
        minX = Math.min(minX, n.x); maxX = Math.max(maxX, n.x);
        minY = Math.min(minY, n.y); maxY = Math.max(maxY, n.y);
      }
      const pad = 70;
      const bw = Math.max(maxX - minX, 120) + pad * 2;
      const bh = Math.max(maxY - minY, 120) + pad * 2;
      const targetK = clamp(Math.min(width / bw, height / bh), 0.22, 2.2);
      const tx = (minX + maxX) / 2;
      const ty = (minY + maxY) / 2;
      const lerp = reducedMotion ? 1 : 0.045;
      cam.k += (targetK - cam.k) * lerp;
      cam.x += (tx - cam.x) * lerp;
      cam.y += (ty - cam.y) * lerp;
    }

    // ---------- interaction ----------
    let hoveredId: string | null = null;
    const onWheel = (ev: WheelEvent) => {
      // Plain scrolling scrolls the page; Ctrl/⌘ + scroll (and trackpad pinch) zooms the map.
      if (!ev.ctrlKey && !ev.metaKey) return;
      ev.preventDefault();
      const rect = canvas.getBoundingClientRect();
      const before = toWorld(ev.clientX - rect.left, ev.clientY - rect.top);
      cam.k = clamp(cam.k * Math.exp(-ev.deltaY * 0.0015), 0.15, 5);
      const after = toWorld(ev.clientX - rect.left, ev.clientY - rect.top);
      cam.x += before.x - after.x;
      cam.y += before.y - after.y;
      setUserCamera(true);
    };
    const onDown = (ev: PointerEvent) => {
      dragging = { x: ev.clientX, y: ev.clientY, cx: cam.x, cy: cam.y };
      dragMoved = false;
      canvas.setPointerCapture(ev.pointerId);
    };
    const onUp = (ev: PointerEvent) => {
      dragging = null;
      canvas.releasePointerCapture(ev.pointerId);
    };
    const onMove = (ev: PointerEvent) => {
      const rect = canvas.getBoundingClientRect();
      if (dragging) {
        const dx = ev.clientX - dragging.x;
        const dy = ev.clientY - dragging.y;
        if (Math.abs(dx) + Math.abs(dy) > 3) {
          dragMoved = true;
          setUserCamera(true);
        }
        cam.x = dragging.cx - dx / cam.k;
        cam.y = dragging.cy - dy / cam.k;
        return;
      }
      const w = toWorld(ev.clientX - rect.left, ev.clientY - rect.top);
      let best: SimNode | null = null;
      let bestD = Infinity;
      for (const n of simNodes.values()) {
        const d = Math.hypot(n.x - w.x, n.y - w.y);
        if (d < bestD && d < Math.max(n.r + 6, 10 / cam.k)) {
          best = n;
          bestD = d;
        }
      }
      const id = best?.id ?? null;
      if (id !== hoveredId) {
        hoveredId = id;
        canvas.style.cursor = id ? "pointer" : "grab";
      }
      const gn = id ? store.state.nodes.get(id) : undefined;
      onHoverRef.current?.(gn ? { node: gn, x: ev.clientX - rect.left, y: ev.clientY - rect.top, w: width } : null);
    };
    const onLeave = () => {
      hoveredId = null;
      onHoverRef.current?.(null);
    };
    const onClick = () => {
      if (hoveredId && !dragMoved) window.open(hoveredId, "_blank", "noopener");
    };
    canvas.addEventListener("wheel", onWheel, { passive: false });
    zoomRef.current = (factor: number) => {
      cam.k = clamp(cam.k * factor, 0.15, 5);
      setUserCamera(true);
    };
    canvas.addEventListener("pointerdown", onDown);
    canvas.addEventListener("pointerup", onUp);
    canvas.addEventListener("pointermove", onMove);
    canvas.addEventListener("pointerleave", onLeave);
    canvas.addEventListener("click", onClick);

    // ---------- render ----------
    const packets: { url: string; dir: 1 | -1; at: number }[] = [];
    const bursts: { url: string; at: number; color: string }[] = [];
    let raf = 0;
    let beamAngle = -Math.PI / 2;
    let last = performance.now();

    const nodeColor = (n: GraphNode): string => STATUS_COLOR[n.status];

    function frame(now: number) {
      const dt = Math.min(now - last, 64);
      last = now;
      const s = store.state;
      syncSim();

      // consume effects
      for (const e of store.effects.splice(0)) {
        if (reducedMotion) continue;
        if (e.kind === "packet-out") packets.push({ url: e.url, dir: 1, at: now });
        else if (e.kind === "packet-in") packets.push({ url: e.url, dir: -1, at: now });
        else bursts.push({ url: e.url, at: now, color: e.color ?? "#ffffff" });
      }

      if (sim.alpha() > sim.alphaMin()) sim.tick(reducedMotion ? 3 : 1);
      autoFit();

      const active = s.phase === "crawling" || s.phase === "discovering" || s.phase === "connecting";
      const learning = s.phase === "learning";
      if (!reducedMotion && (active || learning)) beamAngle += dt * BEAM_SPEED * (learning ? 1.8 : 1);

      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.clearRect(0, 0, width, height);
      ctx.save();
      ctx.translate(width / 2, height / 2);
      ctx.scale(cam.k, cam.k);
      ctx.translate(-cam.x, -cam.y);

      const root = simNodes.values().next().value as SimNode | undefined;
      const rx = root?.x ?? 0;
      const ry = root?.y ?? 0;
      let maxDepth = 1;
      for (const n of simNodes.values()) maxDepth = Math.max(maxDepth, n.depth);
      const ringFade = 1;

      // depth rings
      ctx.lineWidth = 1 / cam.k;
      for (let d = 1; d <= Math.min(maxDepth + 1, 8); d++) {
        ctx.beginPath();
        ctx.arc(rx, ry, d * RING, 0, Math.PI * 2);
        ctx.strokeStyle = `rgba(94,234,212,${0.07 * ringFade})`;
        ctx.setLineDash([4 / cam.k, 6 / cam.k]);
        ctx.stroke();
      }
      ctx.setLineDash([]);
      if (ringFade === 1 && cam.k > 0.45) {
        ctx.fillStyle = "rgba(131,146,173,0.45)";
        ctx.font = `${10 / cam.k}px ${MONO}`;
        for (let d = 1; d <= Math.min(maxDepth, 7); d++) ctx.fillText(`depth ${d}`, rx + 4 / cam.k, ry - d * RING - 4 / cam.k);
      }

      // radar beam
      const beamR = (Math.min(maxDepth, 7) + 1.2) * RING;
      if (!reducedMotion && (active || learning)) {
        const g = ctx.createConicGradient(beamAngle - 0.9, rx, ry);
        const tint = learning ? "129,140,248" : "94,234,212";
        g.addColorStop(0, `rgba(${tint},0)`);
        g.addColorStop(0.9 / (Math.PI * 2), `rgba(${tint},0.16)`);
        g.addColorStop(0.9 / (Math.PI * 2) + 0.002, `rgba(${tint},0)`);
        g.addColorStop(1, `rgba(${tint},0)`);
        ctx.fillStyle = g;
        ctx.beginPath();
        ctx.arc(rx, ry, beamR, 0, Math.PI * 2);
        ctx.fill();
        ctx.beginPath();
        ctx.moveTo(rx, ry);
        ctx.lineTo(rx + Math.cos(beamAngle) * beamR, ry + Math.sin(beamAngle) * beamR);
        ctx.strokeStyle = `rgba(${tint},0.55)`;
        ctx.lineWidth = 1.5 / cam.k;
        ctx.stroke();
      }

      // tree edges
      ctx.lineWidth = 1 / cam.k;
      for (const l of simLinks) {
        const a = l.source as SimNode;
        const b = l.target as SimNode;
        if (typeof a !== "object" || typeof b !== "object") continue;
        const gn = s.nodes.get(b.id);
        const age = gn ? clamp((now - gn.spawnedAt) / SPAWN_MS, 0, 1) : 1;
        const hot = gn && (gn.status === "fetching" || gn.status === "rendering");
        ctx.strokeStyle = hot ? "rgba(94,234,212,0.55)" : `rgba(148,163,184,${0.16 * age})`;
        ctx.beginPath();
        ctx.moveTo(a.x, a.y);
        // draw edge "growing" from parent while spawning
        ctx.lineTo(a.x + (b.x - a.x) * age, a.y + (b.y - a.y) * age);
        ctx.stroke();
      }

      // packets travelling root <-> node
      ctx.globalCompositeOperation = "lighter";
      for (let i = packets.length - 1; i >= 0; i--) {
        const p = packets[i]!;
        const t = (now - p.at) / PACKET_MS;
        const target = simNodes.get(p.url);
        if (t >= 1 || !target) {
          packets.splice(i, 1);
          continue;
        }
        const k = p.dir === 1 ? t : 1 - t;
        const x = rx + (target.x - rx) * k;
        const y = ry + (target.y - ry) * k;
        const color = p.dir === 1 ? "#5eead4" : "#34d399";
        const size = 14 / Math.sqrt(cam.k);
        ctx.drawImage(glowSprite(color), x - size / 2, y - size / 2, size, size);
      }

      // nodes
      for (const sn of simNodes.values()) {
        const n = s.nodes.get(sn.id);
        if (!n) continue;
        const color = nodeColor(n);
        const spawn = clamp((now - n.spawnedAt) / SPAWN_MS, 0, 1);
        const scale = reducedMotion ? 1 : easeOutBack(spawn);
        let r = sn.r * scale;

        // beam glint
        let glint = 0;
        if (active && !reducedMotion) {
          const a = Math.atan2(sn.y - ry, sn.x - rx);
          let diff = (beamAngle - a) % (Math.PI * 2);
          if (diff < 0) diff += Math.PI * 2;
          if (diff < 0.5) glint = 1 - diff / 0.5;
        }

        const fetching = n.status === "fetching" || n.status === "rendering";
        if (fetching && !reducedMotion) {
          const ph = ((now - n.changedAt) % 900) / 900;
          ctx.beginPath();
          ctx.arc(sn.x, sn.y, r + 3 + ph * 14, 0, Math.PI * 2);
          ctx.strokeStyle = `${color}${Math.round((1 - ph) * 200).toString(16).padStart(2, "0")}`;
          ctx.lineWidth = 1.5 / cam.k;
          ctx.stroke();
          r *= 1 + Math.sin(now / 120) * 0.15;
        }

        const glowSize = (r * 5.5) * (1 + glint * 0.6);
        if (n.status !== "queued" && n.status !== "skipped") {
          ctx.globalAlpha = fetching ? 1 : 0.55 + glint * 0.45;
          ctx.drawImage(glowSprite(color), sn.x - glowSize / 2, sn.y - glowSize / 2, glowSize, glowSize);
          ctx.globalAlpha = 1;
        }

        ctx.globalCompositeOperation = "source-over";
        ctx.beginPath();
        ctx.arc(sn.x, sn.y, Math.max(r, 0.5), 0, Math.PI * 2);
        if (n.status === "queued") {
          ctx.fillStyle = "#0c1220";
          ctx.fill();
          ctx.strokeStyle = color;
          ctx.lineWidth = 1.2 / cam.k;
          ctx.stroke();
        } else {
          ctx.fillStyle = color;
          ctx.fill();
        }
        // with the model: a slow indigo orbit
        if (n.reading) {
          ctx.beginPath();
          const spin = reducedMotion ? 0 : now / 600;
          ctx.arc(sn.x, sn.y, r + 3.5, spin, spin + Math.PI * 1.3);
          ctx.strokeStyle = READING;
          ctx.lineWidth = 1.4 / cam.k;
          ctx.stroke();
        }
        if (sn.id === hoveredId) {
          ctx.strokeStyle = "#ffffff";
          ctx.lineWidth = 2 / cam.k;
          ctx.stroke();
        }
        ctx.globalCompositeOperation = "lighter";
      }

      // completion bursts
      for (let i = bursts.length - 1; i >= 0; i--) {
        const b = bursts[i]!;
        const t = (now - b.at) / 520;
        const sn = simNodes.get(b.url);
        if (t >= 1 || !sn) {
          bursts.splice(i, 1);
          continue;
        }
        ctx.beginPath();
        ctx.arc(sn.x, sn.y, sn.r + t * 18, 0, Math.PI * 2);
        ctx.strokeStyle = b.color;
        ctx.globalAlpha = (1 - t) * 0.7;
        ctx.lineWidth = 2 / cam.k;
        ctx.stroke();
        ctx.globalAlpha = 1;
      }
      ctx.globalCompositeOperation = "source-over";

      // labels: root + hovered always; shallow nodes when zoomed in, skipping ones that would overlap
      ctx.font = `${11 / cam.k}px ${MONO}`;
      ctx.textAlign = "center";
      const placed: { x: number; y: number; w: number }[] = [];
      const lh = 13 / cam.k;
      const candidates = [...simNodes.values()].sort((a, b) => b.r - a.r);
      for (const sn of candidates) {
        const n = s.nodes.get(sn.id);
        if (!n) continue;
        const isRoot = n.depth === 0;
        const want = sn.id === hoveredId || isRoot || (n.depth <= 1 && cam.k > 1.1);
        if (!want) continue;
        const label = isRoot ? hostOf(n.id) : n.path.length > 22 ? n.path.slice(0, 21) + "…" : n.path;
        const w = ctx.measureText(label).width;
        const ly = sn.y - sn.r - 6 / cam.k;
        const overlaps = placed.some((p) => Math.abs(p.y - ly) < lh && Math.abs(p.x - sn.x) < (p.w + w) / 2 + 4 / cam.k);
        if (overlaps && sn.id !== hoveredId && !isRoot) continue;
        placed.push({ x: sn.x, y: ly, w });
        ctx.fillStyle = "rgba(7,10,18,0.75)";
        ctx.fillRect(sn.x - w / 2 - 3 / cam.k, ly - lh + 2 / cam.k, w + 6 / cam.k, lh + 1 / cam.k);
        ctx.fillStyle = sn.id === hoveredId ? "#ffffff" : "rgba(230,236,247,0.75)";
        ctx.fillText(label, sn.x, ly);
      }

      ctx.textAlign = "start";
      ctx.restore();

      raf = requestAnimationFrame(frame);
    }
    raf = requestAnimationFrame(frame);

    return () => {
      cancelAnimationFrame(raf);
      ro.disconnect();
      sim.stop();
      canvas.removeEventListener("wheel", onWheel);
      canvas.removeEventListener("pointerdown", onDown);
      canvas.removeEventListener("pointerup", onUp);
      canvas.removeEventListener("pointermove", onMove);
      canvas.removeEventListener("pointerleave", onLeave);
      canvas.removeEventListener("click", onClick);
    };
  }, [store, reducedMotion]);

  return (
    <div ref={wrapRef} className="cr-graph-canvas">
      <canvas ref={canvasRef} aria-label="Live map of crawled pages" role="img" />
      <div className="cr-graph-ctl">
        <span className="cr-hint">Ctrl + scroll to zoom · drag to pan</span>
        {userCamera && (
          <button onClick={() => setUserCamera(false)} className="cr-ctl">
            Re-center
          </button>
        )}
        <button onClick={() => zoomRef.current(1.25)} aria-label="Zoom in" className="cr-ctl sq">
          +
        </button>
        <button onClick={() => zoomRef.current(0.8)} aria-label="Zoom out" className="cr-ctl sq">
          −
        </button>
      </div>
    </div>
  );
}

function hostOf(url: string) {
  try {
    return new URL(url).hostname.replace(/^www\./, "");
  } catch {
    return url;
  }
}
