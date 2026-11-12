import { Info16Regular, Shield20Regular } from "@fluentui/react-icons";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Link } from "react-router";
import { Button, ErrorNotice, UsageBar } from "../../components";
import { call, toBackendError, type BackendError } from "../../lib/backend";
import type { DiskReliability, HardwareReading, HardwareView, PhysicalDisk, SensorsView } from "../../lib/types";
import { useFormat } from "../../lib/useFormat";
import "../modules.css";

const UNITS: Record<string, string> = { Temperature: " °C", Fan: " RPM", Load: " %", Power: " W", Clock: " MHz", Control: " %" };

function kindLabel(kind: string): string {
  if (kind.startsWith("Gpu")) return "gpu";
  return ["Cpu", "Memory", "Motherboard", "Storage", "SuperIO", "Cooler", "EmbeddedController", "Psu", "Battery", "Network"].includes(kind) ? kind.toLowerCase() : "other";
}

function SensorCard({ h }: { h: HardwareReading }) {
  const { t } = useTranslation();
  // Most useful first: temperatures, fans, power, then loads. Clocks are left out to keep it short,
  // and of the loads only the total (per-core and per-engine loads would fill the card).
  const order = ["Temperature", "Fan", "Power", "Load", "Control"];
  const loads = h.sensors.filter((s) => s.kind === "Load");
  const mainLoad = loads.find((s) => /total|gpu core|d3d 3d|^memory$/i.test(s.name)) ?? loads[0];
  const sensors = h.sensors
    .filter((s) => order.includes(s.kind) && (s.kind !== "Load" || s === mainLoad))
    .sort((a, b) => order.indexOf(a.kind) - order.indexOf(b.kind))
    .slice(0, 8);
  return (
    <section className="tool-card" aria-label={h.name}>
      <div>
        <h3 className="section-title">{h.name}</h3>
        <span className="tool-meta">{t(`hardware.kind.${kindLabel(h.kind)}`)}</span>
      </div>
      <table className="table">
        <tbody>
          {sensors.map((s) => (
            <tr key={`${s.kind}-${s.name}`}>
              <td>
                {s.name}
                <span className="row__sub"> · {t(`hardware.sensor.${s.kind}`, { defaultValue: s.kind })}</span>
              </td>
              <td className="num">
                {s.value.toFixed(0)}
                {UNITS[s.kind] ?? ""}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </section>
  );
}

function Sensors({ pawnio }: { pawnio: boolean }) {
  const { t } = useTranslation();
  const [view, setView] = useState<SensorsView | null>(null);
  const [error, setError] = useState<BackendError | null>(null);
  const [admin, setAdmin] = useState(false);
  const [adminError, setAdminError] = useState<BackendError | null>(null);

  useEffect(() => {
    let alive = true;
    const tick = () =>
      (admin ? call("sensors_admin_poll") : call("sensors_poll"))
        .then((v) => alive && setView(v))
        .catch((e) => {
          if (!alive) return;
          if (admin) {
            setAdmin(false);
            setAdminError(toBackendError(e));
          } else setError(toBackendError(e));
        });
    void tick();
    const timer = setInterval(tick, 2000);
    return () => {
      alive = false;
      clearInterval(timer);
      void call(admin ? "sensors_admin_stop" : "sensors_stop").catch(() => undefined);
    };
  }, [admin]);

  if (error) {
    return (
      <p className="cat-row__note">
        <Info16Regular aria-hidden="true" /> {error.code === "unsupported" ? t("hardware.helperMissing") : t(`errors.${error.code}`)}
      </p>
    );
  }
  if (!view) return <p className="flow-muted">{t("common.loading")}</p>;
  if (!view.running) return <p className="cat-row__note">{t("hardware.helperStopped")}</p>;
  if (!view.snapshot) return <p className="flow-muted">{t("hardware.sensorsStarting")}</p>;
  return (
    <>
      {!view.snapshot.elevated && (
        <div className="admin-offer">
          <Button icon={<Shield20Regular />} onClick={() => setAdmin(true)}>
            {t("hardware.adminSensors")}
          </Button>
          <p className="inline-note">
            {pawnio ? t("hardware.adminSensorsWhy") : t("hardware.pawnioNeeded")} {!pawnio && <Link to="/toolbox">{t("hardware.openToolbox")}</Link>}
          </p>
        </div>
      )}
      {view.snapshot.elevated && <p className="tool-meta">{t("hardware.adminOn")}</p>}
      {adminError && <ErrorNotice error={adminError} />}
      <div className="two-col">
        {view.snapshot.hardware.map((h) => (
          <SensorCard key={h.kind + h.name} h={h} />
        ))}
      </div>
    </>
  );
}

function SmartLine({ r }: { r: DiskReliability | undefined }) {
  const { t } = useTranslation();
  if (!r) return null;
  const errors = (r.readErrors ?? 0) + (r.writeErrors ?? 0);
  const parts = [
    r.temperature !== null ? t("hardware.smart.temp", { now: r.temperature, max: r.temperatureMax ?? "?" }) : null,
    r.powerOnHours !== null ? t("hardware.smart.hours", { count: r.powerOnHours }) : null,
    r.wear !== null ? t("hardware.smart.wear", { percent: r.wear }) : null,
    errors > 0 ? t("hardware.smart.errors", { count: errors }) : t("hardware.smart.noErrors"),
  ].filter(Boolean);
  return <span className="row__sub">{parts.join(" · ")}</span>;
}

function reliabilityFor(d: PhysicalDisk, list: DiskReliability[] | null): DiskReliability | undefined {
  if (!list) return undefined;
  return list.find((r) => (d.serial && r.serial && r.serial === d.serial.trim()) || r.name === d.name);
}

export function HardwarePage() {
  const { t } = useTranslation();
  const f = useFormat();
  const [view, setView] = useState<HardwareView | null>(null);
  const [error, setError] = useState<BackendError | null>(null);
  const [smart, setSmart] = useState<DiskReliability[] | null>(null);
  const [smartBusy, setSmartBusy] = useState(false);
  const [smartError, setSmartError] = useState<BackendError | null>(null);

  const readSmart = async () => {
    setSmartBusy(true);
    setSmartError(null);
    try {
      setSmart(await call("disk_reliability"));
    } catch (e) {
      setSmartError(toBackendError(e));
    } finally {
      setSmartBusy(false);
    }
  };

  useEffect(() => {
    call("hardware_info")
      .then(setView)
      .catch((e) => setError(toBackendError(e)));
  }, []);

  const sys = view?.system;
  const mem = view?.memory;
  return (
    <div className="tool">
      <header className="tool__head">
        <h1>{t("modules.hardware.name")}</h1>
        <p className="tool-meta">{t("modules.hardware.tagline")}</p>
      </header>
      {error && <ErrorNotice error={error} />}

      <section className="tool-section" aria-labelledby="sensors-title">
        <h2 id="sensors-title" className="section-title">
          {t("hardware.live")}
        </h2>
        <Sensors pawnio={!!view?.pawnio} />
      </section>

      {!view ? (
        <p className="flow-muted">{t("common.loading")}</p>
      ) : (
        <>
          <section className="tool-section" aria-labelledby="disks-title">
            <h2 id="disks-title" className="section-title">
              {t("hardware.disks")}
            </h2>
            {view.disks ? (
              <ul className="rows">
                {view.disks.map((d) => (
                  <li key={d.name + d.letters.join()} className="row">
                    <span className={`status-dot status-dot--${d.health === "healthy" ? "good" : d.health === "unknown" ? "warning" : "critical"}`} aria-hidden="true" />
                    <div className="row__main">
                      <span className="row__title">
                        {d.letters.join(" ")} {d.name}
                      </span>
                      <span className="row__sub">
                        {[d.mediaType, d.busType, f.bytes(d.sizeBytes), t(`hardware.health.${d.health}`)].filter(Boolean).join(" · ")}
                      </span>
                      <SmartLine r={reliabilityFor(d, smart)} />
                    </div>
                  </li>
                ))}
              </ul>
            ) : (
              <p className="flow-muted">{t("smartCare.card.disksUnknown")}</p>
            )}
            {!smart && (
              <div className="admin-offer">
                <Button icon={<Shield20Regular />} disabled={smartBusy} onClick={() => void readSmart()}>
                  {smartBusy ? t("admin.waiting") : t("hardware.readSmart")}
                </Button>
                <p className="inline-note">{t("hardware.smartAdmin")}</p>
              </div>
            )}
            {smartError && <ErrorNotice error={smartError} />}
          </section>

          <div className="two-col">
            <section className="tool-card" aria-labelledby="cpu-title">
              <h2 id="cpu-title" className="section-title">
                {t("hardware.processor")}
              </h2>
              {sys?.cpu ? (
                <>
                  <span className="row__title">{sys.cpu.name}</span>
                  <span className="row__sub">{t("hardware.cpuLine", { cores: sys.cpu.cores, threads: sys.cpu.threads, ghz: (sys.cpu.maxMhz / 1000).toFixed(1) })}</span>
                </>
              ) : (
                <p className="flow-muted">{t("performance.unavailable")}</p>
              )}
              <h2 className="section-title">{t("hardware.graphics")}</h2>
              {(sys?.gpus ?? []).map((g) => (
                <span key={g.name} className="row__sub">
                  <strong>{g.name}</strong> · {t("hardware.driver", { version: g.driverVersion })}
                </span>
              ))}
            </section>

            <section className="tool-card" aria-labelledby="mem-title">
              <h2 id="mem-title" className="section-title">
                {t("hardware.memory")}
              </h2>
              {mem && (
                <>
                  <span className="row__title">{t("hardware.memInUse", { used: f.bytes(mem.totalBytes - mem.availableBytes), total: f.bytes(mem.totalBytes) })}</span>
                  <UsageBar value={mem.loadPercent} label={t("hardware.memory")} tone={mem.loadPercent > 90 ? "critical" : mem.loadPercent > 80 ? "warning" : "normal"} />
                </>
              )}
              {sys && sys.modules.length > 0 && (
                <table className="table">
                  <tbody>
                    {sys.modules.map((m, i) => (
                      <tr key={`${m.bank}-${i}`}>
                        <td>
                          {m.bank || m.slot}
                          <span className="row__sub"> · {[m.manufacturer, m.partNumber].filter((x) => x && x !== "Unknown").join(" ")}</span>
                        </td>
                        <td className="num">
                          {f.bytes(m.capacityBytes)} {m.kind} {m.speedMts ? `${m.speedMts} MT/s` : ""}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
              {sys?.slots ? <span className="row__sub">{t("hardware.slots", { used: sys.modules.length, total: sys.slots })}</span> : null}
            </section>
          </div>

          {view.hasBattery && (
            <section className="tool-section" aria-labelledby="battery-title">
              <h2 id="battery-title" className="section-title">
                {t("hardware.battery")}
              </h2>
              {view.batteries.length === 0 ? (
                <p className="flow-muted">{t("performance.unavailable")}</p>
              ) : (
                <ul className="rows">
                  {view.batteries.map((b) => {
                    const health = b.designCapacity ? (b.fullChargeCapacity / b.designCapacity) * 100 : 0;
                    return (
                      <li key={b.name} className="row">
                        <div className="row__main">
                          <span className="row__title">{t("hardware.batteryHealth", { percent: f.percent(health) })}</span>
                          <span className="row__sub">
                            {t("hardware.batteryLine", { full: (b.fullChargeCapacity / 1000).toFixed(1), design: (b.designCapacity / 1000).toFixed(1) })}
                            {b.cycleCount ? ` · ${t("hardware.cycles", { count: b.cycleCount })}` : ""}
                          </span>
                          <UsageBar value={health} label={t("hardware.battery")} tone={health < 70 ? "critical" : health < 80 ? "warning" : "normal"} />
                        </div>
                      </li>
                    );
                  })}
                </ul>
              )}
            </section>
          )}
        </>
      )}
      <div style={{ height: 32 }} />
    </div>
  );
}
