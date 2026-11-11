import { useTranslation } from "react-i18next";
import { moduleById } from "../../app/modules";
import { ModuleHero, ScanButton } from "../../components";

export function SmartCarePage() {
  const { t } = useTranslation();
  const def = moduleById("smart-care");
  return (
    <div className="page">
      <div className="page__body">
        <ModuleHero icon={def.icon} title={t("modules.smart-care.name")} tagline={t("modules.smart-care.tagline")} />
      </div>
      <div className="page__action">
        <ScanButton disabled>{t("common.scan")}</ScanButton>
        <p className="page__note">{t("common.plannedFor", { phase: def.phase })}</p>
      </div>
    </div>
  );
}
