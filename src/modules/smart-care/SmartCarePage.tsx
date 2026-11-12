import {
  Apps24Regular,
  ArrowClockwise24Regular,
  Battery1024Regular,
  Broom24Regular,
  HardDrive24Regular,
  Lightbulb20Regular,
  Memory16Regular,
  ShieldCheckmark24Regular,
  Storage24Regular,
  TopSpeed24Regular,
} from "@fluentui/react-icons";
import { useTranslation } from "react-i18next";
import { useNavigate } from "react-router";
import { moduleById } from "../../app/modules";
import { Button, Card, ErrorNotice, ModuleHero, ProgressRing, ScanButton, UsageBar } from "../../components";
import type { Area, Recommendation, SmartCareResult, Status } from "../../lib/types";
import { useFormat } from "../../lib/useFormat";
import { useSmartCare } from "../../stores/smartCare";
import { CleanupDone, Scanning } from "../shared/ScanFlow";
import "../modules.css";

function StatusLine({ status }: { status: Status | undefined }) {
  const { t } = useTranslation();
  if (!status) return null;
  return (
    <div className="status-line">
      <span className={`status-dot status-dot--${status}`} aria-hidden="true" />
      {t(`smartCare.status.${status}`)}
    </div>
  );
}

function RecText({ rec }: { rec: Recommendation }) {
  const { t } = useTranslation();
  const f = useFormat();
  switch (rec.kind) {
    case "replaceDisk":
      return <>{t("smartCare.rec.replaceDisk", { disk: rec.disk })}</>;
    case "freeSpace":
      return <>{t("smartCare.rec.freeSpace", { drive: rec.drive, percent: f.percent(rec.freePercent) })}</>;
    case "restart":
      return <>{t("smartCare.rec.restart", { count: rec.uptimeDays })}</>;
    case "closeApps":
      return <>{t("smartCare.rec.closeApps", { percent: rec.loadPercent })}</>;
    case "cleanJunk":
      return <>{t("smartCare.rec.cleanJunk", { size: f.bytes(rec.bytes) })}</>;
    case "installSecurityUpdates":
    case "installWindowsUpdates":
    case "updateApps":
    case "reviewStartup":
    case "resolveThreats":
      return <>{t(`smartCare.rec.${rec.kind}`, { count: rec.count })}</>;
    case "turnOnRealTimeProtection":
      return <>{t("smartCare.rec.turnOnRealTimeProtection")}</>;
    case "updateSignatures":
      return <>{t("smartCare.rec.updateSignatures", { count: rec.days })}</>;
    case "checkBattery":
      return <>{t("smartCare.rec.checkBattery", { percent: f.percent(rec.healthPercent) })}</>;
    case "reviewDriveLayout":
      return <>{t("smartCare.rec.reviewDriveLayout", { size: f.bytes(rec.bytes) })}</>;
  }
}

