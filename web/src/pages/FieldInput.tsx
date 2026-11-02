import { useId } from "react";

// One configuration field: a select when it has a fixed set of values (a
// key's `options`, a target field's `enum`), free text otherwise. Every form
// that sets up an ability, an account or a channel goes through it, so a
// choice is never typed by hand.
type Props = {
  value: string;
  onChange: (v: string) => void;
  options?: string[];
  /** What to call each option, when not the value itself (a property's name). */
  labels?: Record<string, string>;
  placeholder?: string;
  /** Secret text (a password, a key): masked. Ignored for a choice. */
  secret?: boolean;
  mono?: boolean;
  /** An empty choice is allowed (the ability's own default). */
  optional?: boolean;
  onEnter?: () => void;
  ariaLabel?: string;
  /** A long list read from a service (every repository): typed to search, and any value may be typed. */
  search?: boolean;
  /** Show the labels alone, never the value beside them (an account's id). */
  plain?: boolean;
};

export function FieldInput({ value, onChange, options, labels, placeholder, secret, mono, optional, onEnter, ariaLabel, search, plain }: Props) {
  const listId = useId();
  if (search && options && options.length > 0)
    return (
      <>
        <input
          className={`input ${mono ? "mono" : ""}`}
          type="text"
          autoComplete="off"
          spellCheck={false}
          placeholder={`Search ${options.length}, or type it`}
          list={listId}
          value={value}
          aria-label={ariaLabel}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && onEnter?.()}
        />
        <datalist id={listId}>
          {options.map((o) => (
            <option key={o} value={o}>
              {labels?.[o] && labels[o] !== o ? labels[o] : undefined}
            </option>
          ))}
        </datalist>
      </>
    );
  if (options && options.length > 0)
    return (
      <select className={`input ${mono ? "mono" : ""}`} value={value} aria-label={ariaLabel} onChange={(e) => onChange(e.target.value)}>
        <option value="" disabled={!optional}>
          {optional ? "Default" : "Choose…"}
        </option>
        {options.map((o) => (
          <option key={o} value={o}>
            {labels?.[o] && labels[o] !== o ? (plain ? labels[o] : `${labels[o]} (${o})`) : o}
          </option>
        ))}
      </select>
    );
  return (
    <input
      className={`input ${mono ? "mono" : ""}`}
      type={secret ? "password" : "text"}
      autoComplete="off"
      spellCheck={false}
      placeholder={placeholder}
      value={value}
      aria-label={ariaLabel}
      onChange={(e) => onChange(e.target.value)}
      onKeyDown={(e) => e.key === "Enter" && onEnter?.()}
    />
  );
}

/** A destination field's values, when the kind lists them. */
export function enumOf(spec: unknown): string[] | undefined {
  const e = (spec as { enum?: unknown })?.enum;
  return Array.isArray(e) ? e.map(String) : undefined;
}
