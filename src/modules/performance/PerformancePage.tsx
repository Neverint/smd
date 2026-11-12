import { LockClosed16Regular, Open20Regular } from "@fluentui/react-icons";
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button, ErrorNotice, Tabs, Toggle } from "../../components";
import { call, toBackendError, type BackendError } from "../../lib/backend";
import type { Background, ProcessSnapshot, StartupItem } from "../../lib/types";
import { useFormat } from "../../lib/useFormat";
import "../modules.css";

type Tab = "startup" | "running" | "background";

function StartupTab() {
  const { t } = useTranslation();
  const [items, setItems] = useState<StartupItem[] | null>(null);
  const [error, setError] = useState<BackendError | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  useEffect(() => {
    call("startup_list")
      .then(setItems)
      .catch((e) => setError(toBackendError(e)));
  }, []);

  const toggle = async (item: StartupItem, enabled: boolean) => {
    setBusy(item.id);
    try {
      setItems(await call("startup_set", { id: item.id, enabled }));
      setError(null);
    } catch (e) {
      setError(toBackendError(e));
    } finally {
      setBusy(null);
    }
  };

  if (!items) return error ? <ErrorNotice error={error} /> : <p className="flow-muted">{t("common.loading")}</p>;
  const on = items.filter((i) => i.enabled).length;
  return (
    <div className="tool-section">
      {error && <ErrorNotice error={error} />}
      <p className="tool-meta">
        {t("performance.startupSummary", { on, total: items.length })} · {t("performance.startupHint")}
      </p>
      <ul className="rows">
        {items.map((i) => (
          <li key={i.id} className="row">
            <div className="row__main">
              <span className="row__title">
                {i.name}
                {i.microsoft && <span className="tag">{t("performance.microsoft")}</span>}
              </span>
              <span className="row__sub">
                {i.publisher ?? t("common.unknownPublisher")} · {t(`performance.source.${i.source}`)}
              </span>
              <span className="row__sub row__mono" title={i.command}>
                {i.command}
              </span>
              {i.needsAdmin && (
                <span className="cat-row__note">
                  <LockClosed16Regular aria-hidden="true" /> {t("performance.adminLocked")}
                </span>
              )}
            </div>
            <Toggle checked={i.enabled} disabled={i.needsAdmin || busy !== null} onChange={(v) => void toggle(i, v)} label={t("performance.toggleLabel", { name: i.name })} />
          </li>
        ))}
      </ul>
    </div>
  );
}

