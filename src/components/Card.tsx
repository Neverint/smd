import type { ReactNode } from "react";
import type { ModuleId } from "../app/modules";

export interface CardProps {
  title: string;
  /** Big measured number, e.g. "12.4 GB". */
  value?: string;
  description?: string;
  icon?: ReactNode;
  /** Colors the icon tile with this module's accent. */
  module?: ModuleId;
  actions?: ReactNode;
  children?: ReactNode;
}

export function Card({ title, value, description, icon, module, actions, children }: CardProps) {
  return (
    <section className="smd-card" data-module={module} aria-label={title}>
      <div className="smd-card__head">
        {icon && (
          <div className="smd-card__icon" aria-hidden="true">
            {icon}
          </div>
        )}
        <div>
          <h3 className="smd-card__title">{title}</h3>
          {value && <div className="smd-card__value">{value}</div>}
          {description && <p className="smd-card__desc">{description}</p>}
        </div>
      </div>
      {children}
      {actions && <div className="smd-card__actions">{actions}</div>}
    </section>
  );
}
