import { getCurrentWindow } from "@tauri-apps/api/window";
import { useTranslation } from "react-i18next";
import { isTauri } from "../lib/backend";
import { AppMark } from "./AppMark";

/** Segoe Fluent Icons code points for the caption buttons (same glyphs as native Windows 11). */
const GLYPH = { minimize: "", maximize: "", close: "" };

export function TitleBar() {
  const { t } = useTranslation();
  const inTauri = isTauri();
  const win = inTauri ? getCurrentWindow() : null;

  return (
    <div className="titlebar" data-tauri-drag-region>
      <div className="titlebar__brand" data-tauri-drag-region>
        <AppMark size={16} />
        <span data-tauri-drag-region>{t("app.name")}</span>
      </div>
      {win && (
        <div className="titlebar__caption">
          <button type="button" aria-label={t("titlebar.minimize")} title={t("titlebar.minimize")} onClick={() => void win.minimize()}>
            {GLYPH.minimize}
          </button>
          <button
            type="button"
            aria-label={t("titlebar.maximize")}
            title={t("titlebar.maximize")}
            onClick={() => void win.toggleMaximize()}
          >
            {GLYPH.maximize}
          </button>
          <button
            type="button"
            className="titlebar__close"
            aria-label={t("titlebar.close")}
            title={t("titlebar.close")}
            onClick={() => void win.close()}
          >
            {GLYPH.close}
          </button>
        </div>
      )}
    </div>
  );
}
