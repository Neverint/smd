import { Delete20Regular, Open20Regular, Sparkle20Regular } from "@fluentui/react-icons";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button, UsageBar } from "../../components";
import { call } from "../../lib/backend";
import type { TrayInfo, TraySample } from "../../lib/types";
import { useFormat } from "../../lib/useFormat";
import "./tray.css";

const POINTS = 60;

function Spark({ values, max, label }: { values: number[]; max: number; label: string }) {
  const w = 120;
  const h = 28;
  const top = Math.max(max, 1);
  const pts = values.map((v, i) => `${(i / (POINTS - 1)) * w},${h - (Math.min(v, top) / top) * (h - 2) - 1}`).join(" ");
  return (
    <svg className="spark" viewBox={`0 0 ${w} ${h}`} width={w} height={h} role="img" aria-label={label} preserveAspectRatio="none">
      <polyline points={pts} fill="none" stroke="currentColor" strokeWidth="1.5" vectorEffect="non-scaling-stroke" />
    </svg>
  );
}

function Metric({ name, value, values, max }: { name: string; value: string; values: number[]; max: number }) {
  return (
    <div className="tray-metric">
      <div>
        <div className="tray-metric__name">{name}</div>
        <div className="tray-metric__value">{value}</div>
      </div>
      <Spark values={values} max={max} label={`${name}: ${value}`} />
    </div>
  );
}

/** The tray icon's popover: live numbers and quick actions. */
export function TrayPage() {
  const { t } = useTranslation();
  const f = useFormat();
  const [samples, setSamples] = useState<TraySample[]>([]);
  const [info, setInfo] = useState<TrayInfo | null>(null);
  const [confirmBin, setConfirmBin] = useState(false);

  useEffect(() => {
    document.documentElement.classList.add("tray-window");
    let alive = true;
    const tick = () =>
      call("tray_sample")
        .then((s) => alive && setSamples((prev) => [...prev, s].slice(-POINTS)))
        .catch(() => undefined);
    const slow = () =>
      call("tray_info")
        .then((i) => alive && setInfo(i))
        .catch(() => undefined);
    void tick();
    void slow();
    const a = setInterval(tick, 1000);
    const b = setInterval(slow, 10_000);
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") void call("tray_hide");
    };
    window.addEventListener("keydown", onKey);
    return () => {
      alive = false;
      clearInterval(a);
      clearInterval(b);
      window.removeEventListener("keydown", onKey);
    };
  }, []);

  const pad = (vals: number[]) => [...Array(Math.max(0, POINTS - vals.length)).fill(0), ...vals];
  const last = samples[samples.length - 1];
  const cpu = pad(samples.map((s) => s.cpuPercent));
  const mem = pad(samples.map((s) => s.memoryPercent));
  const disk = pad(samples.map((s) => s.diskBytesPerSec));
  const net = pad(samples.map((s) => s.netBytesPerSec));
  const freePct = info && info.systemTotal ? (info.systemFree / info.systemTotal) * 100 : 0;

  const emptyBin = async () => {
    setConfirmBin(false);
    await call("tray_action", { action: "emptyRecycleBin" }).catch(() => undefined);
    setInfo(await call("tray_info"));
  };

  return (
    <main className="tray" aria-label={t("tray.title")}>
      <header className="tray__head">
        <strong>{t("app.name")}</strong>
        {info?.lastScore !== null && info?.lastScore !== undefined && (
          <span className="tray__score" title={info.lastScoreMs ? f.dateTime(info.lastScoreMs) : undefined}>
            {t("tray.score", { score: info.lastScore })}
          </span>
        )}
      </header>
      <Metric name={t("tray.cpu")} value={last ? `${Math.round(last.cpuPercent)} %` : "…"} values={cpu} max={100} />
      <Metric name={t("tray.memory")} value={last ? `${Math.round(last.memoryPercent)} %` : "…"} values={mem} max={100} />
      <Metric name={t("tray.disk")} value={last ? `${f.bytes(last.diskBytesPerSec)}/s` : "…"} values={disk} max={Math.max(...disk, 1024 * 1024)} />
      <Metric name={t("tray.network")} value={last ? `${f.bytes(last.netBytesPerSec)}/s` : "…"} values={net} max={Math.max(...net, 64 * 1024)} />
      <div className="tray-metric">
        <div>
          <div className="tray-metric__name">{t("tray.cpuTemp")}</div>
          <div className="tray-metric__value">{info?.cpuTempC ? `${Math.round(info.cpuTempC)} °C` : t("tray.cpuTempNone")}</div>
        </div>
      </div>
      {info && (
        <div className="tray-free">
          <div className="tray-metric__name">{t("tray.free", { drive: info.systemDrive, free: f.bytes(info.systemFree), total: f.bytes(info.systemTotal) })}</div>
          <UsageBar value={100 - freePct} label={t("tray.free", { drive: info.systemDrive, free: f.bytes(info.systemFree), total: f.bytes(info.systemTotal) })} tone={freePct < 10 ? "critical" : freePct < 20 ? "warning" : "normal"} />
        </div>
      )}
      <div className="tray__actions">
        <Button icon={<Sparkle20Regular />} disabled={info?.busy} onClick={() => void call("tray_action", { action: "smartCare" })}>
          {t("tray.smartCare")}
        </Button>
        {confirmBin ? (
          <Button variant="danger" onClick={() => void emptyBin()}>
            {t("tray.emptyConfirm", { size: f.bytes(info?.recycleBinBytes ?? 0) })}
          </Button>
        ) : (
          <Button icon={<Delete20Regular />} disabled={!info?.recycleBinBytes || info.busy} onClick={() => setConfirmBin(true)}>
            {t("tray.emptyBin")}
          </Button>
        )}
        <Button variant="primary" icon={<Open20Regular />} onClick={() => void call("tray_action", { action: "openApp" })}>
          {t("tray.open")}
        </Button>
      </div>
      {info?.busy && <p className="tray__note">{t("tray.busy")}</p>}
    </main>
  );
}
