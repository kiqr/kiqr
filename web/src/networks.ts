// The project's networks (channel kinds), loaded once per project and shared
// by every post card, the channel picker and the profile's social links.
// Everything about a network comes from the ability that declares it.
import { useEffect, useState } from "react";
import { api, useEvents, type Asset, type Network } from "./api";

const cache = new Map<string, Promise<Network[]>>();
const RELOAD = ["extension_installed", "extension_removed", "extension_paused", "marketplace_refreshed"];

function load(slug: string): Promise<Network[]> {
  let p = cache.get(slug);
  if (!p) {
    p = api.networks(slug).then((r) => r.networks ?? [], () => []);
    cache.set(slug, p);
  }
  return p;
}

export function useNetworks(slug: string): Network[] {
  const [list, setList] = useState<Network[]>([]);
  const [key, setKey] = useState(0);
  useEffect(() => {
    let live = true;
    load(slug).then((l) => live && setList(l));
    return () => {
      live = false;
    };
  }, [slug, key]);
  useEvents(null, (e) => {
    if (!RELOAD.includes(e.type)) return;
    cache.delete(slug);
    setKey((k) => k + 1);
  }, { demo: false });
  return list;
}

/** A post's network: the channel kind in its tags that an installed ability declares. */
export function networkOf(a: Pick<Asset, "tags">, networks: Network[]): Network | null {
  for (const t of a.tags.map((t) => t.toLowerCase())) {
    const n = networks.find((n) => n.kind === t && n.installed);
    if (n) return n;
  }
  return null;
}

/** A kind's name as people write it, else the kind itself. */
export function networkName(kind: string, networks: Network[]): string {
  return networks.find((n) => n.kind === kind)?.name || kind;
}
