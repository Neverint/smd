import { useId, useRef, type KeyboardEvent, type ReactNode } from "react";

export interface TabDef<T extends string> {
  id: T;
  label: string;
  /** Small count shown after the label. */
  count?: number;
}

export interface TabsProps<T extends string> {
  label: string;
  tabs: TabDef<T>[];
  value: T;
  onChange: (id: T) => void;
  children: ReactNode;
}

/** Tab list (ARIA tabs pattern: arrow keys, Home/End) with one panel for the selected tab. */
export function Tabs<T extends string>({ label, tabs, value, onChange, children }: TabsProps<T>) {
  const base = useId();
  const refs = useRef<(HTMLButtonElement | null)[]>([]);
  const index = Math.max(0, tabs.findIndex((t) => t.id === value));

  const onKeyDown = (e: KeyboardEvent) => {
    let next = -1;
    if (e.key === "ArrowRight") next = (index + 1) % tabs.length;
    else if (e.key === "ArrowLeft") next = (index - 1 + tabs.length) % tabs.length;
    else if (e.key === "Home") next = 0;
    else if (e.key === "End") next = tabs.length - 1;
    if (next < 0) return;
    e.preventDefault();
    onChange(tabs[next].id);
    refs.current[next]?.focus();
  };

  return (
    <div className="smd-tabs">
      <div className="smd-tabs__list" role="tablist" aria-label={label} onKeyDown={onKeyDown}>
        {tabs.map((t, i) => {
          const selected = t.id === value;
          return (
            <button
              key={t.id}
              ref={(el) => {
                refs.current[i] = el;
              }}
              type="button"
              role="tab"
              id={`${base}-tab-${t.id}`}
              aria-selected={selected}
              aria-controls={`${base}-panel`}
              tabIndex={selected ? 0 : -1}
              className="smd-tabs__tab"
              onClick={() => onChange(t.id)}
            >
              {t.label}
              {t.count !== undefined && <span className="smd-tabs__count">{t.count}</span>}
            </button>
          );
        })}
      </div>
      <div className="smd-tabs__panel" role="tabpanel" id={`${base}-panel`} aria-labelledby={`${base}-tab-${value}`}>
        {children}
      </div>
    </div>
  );
}
