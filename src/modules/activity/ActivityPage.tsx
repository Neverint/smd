import {
  AppsListDetail20Regular,
  ArrowClockwise20Regular,
  ArrowDownload20Regular,
  ArrowSync20Regular,
  ArrowUndo20Regular,
  Broom20Regular,
  DataPie20Regular,
  Delete20Regular,
  DocumentCopy20Regular,
  FolderArrowRight20Regular,
  LockClosed20Regular,
  ShieldCheckmark20Regular,
  Search20Regular,
  Sparkle20Regular,
  TopSpeed20Regular,
} from "@fluentui/react-icons";
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button, ErrorNotice } from "../../components";
import { call, toBackendError, type BackendError } from "../../lib/backend";
import type { ActivityEntry, ActivityKind } from "../../lib/types";
import { useFormat } from "../../lib/useFormat";
import "../modules.css";

const ICONS: Record<ActivityKind, typeof Broom20Regular> = {
  smartCareScan: Sparkle20Regular,
  cleanupScan: Search20Regular,
  cleanup: Broom20Regular,
  emptyRecycleBin: Delete20Regular,
  spaceLensScan: DataPie20Regular,
  restore: ArrowUndo20Regular,
  startupChange: TopSpeed20Regular,
  uninstall: AppsListDetail20Regular,
  leftoversRemoved: FolderArrowRight20Regular,
  appUpdate: ArrowSync20Regular,
  updatesSetup: ArrowDownload20Regular,
  clutterRemoved: DocumentCopy20Regular,
  defenderScan: ShieldCheckmark20Regular,
  signatureUpdate: ArrowDownload20Regular,
  permissionChange: LockClosed20Regular,
};

function Detail({ e }: { e: ActivityEntry }) {
  const { t } = useTranslation();
  const f = useFormat();
  const skipped = e.failed > 0 ? ` · ${t("activity.detail.skipped", { count: e.failed })}` : "";
  switch (e.kind) {
    case "smartCareScan":
      return <>{t("activity.detail.smartCareScan", { score: e.score ?? "—", size: f.bytes(e.bytes) })}</>;
    case "cleanupScan":
      return <>{t("activity.detail.cleanupScan", { size: f.bytes(e.bytes) })}</>;
    case "cleanup":
      return (
        <>
          {t(e.mode === "permanent" ? "activity.detail.cleanupPermanent" : "activity.detail.cleanupRecycle", { size: f.bytes(e.bytes) })} · {t("common.items", { count: e.items })}
          {skipped}
        </>
      );
    case "emptyRecycleBin":
      return <>{t("activity.detail.emptyRecycleBin", { size: f.bytes(e.bytes) })}</>;
    case "spaceLensScan":
      return <>{t("activity.detail.spaceLensScan", { target: e.target ?? "", size: f.bytes(e.bytes) })}</>;
    case "restore":
      return (
        <>
          {t("activity.detail.restore", { count: e.items })}
          {skipped}
        </>
      );
    case "startupChange":
      return <>{t(e.enabled ? "activity.detail.startupOn" : "activity.detail.startupOff", { name: e.target ?? "" })}</>;
    case "uninstall":
      return <>{t(e.failed > 0 ? "activity.detail.uninstallFailed" : "activity.detail.uninstall", { name: e.target ?? "" })}</>;
    case "leftoversRemoved":
      return (
        <>
          {t("activity.detail.leftovers", { name: e.target ?? "", size: f.bytes(e.bytes), count: e.items })}
          {skipped}
        </>
      );
    case "appUpdate":
      return <>{t(e.failed > 0 ? "activity.detail.updateFailed" : "activity.detail.update", { name: e.target ?? "" })}</>;
    case "updatesSetup":
      return <>{t(e.failed > 0 ? "activity.detail.setupFailed" : "activity.detail.setup")}</>;
    case "defenderScan":
      return <>{t(e.failed > 0 ? "activity.detail.scanStopped" : e.items > 0 ? "activity.detail.scanFound" : "activity.detail.scanClean", { kind: t(`protection.scanKind.${e.target ?? "quick"}`) })}</>;
    case "signatureUpdate":
      return <>{t(e.failed > 0 ? "activity.detail.signaturesFailed" : "activity.detail.signatures")}</>;
    case "permissionChange": {
      const [cap, app] = (e.target ?? "|").split("|");
      return <>{t(e.enabled ? "activity.detail.permissionOn" : "activity.detail.permissionOff", { app, what: t(`protection.capability.${cap}`) })}</>;
    }
    case "clutterRemoved":
      return (
        <>
          {t("activity.detail.clutter", { size: f.bytes(e.bytes), count: e.items })}
          {skipped}
        </>
      );
  }
}

export function ActivityPage() {
  const { t } = useTranslation();
  const f = useFormat();
  const [entries, setEntries] = useState<ActivityEntry[] | null>(null);
  const [error, setError] = useState<BackendError | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [busy, setBusy] = useState<number | null>(null);

  const load = useCallback(() => {
    call("activity_list")
      .then((list) => {
        setEntries(list);
        setError(null);
      })
      .catch((e) => setError(toBackendError(e)));
  }, []);

  useEffect(load, [load]);

  const undo = async (id: number) => {
    setBusy(id);
    setMessage(null);
    try {
      const r = await call("activity_undo", { id });
      const parts = [t("activity.restored", { count: r.restored })];
      if (r.missing > 0) parts.push(t("activity.missing", { count: r.missing }));
      if (r.failed > 0) parts.push(t("activity.restoreFailed", { count: r.failed }));
      setMessage(parts.join(" "));
      load();
    } catch (e) {
      setError(toBackendError(e));
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="activity">
      <div className="activity__head">
        <h1>{t("modules.activity.name")}</h1>
        <Button variant="subtle" icon={<ArrowClockwise20Regular />} onClick={load}>
          {t("activity.refresh")}
        </Button>
      </div>
      {error && <ErrorNotice error={error} />}
      {message && (
        <p className="activity-message" role="status">
          {message}
        </p>
      )}
      {entries && entries.length === 0 && <p className="flow-muted">{t("activity.empty")}</p>}
      {entries && entries.length > 0 && (
        <ul className="activity-list">
          {entries.map((e) => {
            const Icon = ICONS[e.kind];
            return (
              <li key={e.id} className="activity-row">
                <span className="activity-row__icon" aria-hidden="true">
                  <Icon />
                </span>
                <div className="activity-row__main">
                  <span className="activity-row__title">{t(`activity.kind.${e.kind}`)}</span>
                  <span className="activity-row__detail">
                    <Detail e={e} />
                  </span>
                </div>
                <span className="activity-row__time">{f.dateTime(e.timeMs)}</span>
                {e.undo === "available" && (
                  <Button icon={<ArrowUndo20Regular />} disabled={busy !== null} onClick={() => void undo(e.id)} aria-label={t("activity.undoLabel", { what: t(`activity.kind.${e.kind}`) })}>
                    {t("activity.undo")}
                  </Button>
                )}
                {e.undo === "done" && <span className="tag">{t("activity.undone")}</span>}
              </li>
            );
          })}
        </ul>
      )}
    </div>
  );
}