function Results({ result }: { result: SmartCareResult }) {
  const { t } = useTranslation();
  const f = useFormat();
  const navigate = useNavigate();
  const { runCleanup } = useSmartCare();
  const statusOf = (area: Area) => result.assessment.areas.find((a) => a.area === area)?.status;
  const { assessment, systemDrive, memory, disks } = result;
  const junk = result.cleanup.reclaimableBytes;
  const badDisks = disks?.filter((d) => d.health === "warning" || d.health === "unhealthy") ?? [];
  const days = Math.floor(result.uptimeSeconds / 86_400);

  return (
    <div className="results">
      <div className="score">
        <ProgressRing value={assessment.score} size={132} thickness={10} label={t("smartCare.scoreLabel", { score: assessment.score })}>
          <div className="score__number">{assessment.score}</div>
          <div className="score__outof">{t("smartCare.outOf")}</div>
        </ProgressRing>
        <div>
          <div className="score__verdict">{t(`smartCare.verdict.${assessment.verdict}`)}</div>
          <div className="results__meta">{t("common.scannedIn", { seconds: f.seconds(result.durationMs) })}</div>
        </div>
      </div>

      <section aria-labelledby="recs-title">
        <h2 id="recs-title" className="section-title">
          {t("smartCare.recommended")}
        </h2>
        {assessment.recommendations.length === 0 ? (
          <p className="flow-muted">{t("smartCare.allGood")}</p>
        ) : (
          <ul className="recs">
            {assessment.recommendations.map((rec, i) => (
              <li key={i}>
                <Lightbulb20Regular aria-hidden="true" />
                <span>
                  <RecText rec={rec} />
                </span>
              </li>
            ))}
          </ul>
        )}
      </section>

      <div className="card-grid">
        <Card
          module="cleanup"
          icon={<Broom24Regular />}
          title={t("smartCare.card.junk")}
          value={f.bytes(junk)}
          description={junk > 0 ? t("smartCare.card.junkDesc") : t("smartCare.card.junkNone")}
          actions={
            <>
              <Button onClick={() => navigate("/cleanup")}>{t("common.review")}</Button>
              {junk > 0 && (
                <Button variant="primary" onClick={() => void runCleanup()}>
                  {t("smartCare.clean", { size: f.bytes(junk) })}
                </Button>
              )}
            </>
          }
        />
        <Card
          module="hardware"
          icon={<HardDrive24Regular />}
          title={t("smartCare.card.diskHealth")}
          description={
            disks === null
              ? t("smartCare.card.disksUnknown")
              : badDisks.length > 0
                ? t("smartCare.card.disksBad", { count: badDisks.length })
                : t("smartCare.card.disksOk", { count: disks.length })
          }
        >
          <StatusLine status={statusOf("diskHealth")} />
          {disks && (
            <ul className="disk-list">
              {disks.map((d) => (
                <li key={d.name + d.letters.join()}>
                  <span>
                    {d.letters.join(" ")} {d.name}
                  </span>
                  <span>{d.mediaType}</span>
                </li>
              ))}
            </ul>
          )}
        </Card>
        {systemDrive && (
          <Card
            module="space-lens"
            icon={<Storage24Regular />}
            title={t("smartCare.card.freeSpace", { drive: systemDrive.letter })}
            value={f.bytes(systemDrive.freeBytes)}
            description={t("smartCare.card.freeSpaceValue", { free: f.bytes(systemDrive.freeBytes), total: f.bytes(systemDrive.totalBytes) })}
          >
            <UsageBar
              value={100 - (systemDrive.freeBytes / systemDrive.totalBytes) * 100}
              label={t("smartCare.card.freeSpace", { drive: systemDrive.letter })}
              tone={statusOf("freeSpace") === "critical" ? "critical" : statusOf("freeSpace") === "warning" ? "warning" : "normal"}
            />
            <StatusLine status={statusOf("freeSpace")} />
          </Card>
        )}
        {memory && (
          <Card module="performance" icon={<Memory16Regular />} title={t("smartCare.card.memory")} value={t("smartCare.card.memoryValue", { percent: memory.loadPercent })}>
            <UsageBar value={memory.loadPercent} label={t("smartCare.card.memory")} tone={statusOf("memory") === "good" ? "normal" : statusOf("memory") === "critical" ? "critical" : "warning"} />
            <StatusLine status={statusOf("memory")} />
          </Card>
        )}
        <Card
          module="smart-care"
          icon={<ArrowClockwise24Regular />}
          title={t("smartCare.card.uptime")}
          value={days >= 1 ? t("smartCare.card.uptimeValue", { count: days }) : t("smartCare.card.uptimeHours", { count: Math.floor(result.uptimeSeconds / 3600) })}
        >
          <StatusLine status={statusOf("maintenance")} />
        </Card>
        {result.startupEnabled !== null && (
          <Card
            module="performance"
            icon={<TopSpeed24Regular />}
            title={t("smartCare.card.startup")}
            value={t("smartCare.card.startupValue", { count: result.startupEnabled })}
            description={t("smartCare.card.startupDesc")}
            actions={<Button onClick={() => navigate("/performance")}>{t("smartCare.card.open")}</Button>}
          >
            <StatusLine status={statusOf("startupLoad")} />
          </Card>
        )}
        <Card
          module="applications"
          icon={<Apps24Regular />}
          title={t("smartCare.card.updates")}
          value={result.appUpdates !== null || result.windowsUpdates ? t("smartCare.card.updatesValue", { count: (result.appUpdates ?? 0) + (result.windowsUpdates?.total ?? 0) }) : undefined}
          description={result.appUpdates === null ? t("smartCare.card.updatesNotSetUp") : t("smartCare.card.updatesDesc")}
          actions={<Button onClick={() => navigate("/applications")}>{t("smartCare.card.open")}</Button>}
        >
          {result.windowsUpdates && <div className="row__sub">{t("smartCare.card.windowsUpdatesLine", { total: result.windowsUpdates.total, security: result.windowsUpdates.security })}</div>}
        </Card>
        <Card
          module="protection"
          icon={<ShieldCheckmark24Regular />}
          title={t("smartCare.card.protection")}
          value={
            result.defender
              ? (result.activeThreats ?? 0) > 0
                ? t("smartCare.card.threats", { count: result.activeThreats ?? 0 })
                : result.defender.realTime
                  ? t("smartCare.card.protected")
                  : t("smartCare.card.notProtected")
              : undefined
          }
          description={result.defender ? (result.defender.signaturesUpdatedMs ? t("protection.signatures", { date: f.dateTime(result.defender.signaturesUpdatedMs) }) : undefined) : t("smartCare.card.defenderUnknown")}
          actions={<Button onClick={() => navigate("/protection")}>{t("smartCare.card.open")}</Button>}
        >
          <StatusLine status={statusOf("protection")} />
        </Card>
        {result.driveLayout && (
          <Card
            module="drive-organizer"
            icon={<Storage24Regular />}
            title={t("smartCare.card.driveLayout")}
            value={result.driveLayout.misplacedBytes > 0 ? t("smartCare.card.misplaced", { size: f.bytes(result.driveLayout.misplacedBytes) }) : t("smartCare.card.layoutOk")}
            description={t("smartCare.card.driveLayoutDesc")}
            actions={<Button onClick={() => navigate("/drive-organizer")}>{t("smartCare.card.open")}</Button>}
          >
            <StatusLine status={statusOf("driveLayout")} />
          </Card>
        )}
        {result.batteryHealth !== null && (
          <Card
            module="hardware"
            icon={<Battery1024Regular />}
            title={t("smartCare.card.battery")}
            value={`${f.percent(result.batteryHealth)} %`}
            description={t("smartCare.card.batteryDesc")}
            actions={<Button onClick={() => navigate("/hardware")}>{t("smartCare.card.open")}</Button>}
          >
            <StatusLine status={statusOf("battery")} />
          </Card>
        )}
      </div>
    </div>
  );
}

