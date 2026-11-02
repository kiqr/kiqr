// The on/off switch used across settings.
import { motion } from "motion/react";

export function Switch({ on, onChange, disabled, label }: { on: boolean; onChange: (on: boolean) => void; disabled?: boolean; label?: string }) {
  return (
    <button type="button" role="switch" aria-checked={on} aria-label={label} title={label} disabled={disabled} className={`switch ${on ? "on" : ""}`} onClick={() => onChange(!on)}>
      <motion.i layout transition={{ type: "spring", stiffness: 600, damping: 35 }} />
    </button>
  );
}
