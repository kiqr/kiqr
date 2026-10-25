// The on/off switch used across settings.
import { motion } from "motion/react";

export function Switch({ on, onChange, disabled }: { on: boolean; onChange: (on: boolean) => void; disabled?: boolean }) {
  return (
    <button type="button" role="switch" aria-checked={on} disabled={disabled} className={`switch ${on ? "on" : ""}`} onClick={() => onChange(!on)}>
      <motion.i layout transition={{ type: "spring", stiffness: 600, damping: 35 }} />
    </button>
  );
}