export function SmartCarePage() {
  const { t } = useTranslation();
  const def = moduleById("smart-care");
  const { phase, progress, result, report, error, startScan, reset } = useSmartCare();

  if (phase === "scanning") {
    return (
      <div className="page">
        <Scanning title={t("smartCare.scanning")} progress={progress} />
      </div>
    );
  }
  if (phase === "running") {
    return (
      <div className="page">
        <Scanning title={t("smartCare.running")} progress={null} />
      </div>
    );
  }
  if (phase === "done" && report) {
    return (
      <div className="page">
        <CleanupDone report={report} title={t("smartCare.doneTitle")}>
          <div className="flow-actions">
            <Button variant="primary" size="lg" onClick={() => void startScan()}>
              {t("common.scanAgain")}
            </Button>
          </div>
        </CleanupDone>
      </div>
    );
  }
  if (phase === "results" && result) {
    return (
      <div className="page">
        <div className="page__body">
          <Results result={result} />
        </div>
        <div className="page__action">
          <Button variant="subtle" onClick={() => void startScan()}>
            {t("common.scanAgain")}
          </Button>
        </div>
      </div>
    );
  }

  return (
    <div className="page">
      <div className="page__body">
        <ModuleHero icon={def.icon} title={t("modules.smart-care.name")} tagline={t("modules.smart-care.tagline")} />
        {phase === "error" && error && (
          <ErrorNotice
            error={error}
            actions={
              <Button variant="subtle" onClick={reset}>
                {t("common.close")}
              </Button>
            }
          />
        )}
      </div>
      <div className="page__action">
        <ScanButton onClick={() => void startScan()}>{t("common.scan")}</ScanButton>
      </div>
    </div>
  );
}
