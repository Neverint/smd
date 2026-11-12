import { CheckmarkCircle48Regular } from "@fluentui/react-icons";
import { motion } from "motion/react";
import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Link } from "react-router";
import { ProgressRing } from "../../components";
import type { CleanupReport, ScanProgress } from "../../lib/types";
import { useFormat } from "../../lib/useFormat";

/** Human text for a scan step (a rule id or a Smart Care check). */
export function useStepLabel() {
  const { t } = useTranslation();
  return (step: string | undefined): string => {
    if (!step) return "";
    if (["disks", "memory", "drive", "startup", "updates"].includes(step)) return t(`smartCare.step.${step}`);
    return t("smartCare.step.rule", { name: t(`cleanup.cat.${step}.name`) });
  };
}

export interface ScanningProps {
  title: string;
  progress: ScanProgress | null;
  /** Override the step line (Space Lens counters). */
  detail?: ReactNode;
  children?: ReactNode;
}

/** Ring progress with the current step under it. */
export function Scanning({ title, progress, detail, children }: ScanningProps) {
  const stepLabel = useStepLabel();
  const pct = progress && progress.total > 0 ? (progress.done / progress.total) * 100 : undefined;
  const step = detail ?? stepLabel(progress?.step);
  return (
    <div className="flow-center" aria-live="polite">
      <ProgressRing value={pct} size={176} thickness={12} label={title}>
        {pct !== undefined && <span className="flow-ring-value">{Math.round(pct)} %</span>}
      </ProgressRing>
      <h1 className="flow-title">{title}</h1>
      <p className="flow-step">{step || " "}</p>
      {children}
    </div>
  );
}

/** "Freed X" summary after a cleanup, with the undo pointer. */
export function CleanupDone({ report, title, children }: { report: CleanupReport; title: string; children?: ReactNode }) {
  const { t } = useTranslation();
  const f = useFormat();
  const removed = report.removedItems > 0;
  return (
    <div className="flow-center">
      <motion.div
        className="flow-done-icon"
        initial={{ scale: 0.6, opacity: 0 }}
        animate={{ scale: 1, opacity: 1 }}
        transition={{ type: "spring", stiffness: 260, damping: 18 }}
        aria-hidden="true"
      >
        <CheckmarkCircle48Regular />
      </motion.div>
      <h1 className="flow-title">{title}</h1>
      <div className="flow-big" aria-hidden={!removed}>
        {f.bytes(report.removedBytes + (report.binEmptiedBytes ?? 0))}
      </div>
      <div className="flow-lines">
        {removed && <p>{t(report.mode === "permanent" ? "cleanup.donePermanent" : "cleanup.doneRecycled", { size: f.bytes(report.removedBytes) })}</p>}
        {report.binEmptiedBytes !== null && <p>{t("cleanup.doneBin", { size: f.bytes(report.binEmptiedBytes) })}</p>}
        {!removed && report.binEmptiedBytes === null && <p>{t("cleanup.doneNothing")}</p>}
        {report.failedCount > 0 && <p className="flow-muted">{t("cleanup.doneSkipped", { count: report.failedCount })}</p>}
        {report.undoAvailable && (
          <p className="flow-muted">
            {t("cleanup.undoHint")} <Link to="/activity">{t("cleanup.openActivity")}</Link>
          </p>
        )}
      </div>
      {children}
    </div>
  );
}
