// A domain's icon: its favicon through the daemon (fetched once and kept),
// else its first letter on a tile. Credentials, channels and abilities all
// show where they belong this way.
import { useState } from "react";
import { DEMO } from "../demo";

export function Favicon({ domain, size = 28 }: { domain?: string | null; size?: number }) {
  const [failed, setFailed] = useState(false);
  const d = (domain ?? "").trim().toLowerCase();
  const letter = (d.replace(/^www\./, "")[0] ?? "?").toUpperCase();
  const style = { width: size, height: size, fontSize: Math.round(size * 0.46) };
  if (!d || failed || DEMO)
    return (
      <span className="fav letter" style={style} aria-hidden>
        {letter}
      </span>
    );
  return (
    <span className="fav" style={style} aria-hidden>
      <img src={`/api/favicon/${encodeURIComponent(d)}`} alt="" loading="lazy" onError={() => setFailed(true)} />
    </span>
  );
}
