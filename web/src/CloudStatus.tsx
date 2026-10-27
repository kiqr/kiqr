// kiqr.cloud in the sidebar, next to Settings: signed in (and as whom), the
// code to enter while a device sign-in waits, or a button to sign in.
import { useState } from "react";
import { NavLink, useNavigate } from "react-router-dom";
import { Cloud, Loader2, LogIn } from "lucide-react";
import { api } from "./api";
import { useSignIns } from "./pages/SignIns";
import "./cloudstatus.css";

export function CloudStatus() {
  const data = useSignIns();
  const navigate = useNavigate();
  const [busy, setBusy] = useState(false);
  const [opened, setOpened] = useState(false);
  const cloud = data.data?.signins.find((s) => s.id === "kiqr-cloud");
  if (!cloud) return null;

  if (cloud.signed_in)
    return (
      <NavLink to="/settings" className="cloud-status on" title="kiqr.cloud connects this KIQR agent with your apps">
        <Cloud size={16} />
        <span className="txt">
          <span>kiqr.cloud</span>
          <small>{cloud.account ?? "signed in"}</small>
        </span>
        <b className="dot" aria-label="signed in" />
      </NavLink>
    );

  if (cloud.waiting)
    return (
      <a className="cloud-status" href={cloud.waiting.verification_uri_complete ?? cloud.waiting.verification_uri} target="_blank" rel="noreferrer">
        <Cloud size={16} />
        <span className="txt">
          <span>Enter {cloud.waiting.user_code.replace(/^(.{4})(.{4})$/, "$1-$2")}</span>
          <small>at {cloud.waiting.verification_uri.replace(/^https?:\/\//, "")}</small>
        </span>
      </a>
    );

  const start = async () => {
    setBusy(true);
    try {
      const r = await api.startSignIn("kiqr-cloud", window.location.origin);
      if (r.mode === "redirect") {
        window.open(r.url, "_blank", "noopener");
        setOpened(true);
      }
      data.reload();
    } catch {
      // The Settings card says what went wrong.
      navigate("/settings");
    } finally {
      setBusy(false);
    }
  };
  return (
    <button className="cloud-status signin" disabled={busy} onClick={() => void start()} title="Connect this KIQR agent with your apps through kiqr.cloud">
      {busy ? <Loader2 size={16} className="spin" /> : opened ? <Cloud size={16} /> : <LogIn size={16} />}
      <span className="txt">
        <span>{opened ? "Approve in the new tab" : "Sign in to kiqr.cloud"}</span>
        <small>Connect your apps</small>
      </span>
    </button>
  );
}
