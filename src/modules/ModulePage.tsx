import { useTranslation } from "react-i18next";
import { moduleById, type ModuleId } from "../app/modules";
import { Button, ModuleHero } from "../components";

/** Placeholder page for modules that later phases will build: hero, one-line explanation, primary action. */
export function ModulePage({ id }: { id: ModuleId }) {
  const { t } = useTranslation();
  const def = moduleById(id);
  return (
    <div className="page">
      <div className="page__body">
        <ModuleHero icon={def.icon} title={t(`modules.${id}.name`)} tagline={t(`modules.${id}.tagline`)} />
      </div>
      <div className="page__action">
        <Button variant="primary" size="lg" disabled>
          {t("common.scan")}
        </Button>
        <p className="page__note">{t("common.plannedFor", { phase: def.phase })}</p>
      </div>
    </div>
  );
}
