import { useEffect, useRef, useState, type ReactNode } from "react";
import { Icons } from "../ui/icons";

export function Switch({ checked, onChange, label, disabled }: { checked: boolean; onChange: (value: boolean) => void; label: string; disabled?: boolean }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      className={`switch${checked ? " on" : ""}`}
      onClick={() => onChange(!checked)}
    >
      <span />
    </button>
  );
}

export function Segmented<T extends string>({ value, options, onChange, label }: { value: T; options: { value: T; label: string; icon?: ReactNode }[]; onChange: (value: T) => void; label: string }) {
  return (
    <div className="segmented" role="radiogroup" aria-label={label}>
      {options.map((option) => (
        <button
          type="button"
          role="radio"
          aria-checked={value === option.value}
          key={option.value}
          className={value === option.value ? "on" : ""}
          onClick={() => onChange(option.value)}
        >
          {option.icon}
          {option.label}
        </button>
      ))}
    </div>
  );
}

export function Select({ value, onChange, options, label, disabled }: { value: string; onChange: (value: string) => void; options: { value: string; label: string }[]; label: string; disabled?: boolean }) {
  return (
    <span className="select">
      <select value={value} aria-label={label} disabled={disabled} onChange={(event) => onChange(event.target.value)}>
        {options.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
    </span>
  );
}

/** A settings row: title and help on the left, the control on the right. */
export function Row({ title, help, children, tone }: { title: ReactNode; help?: ReactNode; children?: ReactNode; tone?: "danger" }) {
  return (
    <div className={`setting-row${tone ? ` ${tone}` : ""}`}>
      <div className="setting-text">
        <div className="setting-title">{title}</div>
        {help && <div className="setting-help">{help}</div>}
      </div>
      {children && <div className="setting-control">{children}</div>}
    </div>
  );
}

/** Text input that only reports on blur or Enter, so typing does not save every keystroke. */
export function CommitInput({ value, onCommit, placeholder, type = "text", label, width, secret }: { value: string; onCommit: (value: string) => void; placeholder?: string; type?: string; label: string; width?: number; secret?: boolean }) {
  const [draft, setDraft] = useState(value);
  const [shown, setShown] = useState(false);
  const reverting = useRef(false);
  useEffect(() => setDraft(value), [value]);
  const commit = () => {
    if (reverting.current) {
      reverting.current = false;
      return;
    }
    if (draft !== value) onCommit(draft);
  };
  return (
    <span className="commit-input" style={width ? { width } : undefined}>
      <input
        className="input"
        aria-label={label}
        type={secret && !shown ? "password" : type}
        value={draft}
        placeholder={placeholder}
        onChange={(event) => setDraft(event.target.value)}
        onBlur={commit}
        onKeyDown={(event) => {
          if (event.key === "Enter") (event.target as HTMLInputElement).blur();
          if (event.key === "Escape") {
            reverting.current = true;
            setDraft(value);
            (event.target as HTMLInputElement).blur();
          }
        }}
      />
      {secret && (
        <button type="button" className="input-addon" aria-pressed={shown} aria-label={label} onClick={() => setShown((item) => !item)}>
          {shown ? <Icons.eyeOff /> : <Icons.eye />}
        </button>
      )}
    </span>
  );
}
