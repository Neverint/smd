import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Link } from "react-router";
import { Segmented } from "../../components";
import { call, type AppInfo } from "../../lib/backend";
import { useSettings, type LanguagePref, type ThemePref } from "../../stores/settings";

export function SettingsPage() {
  const { t } = useTranslation();
  const { theme, language, setTheme, setLanguage } = useSettings();
  const [info, setInfo] = useState<AppInfo | null>(null);

  useEffect(() => {
    call("app_info")
      .then(setInfo)
      .catch(() => setInfo(null));
  }, []);

  return (
    <div className="settings">
      <h1>{t("modules.settings.name")}</h1>

      <div className="smd-setting">
        <div>
          <div className="smd-setting__label" id="lang-label">
            {t("settings.language")}
          </div>
          <div className="smd-setting__hint">{t("settings.languageHint")}</div>
        </div>
        <Segmented<LanguagePref>
          label={t("settings.language")}
          value={language}
          onChange={setLanguage}
          options={[
            { value: "system", label: t("settings.system") },
            { value: "en", label: "English" },
            { value: "fr", label: "Français" },
          ]}
        />
      </div>

      <div className="smd-setting">
        <div className="smd-setting__label">{t("settings.theme")}</div>
        <Segmented<ThemePref>
          label={t("settings.theme")}
          value={theme}
          onChange={setTheme}
          options={[
            { value: "system", label: t("settings.system") },
            { value: "light", label: t("settings.light") },
            { value: "dark", label: t("settings.dark") },
          ]}
        />
      </div>

      <h2>{t("settings.about")}</h2>
      <div className="smd-setting">
        <div>
          <div className="smd-setting__label">{t("app.name")}</div>
          {info && (
            <div className="smd-setting__hint">
              {t("settings.version", { version: info.version })} ·{" "}
              {t("settings.windowEffect", { effect: t(`settings.effect.${info.windowEffect}`) })}
            </div>
          )}
          <div className="smd-setting__hint">{t("settings.privacy")}</div>
        </div>
        {import.meta.env.DEV && <Link to="/dev/gallery">{t("settings.gallery")}</Link>}
      </div>
    </div>
  );
}
