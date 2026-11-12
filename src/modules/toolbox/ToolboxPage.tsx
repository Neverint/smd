import { ArrowDownload20Regular, ArrowSync20Regular, Globe16Regular, Open20Regular } from "@fluentui/react-icons";
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { moduleById } from "../../app/modules";
import { Button, ErrorNotice, ModuleHero } from "../../components";
import { call, toBackendError, type BackendError } from "../../lib/backend";
import type { Tool, ToolboxView, WingetResult } from "../../lib/types";
import { useFormat } from "../../lib/useFormat";
import "../modules.css";

const CATEGORIES = ["apps", "files", "hardware", "system", "privacy", "security"];

function ToolCard({ tool, busy, onInstall, onOpen }: { tool: Tool; busy: string | null; onInstall: (update: boolean) => void; onOpen: () => void }) {
  const { t, i18n } = useTranslation();
  const lang = i18n.resolvedLanguage?.startsWith("fr") ? "fr" : "en";
  const working = busy === tool.id;
  return (
    <li className="tool-card toolbox-card">
      <div className="toolbox-card__head">
        <h3 className="row__title">{tool.name}</h3>
        {tool.installed ? <span className="tag tag--good">{tool.version ? t("toolbox.installedVersion", { version: tool.version }) : t("toolbox.installed")}</span> : null}
      </div>
      <p className="row__sub">{tool.description[lang]}</p>
      <p className="row__sub">
        {t("toolbox.license", { license: tool.license })}
        {tool.openSource ? ` · ${t("toolbox.openSource")}` : ""} · <code>{tool.wingetId}</code>
      </p>
      <div className="toolbar">
        {!tool.installed && (
          <Button variant="primary" icon={<ArrowDownload20Regular />} disabled={busy !== null} onClick={() => onInstall(false)} aria-label={t("toolbox.installLabel", { name: tool.name })}>
            {working ? t("toolbox.installing") : t("toolbox.install")}
          </Button>
        )}
        {tool.installed && (
          <Button icon={<ArrowSync20Regular />} disabled={busy !== null} onClick={() => onInstall(true)} aria-label={t("toolbox.updateLabel", { name: tool.name })}>
            {working ? t("toolbox.updating") : t("toolbox.update")}
          </Button>
        )}
        {tool.installed && tool.canOpen && (
          <Button variant="subtle" icon={<Open20Regular />} disabled={working} onClick={onOpen} aria-label={t("toolbox.openLabel", { name: tool.name })}>
            {t("toolbox.open")}
          </Button>
        )}
      </div>
    </li>
  );
}

export function ToolboxPage() {
  const { t } = useTranslation();
  const f = useFormat();
  const def = moduleById("toolbox");
  const [view, setView] = useState<ToolboxView | null>(null);
  const [error, setError] = useState<BackendError | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(true);
  const [last, setLast] = useState<{ name: string; update: boolean; result: WingetResult } | null>(null);

  const load = useCallback((refreshNow: boolean) => {
    call("toolbox_list", { refreshNow })
      .then((v) => {
        setView(v);
        setError(null);
      })
      .catch((e) => setError(toBackendError(e)))
      .finally(() => setRefreshing(false));
  }, []);
  useEffect(() => load(false), [load]);

  const install = async (tool: Tool, update: boolean) => {
    setBusy(tool.id);
    setLast(null);
    try {
      const r = await call("toolbox_install", { id: tool.id, update });
      setView(r.view);
      setLast({ name: tool.name, update, result: r.result });
      setError(null);
    } catch (e) {
      setError(toBackendError(e));
    } finally {
      setBusy(null);
    }
  };

  const open = (tool: Tool) => void call("toolbox_open", { id: tool.id }).catch((e) => setError(toBackendError(e)));

  return (
    <div className="tool">
      <ModuleHero icon={def.icon} title={t("modules.toolbox.name")} tagline={t("modules.toolbox.tagline")} />
      <p className="cat-row__note">
        <Globe16Regular aria-hidden="true" /> {t("toolbox.network")}
      </p>
      <div className="toolbar">
        <Button variant="subtle" icon={<ArrowSync20Regular />} disabled={refreshing || busy !== null} onClick={() => {
            setRefreshing(true);
            load(true);
          }}
        >
          {refreshing ? t("toolbox.checking") : t("toolbox.check")}
        </Button>
        {view && view.checkedMs > 0 && <span className="tool-meta">{t("toolbox.checked", { date: f.dateTime(view.checkedMs) })}</span>}
      </div>
      {error && <ErrorNotice error={error} />}
      {view?.wingetError && <p className="setting-warning">{t("toolbox.wingetMissing")}</p>}
      {last && (
        <p className={last.result.ok ? "activity-message" : "setting-warning"} role="status">
          {last.result.ok ? t(last.update ? "toolbox.updated" : "toolbox.done", { name: last.name }) : t("toolbox.failed", { name: last.name, code: last.result.code })}
          {!last.result.ok && last.result.output && <span className="row__sub row__mono">{last.result.output}</span>}
        </p>
      )}
      {!view ? (
        !error && <p className="flow-muted">{t("toolbox.loading")}</p>
      ) : (
        CATEGORIES.map((cat) => {
          const tools = view.tools.filter((x) => x.category === cat);
          if (tools.length === 0) return null;
          return (
            <section key={cat} className="tool-section" aria-labelledby={`tb-${cat}`}>
              <h2 id={`tb-${cat}`} className="section-title">
                {t(`toolbox.category.${cat}`)}
              </h2>
              <ul className="toolbox-grid">
                {tools.map((tool) => (
                  <ToolCard key={tool.id} tool={tool} busy={busy} onInstall={(u) => void install(tool, u)} onOpen={() => open(tool)} />
                ))}
              </ul>
            </section>
          );
        })
      )}
      <p className="inline-note">{t("toolbox.agreement")}</p>
      <div style={{ height: 32 }} />
    </div>
  );
}
