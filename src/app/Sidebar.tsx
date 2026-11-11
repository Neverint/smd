import { Navigation20Regular } from "@fluentui/react-icons";
import { NavLink } from "react-router";
import { useTranslation } from "react-i18next";
import { Badge } from "../components";
import { useSettings } from "../stores/settings";
import { MODULES, type ModuleDef, type ModuleId } from "./modules";

/** Pending-item counts per module. Filled by scans from Phase 1 on. */
const PENDING: Partial<Record<ModuleId, number>> = {};

function NavItem({ def, collapsed }: { def: ModuleDef; collapsed: boolean }) {
  const { t } = useTranslation();
  const Icon = def.icon;
  const name = t(`modules.${def.id}.name`);
  const pending = PENDING[def.id] ?? 0;
  return (
    <li>
      <NavLink
        to={def.path}
        end
        className="nav-item"
        data-module={def.accent ? def.id : undefined}
        title={collapsed ? name : undefined}
        aria-label={collapsed ? name : undefined}
      >
        <span className="nav-item__icon" aria-hidden="true">
          <Icon />
        </span>
        {!collapsed && <span className="nav-item__label">{name}</span>}
        {pending > 0 && (
          <span className="nav-item__badge">
            <Badge label={t("nav.pending", { count: pending })}>{pending}</Badge>
          </span>
        )}
      </NavLink>
    </li>
  );
}

export function Sidebar() {
  const { t } = useTranslation();
  const collapsed = useSettings((s) => s.sidebarCollapsed);
  const toggle = useSettings((s) => s.toggleSidebar);

  return (
    <nav className={`sidebar${collapsed ? " sidebar--collapsed" : ""}`} aria-label={t("nav.label")}>
      <button
        type="button"
        className="nav-item nav-toggle"
        onClick={toggle}
        aria-expanded={!collapsed}
        aria-label={collapsed ? t("nav.expand") : t("nav.collapse")}
        title={collapsed ? t("nav.expand") : t("nav.collapse")}
      >
        <span className="nav-item__icon" aria-hidden="true">
          <Navigation20Regular />
        </span>
      </button>
      <ul className="sidebar__list">
        {MODULES.filter((m) => m.group === "main").map((def) => (
          <NavItem key={def.id} def={def} collapsed={collapsed} />
        ))}
      </ul>
      <ul className="sidebar__list sidebar__list--footer">
        {MODULES.filter((m) => m.group === "footer").map((def) => (
          <NavItem key={def.id} def={def} collapsed={collapsed} />
        ))}
      </ul>
    </nav>
  );
}
