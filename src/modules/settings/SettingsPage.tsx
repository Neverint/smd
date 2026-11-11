import { Delete20Regular, FolderOpen20Regular } from "@fluentui/react-icons";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Link } from "react-router";
import { Button, ErrorNotice, IconButton, Segmented } from "../../components";
import { call, toBackendError, type AppInfo, type BackendError } from "../../lib/backend";
import type { BackendSettings, DeleteMode } from "../../lib/types";
import { useSettings, type LanguagePref, type ThemePref } from "../../stores/settings";
import "../modules.css";

function CleanupSettings() {
  const { t } = useTranslation();
  const [s, setS] = useState<BackendSettings | null>(null);
  const [error, setError] = useState<BackendError | null>(null);

  useEffect(() => {
    call("settings_get")
      .then(setS)
      .catch((e) => setError(toBackendError(e)));
  }, []);

  const save = (next: BackendSettings) => {
    call("settings_set", { settings: next })
      .then(setS)
      .catch((e) => setError(toBackendError(e)));
  };

  if (!s) return error ? <ErrorNotice error={error} /> : null;
  return (
    <>
      <h2>{t("settings.cleanup")}</h2>
      {error && <ErrorNotice error={error} />}
      <div className="smd-setting smd-setting--stack">
        <div className="smd-setting" style={{ border: "none", padding: 0, background: "none" }}>
          <div>
            <div className="smd-setting__label">{t("settings.deleteMode")}</div>
            <div className="smd-setting__hint">{t("settings.deleteModeHint")}</div>
          </div>
          <Segmented<DeleteMode>
            label={t("settings.deleteMode")}
            value={s.deleteMode}
            onChange={(deleteMode) => save({ ...s, deleteMode })}
            options={[
              { value: "recycleBin", label: t("settings.recycleBin") },
              { value: "permanent", label: t("settings.permanent") },
            ]}
          />
        </div>
        {s.deleteMode === "permanent" && (
          <p className="setting-warning" role="status">
            {t("settings.permanentWarning")}
          </p>
        )}
      </div>
      <div className="smd-setting smd-setting--stack">
        <div>
          <div className="smd-setting__label">{t("settings.exclusions")}</div>
          <div className="smd-setting__hint">{t("settings.exclusionsHint")}</div>
        </div>
        {s.exclusions.length === 0 ? (
          <div className="smd-setting__hint">{t("settings.noExclusions")}</div>
        ) : (
          <ul className="exclusion-list">
            {s.exclusions.map((p) => (
              <li key={p}>
                <code>{p}</code>
                <IconButton label={t("settings.removeLabel", { path: p })} icon={<Delete20Regular />} onClick={() => save({ ...s, exclusions: s.exclusions.filter((x) => x !== p) })} />
              </li>
            ))}
          </ul>
        )}
      </div>
      <div className="smd-setting">
        <div>
          <div className="smd-setting__label">{t("settings.logs")}</div>
          <div className="smd-setting__hint">{t("settings.logsHint")}</div>
        </div>
        <Button icon={<FolderOpen20Regular />} onClick={() => void call("open_log_folder").catch((e) => setError(toBackendError(e)))}>
          {t("settings.openLogs")}
        </Button>
      </div>
    </>
  );
}

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

      <CleanupSettings />

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