function RunningTab() {
  const { t } = useTranslation();
  const f = useFormat();
  const [snap, setSnap] = useState<ProcessSnapshot | null>(null);
  const [error, setError] = useState<BackendError | null>(null);

  useEffect(() => {
    let alive = true;
    const tick = () =>
      call("processes_snapshot")
        .then((s) => alive && setSnap(s))
        .catch((e) => alive && setError(toBackendError(e)));
    void tick();
    const timer = setInterval(tick, 2000);
    return () => {
      alive = false;
      clearInterval(timer);
      void call("processes_stop").catch(() => undefined);
    };
  }, []);

  if (!snap) return error ? <ErrorNotice error={error} /> : <p className="flow-muted">{t("common.loading")}</p>;
  const table = (title: string, rows: ProcessSnapshot["byCpu"], value: (r: ProcessSnapshot["byCpu"][number]) => string) => (
    <section className="tool-card" aria-label={title}>
      <h2 className="section-title">{title}</h2>
      <table className="table">
        <tbody>
          {rows.map((r) => (
            <tr key={r.name}>
              <td>
                {r.name}
                <span className="row__sub"> · {t("performance.processes", { count: r.processes })}</span>
              </td>
              <td className="num">{value(r)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </section>
  );
  return (
    <div className="tool-section">
      <p className="tool-meta" aria-live="off">
        {t("performance.cpuTotal", { percent: f.percent(snap.cpuPercent), count: snap.processCount })} · {t("performance.liveNote")}
      </p>
      <div className="two-col">
        {table(t("performance.cpuTitle"), snap.byCpu, (r) => `${r.cpuPercent.toFixed(1)} %`)}
        {table(t("performance.memoryTitle"), snap.byMemory, (r) => f.bytes(r.memoryBytes))}
      </div>
    </div>
  );
}

function BackgroundTab({ data }: { data: Background | null }) {
  const { t } = useTranslation();
  if (!data) return <p className="flow-muted">{t("common.loading")}</p>;
  return (
    <div className="tool-section">
      <section className="tool-card">
        <div className="tool-card__head">
          <div>
            <h2 className="section-title">{t("performance.tasks")}</h2>
            <p className="tool-meta">{t("performance.tasksHint")}</p>
          </div>
          <Button icon={<Open20Regular />} onClick={() => void call("open_system_tool", { tool: "taskScheduler" })}>
            {t("performance.openTaskScheduler")}
          </Button>
        </div>
        {data.tasks === null ? (
          <p className="flow-muted">{t("performance.unavailable")}</p>
        ) : data.tasks.length === 0 ? (
          <p className="flow-muted">{t("performance.none")}</p>
        ) : (
          <table className="table">
            <tbody>
              {data.tasks.map((task) => (
                <tr key={task.path + task.name}>
                  <td>
                    {task.name}
                    <span className="row__sub"> · {task.publisher ?? (task.author || t("common.unknownPublisher"))}</span>
                  </td>
                  <td className="num">{task.state}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </section>
      <section className="tool-card">
        <div className="tool-card__head">
          <div>
            <h2 className="section-title">{t("performance.services")}</h2>
            <p className="tool-meta">{t("performance.servicesHint")}</p>
          </div>
          <Button icon={<Open20Regular />} onClick={() => void call("open_system_tool", { tool: "services" })}>
            {t("performance.openServices")}
          </Button>
        </div>
        {data.services === null ? (
          <p className="flow-muted">{t("performance.unavailable")}</p>
        ) : data.services.length === 0 ? (
          <p className="flow-muted">{t("performance.none")}</p>
        ) : (
          <table className="table">
            <tbody>
              {data.services.map((s) => (
                <tr key={s.name}>
                  <td>
                    {s.displayName}
                    <span className="row__sub"> · {s.publisher ?? t("common.unknownPublisher")}</span>
                  </td>
                  <td className="num">
                    {s.state} · {s.startMode}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </section>
    </div>
  );
}

export function PerformancePage() {
  const { t } = useTranslation();
  const f = useFormat();
  const [tab, setTab] = useState<Tab>("startup");
  const [bg, setBg] = useState<Background | null>(null);

  const loadBackground = useCallback(() => {
    call("background_list")
      .then(setBg)
      .catch(() => setBg({ tasks: null, services: null, boot: null }));
  }, []);
  useEffect(loadBackground, [loadBackground]);

  const boot = bg?.boot;
  return (
    <div className="tool">
      <header className="tool__head">
        <h1>{t("modules.performance.name")}</h1>
        <p className="tool-meta">{t("modules.performance.tagline")}</p>
        {boot && (
          <p className="tool-meta">
            {boot.bootDurationMs
              ? t("performance.bootRecorded", { date: f.dateTime(boot.lastBootMs), seconds: f.seconds(boot.bootDurationMs) })
              : t("performance.bootUnknown", { date: f.dateTime(boot.lastBootMs) })}
          </p>
        )}
      </header>
      <Tabs<Tab>
        label={t("modules.performance.name")}
        value={tab}
        onChange={setTab}
        tabs={[
          { id: "startup", label: t("performance.tabs.startup") },
          { id: "running", label: t("performance.tabs.running") },
          { id: "background", label: t("performance.tabs.background") },
        ]}
      >
        {tab === "startup" && <StartupTab />}
        {tab === "running" && <RunningTab />}
        {tab === "background" && <BackgroundTab data={bg} />}
      </Tabs>
    </div>
  );
}
