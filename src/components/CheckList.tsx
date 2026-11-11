import { useEffect, useId, useRef } from "react";
import { useTranslation } from "react-i18next";

export interface CheckItem {
  id: string;
  label: string;
  description?: string;
  /** Pre-formatted size, e.g. "1.2 GB". */
  size?: string;
}

export interface CheckListProps {
  items: CheckItem[];
  selected: ReadonlySet<string>;
  onChange: (next: Set<string>) => void;
  /** Accessible name of the list. */
  label: string;
}

/** Checkable review list with a "Select all" row (indeterminate when partly selected). */
export function CheckList({ items, selected, onChange, label }: CheckListProps) {
  const { t } = useTranslation();
  const baseId = useId();
  const allRef = useRef<HTMLInputElement>(null);
  const count = items.filter((i) => selected.has(i.id)).length;
  const all = count === items.length && items.length > 0;

  useEffect(() => {
    if (allRef.current) allRef.current.indeterminate = count > 0 && !all;
  }, [count, all]);

  const toggle = (id: string) => {
    const next = new Set(selected);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    onChange(next);
  };

  return (
    <div role="group" aria-label={label}>
      <label className="smd-check smd-checklist__all">
        <input
          ref={allRef}
          type="checkbox"
          checked={all}
          onChange={() => onChange(all ? new Set() : new Set(items.map((i) => i.id)))}
        />
        <span className="smd-check__text">{t("common.selectAll")}</span>
      </label>
      <ul className="smd-checklist">
        {items.map((item) => {
          const descId = item.description ? `${baseId}-${item.id}-desc` : undefined;
          return (
            <li key={item.id}>
              <label className="smd-check">
                <input type="checkbox" checked={selected.has(item.id)} onChange={() => toggle(item.id)} aria-describedby={descId} />
                <span className="smd-check__text">
                  <span>{item.label}</span>
                  {item.description && (
                    <span id={descId} className="smd-check__desc" style={{ display: "block" }}>
                      {item.description}
                    </span>
                  )}
                </span>
                {item.size && <span className="smd-check__size">{item.size}</span>}
              </label>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
