import { ArrowDownload20Regular, CheckmarkCircle20Regular, LockClosed16Regular, Open20Regular, Warning20Regular } from "@fluentui/react-icons";
import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button, CheckList, ConfirmDialog, ErrorNotice, ProgressRing, Segmented, SideSheet, Tabs } from "../../components";
import { call, stream, toBackendError, type BackendError } from "../../lib/backend";
import type { AppUpdate, IndexProgress, InstalledApp, LeftoversView, RemovalReport, UninstallOutcome, UpdateOutcome, WindowsUpdates } from "../../lib/types";
import { useFormat } from "../../lib/useFormat";
import "../modules.css";

type Tab = "installed" | "updates";
type Kind = "all" | "desktop" | "store";

function LeftoversSheet({ view, onClose }: { view: LeftoversView | null; onClose: () => void }) {
  const { t } = useTranslation();
  const f = useFormat();
  const selectable = useMemo(() => view?.items.filter((i) => !i.needsAdmin) ?? [], [view]);
  const locked = view?.items.filter((i) => i.needsAdmin) ?? [];
  // The parent keys this sheet by scan id, so these start fresh for every scan.
  const [selected, setSelected] = useState<Set<string>>(() => new Set(selectable.filter((i) => i.selected).map((i) => i.path)));
  const [report, setReport] = useState<RemovalReport | null>(null);
  const [error, setError] = useState<BackendError | null>(null);

  const remove = async () => {
    if (!view) return;
    try {
      setReport(await call("apps_remove_leftovers", { scanId: view.scanId, paths: [...selected] }));
    } catch (e) {
      setError(toBackendError(e));
    }
  };

  return (
    <SideSheet
      open={!!view}
      onClose={onClose}
      title={view ? t("applications.leftoversTitle", { name: view.appName }) : ""}
      footer={
        report ? (
          <Button variant="primary" onClick={onClose}>
            {t("common.done")}
          </Button>
        ) : (
          <Button variant="primary" disabled={selected.size === 0} onClick={() => void remove()}>
            {t("applications.removeLeftovers", { count: selected.size })}
          </Button>
        )
      }
    >
      {error && <ErrorNotice error={error} />}
      {report ? (
        <p role="status">{t("applications.leftoversDone", { count: report.removedItems, size: f.bytes(report.removedBytes) })}</p>
      ) : view && view.items.length === 0 ? (
        <p>{t("applications.leftoversNone")}</p>
      ) : (
        <>
          <p className="review-more">{t("applications.leftoversHint")}</p>
          <CheckList
            label={t("applications.leftoversTitle", { name: view?.appName ?? "" })}
            items={selectable.map((i) => ({
              id: i.path,
              label: i.path,
              size: i.kind === "folder" ? f.bytes(i.bytes) : undefined,
              description: `${t(`applications.confidence.${i.confidence}`)}${i.kind === "registryKey" ? ` · ${t("applications.registryKey")}` : ""}`,
            }))}
            selected={selected}
            onChange={setSelected}
          />
          {locked.length > 0 && (
            <>
              <h3 className="section-title" style={{ marginTop: 16 }}>
                <LockClosed16Regular aria-hidden="true" /> {t("applications.leftoversLocked")}
              </h3>
              <ul className="rows rows--plain">
                {locked.map((i) => (
                  <li key={i.path} className="row__sub">
                    {i.path} · {f.bytes(i.bytes)}
                  </li>
                ))}
              </ul>
            </>
          )}
        </>
      )}
    </SideSheet>
  );
}

function InstalledTab() {
  const { t } = useTranslation();
  const f = useFormat();
  const [apps, setApps] = useState<InstalledApp[] | null>(null);
  const [error, setError] = useState<BackendError | null>(null);
  const [query, setQuery] = useState("");
  const [kind, setKind] = useState<Kind>("all");
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [confirming, setConfirming] = useState(false);
  const [progress, setProgress] = useState<IndexProgress | null>(null);
  const [results, setResults] = useState<UninstallOutcome[] | null>(null);
  const [leftovers, setLeftovers] = useState<LeftoversView | null>(null);

  const load = () =>
    call("apps_list")
      .then((a) => {
        setApps(a);
        setError(null);
      })
      .catch((e) => setError(toBackendError(e)));
  useEffect(() => {
    void load();
  }, []);

  const shown = useMemo(() => {
    const q = query.trim().toLowerCase();
    return (apps ?? []).filter((a) => (kind === "all" || a.kind === kind) && (!q || a.name.toLowerCase().includes(q) || a.publisher.toLowerCase().includes(q)));
  }, [apps, query, kind]);

  const uninstall = async () => {
    setConfirming(false);
    setProgress({ index: 0, total: selected.size });
    try {
      const out = await stream("apps_uninstall", { ids: [...selected] }, setProgress);
      setResults(out);
      setSelected(new Set());
    } catch (e) {
      setError(toBackendError(e));
    } finally {
      setProgress(null);
    }
  };

  const findLeftovers = async (id: string) => {
    try {
      setLeftovers(await call("apps_leftovers", { id }));
    } catch (e) {
      setError(toBackendError(e));
    }
  };

  if (progress) {
    return (
      <div className="flow-center" aria-live="polite">
        <ProgressRing value={progress.total ? ((progress.index - 1) / progress.total) * 100 : undefined} size={140} label={t("applications.uninstalling", { name: progress.name ?? "", index: progress.index, total: progress.total })} />
        <h2 className="flow-title">{t("applications.uninstalling", { name: progress.name ?? "…", index: progress.index, total: progress.total })}</h2>
        <p className="flow-step">{t("applications.uninstallingHint")}</p>
      </div>
    );
  }

  if (results) {
    return (
      <div className="tool-section">
        <h2 className="section-title">{t("applications.resultsTitle")}</h2>
        {error && <ErrorNotice error={error} />}
        <ul className="rows">
          {results.map((r) => {
            const ok = r.result && !r.result.stillInstalled;
            return (
              <li key={r.id} className="row">
                {ok ? <CheckmarkCircle20Regular className="ok-icon" aria-hidden="true" /> : <Warning20Regular className="warn-icon" aria-hidden="true" />}
                <div className="row__main">
                  <span className="row__title">{r.name}</span>
                  <span className="row__sub">{ok ? t("applications.removed") : r.error ? t("applications.failed") : t("applications.stillInstalled")}</span>
                </div>
                {ok && <Button onClick={() => void findLeftovers(r.id)}>{t("applications.findLeftovers")}</Button>}
              </li>
            );
          })}
        </ul>
        <div>
          <Button
            onClick={() => {
              setResults(null);
              void load();
            }}
          >
            {t("applications.backToList")}
          </Button>
        </div>
        <LeftoversSheet key={leftovers?.scanId ?? 0} view={leftovers} onClose={() => setLeftovers(null)} />
      </div>
    );
  }

  return (
    <div className="tool-section">
      <div className="toolbar">
        <input className="search" type="search" placeholder={t("common.search")} aria-label={t("common.search")} value={query} onChange={(e) => setQuery(e.target.value)} />
        <Segmented<Kind>
          label={t("applications.filter.all")}
          value={kind}
          onChange={setKind}
          options={[
            { value: "all", label: t("applications.filter.all") },
            { value: "desktop", label: t("applications.filter.desktop") },
            { value: "store", label: t("applications.filter.store") },
          ]}
        />
        <span className="tool-meta">{t("applications.count", { count: shown.length })}</span>
      </div>
      {error && <ErrorNotice error={error} />}
      {!apps ? (
        <p className="flow-muted">{t("common.loading")}</p>
      ) : (
        <ul className="rows">
          {shown.map((a) => (
            <li key={a.id} className="row">
              <input
                type="checkbox"
                checked={selected.has(a.id)}
                aria-label={a.name}
                onChange={() => {
                  const next = new Set(selected);
                  if (next.has(a.id)) next.delete(a.id);
                  else next.add(a.id);
                  setSelected(next);
                }}
              />
              <div className="row__main">
                <span className="row__title">
                  {a.name}
                  {a.kind === "store" && <span className="tag">{t("applications.store")}</span>}
                </span>
                <span className="row__sub">
                  {[a.publisher || t("common.unknownPublisher"), a.version, a.installDate ? t("applications.installedOn", { date: a.installDate }) : null, a.machineWide ? t("applications.adminPrompt") : null]
                    .filter(Boolean)
                    .join(" · ")}
                </span>
              </div>
              <span className="row__size">{a.sizeBytes ? f.bytes(a.sizeBytes) : ""}</span>
            </li>
          ))}
        </ul>
      )}
      <div className="page__action">
        <Button variant="primary" size="lg" disabled={selected.size === 0} onClick={() => setConfirming(true)}>
          {t("applications.uninstall", { count: selected.size })}
        </Button>
      </div>
      <ConfirmDialog open={confirming} title={t("applications.confirmTitle")} confirmLabel={t("applications.confirm")} onCancel={() => setConfirming(false)} onConfirm={() => void uninstall()}>
        <p>{t("applications.confirmBody")}</p>
        <ul>
          {(apps ?? [])
            .filter((a) => selected.has(a.id))
            .map((a) => (
              <li key={a.id}>{a.name}</li>
            ))}
        </ul>
      </ConfirmDialog>
    </div>
  );
}

function UpdatesTab() {
  const { t } = useTranslation();
  const [installed, setInstalled] = useState<boolean | null>(null);
  const [settingUp, setSettingUp] = useState(false);
  const [updates, setUpdates] = useState<AppUpdate[] | null>(null);
  const [wu, setWu] = useState<WindowsUpdates | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [progress, setProgress] = useState<IndexProgress | null>(null);
  const [results, setResults] = useState<UpdateOutcome[] | null>(null);
  const [error, setError] = useState<BackendError | null>(null);

  const list = () => {
    setUpdates(null);
    call("updates_list")
      .then((u) => {
        setUpdates(u);
        setSelected(new Set(u.map((x) => x.id)));
      })
      .catch((e) => setError(toBackendError(e)));
  };

  useEffect(() => {
    call("updates_status")
      .then((s) => {
        setInstalled(s.moduleInstalled);
        if (s.moduleInstalled) list();
      })
      .catch((e) => setError(toBackendError(e)));
    call("windows_updates")
      .then(setWu)
      .catch(() => setWu(null));
  }, []);

  const setup = async () => {
    setSettingUp(true);
    try {
      const s = await call("updates_setup");
      setInstalled(s.moduleInstalled);
      if (s.moduleInstalled) list();
    } catch (e) {
      setError(toBackendError(e));
    } finally {
      setSettingUp(false);
    }
  };

  const apply = async () => {
    setProgress({ index: 0, total: selected.size });
    try {
      setResults(await stream("updates_apply", { ids: [...selected] }, setProgress));
    } catch (e) {
      setError(toBackendError(e));
    } finally {
      setProgress(null);
    }
  };

  return (
    <div className="tool-section">
      {error && <ErrorNotice error={error} />}
      <section className="tool-card">
        <div className="tool-card__head">
          <div>
            <h2 className="section-title">{t("applications.windowsUpdate")}</h2>
            <p className="tool-meta">
              {wu
                ? wu.total > 0
                  ? t("applications.windowsUpdateLine", { count: wu.total, security: wu.security })
                  : t("applications.windowsUpdateNone")
                : t("common.loading")}{" "}
              {t("applications.windowsUpdateHint")}
            </p>
          </div>
          <Button icon={<Open20Regular />} onClick={() => void call("open_system_tool", { tool: "windowsUpdate" })}>
            {t("applications.openWindowsUpdate")}
          </Button>
        </div>
      </section>

      {installed === false && (
        <section className="tool-card">
          <h2 className="section-title">{t("applications.updatesSetupTitle")}</h2>
          <p className="tool-meta">{t("applications.updatesSetupBody")}</p>
          <div>
            <Button variant="primary" icon={<ArrowDownload20Regular />} disabled={settingUp} onClick={() => void setup()}>
              {settingUp ? t("applications.updatesSetupRunning") : t("applications.updatesSetupButton")}
            </Button>
          </div>
        </section>
      )}

      {installed && progress && (
        <div className="flow-center" aria-live="polite">
          <ProgressRing size={120} label={t("applications.updating", { id: progress.id ?? "", index: progress.index, total: progress.total })} />
          <p className="flow-step">{t("applications.updating", { id: progress.id ?? "…", index: progress.index, total: progress.total })}</p>
        </div>
      )}

      {installed && !progress && results && (
        <ul className="rows">
          {results.map((r) => {
            const ok = r.result?.status?.toLowerCase() === "ok";
            return (
              <li key={r.id} className="row">
                {ok ? <CheckmarkCircle20Regular className="ok-icon" aria-hidden="true" /> : <Warning20Regular className="warn-icon" aria-hidden="true" />}
                <div className="row__main">
                  <span className="row__title">{r.id}</span>
                  <span className="row__sub">{ok ? (r.result?.rebootRequired ? t("applications.rebootNeeded") : t("applications.updated")) : t("applications.updateFailed")}</span>
                </div>
              </li>
            );
          })}
        </ul>
      )}

      {installed && !progress && !results && (
        <>
          {updates === null ? (
            <p className="flow-muted">{t("applications.updatesChecking")}</p>
          ) : updates.length === 0 ? (
            <p className="flow-muted">{t("applications.updatesNone")}</p>
          ) : (
            <>
              <div className="smd-card">
                <CheckList
                  label={t("applications.tabs.updates")}
                  items={updates.map((u) => ({ id: u.id, label: u.name, description: `${u.id} · ${t("applications.version", { installed: u.installed, available: u.available })}` }))}
                  selected={selected}
                  onChange={setSelected}
                />
              </div>
              <div className="page__action">
                <Button variant="primary" size="lg" disabled={selected.size === 0} onClick={() => void apply()}>
                  {t("applications.update", { count: selected.size })}
                </Button>
                <p className="inline-note">{t("applications.updatesNetwork")}</p>
              </div>
            </>
          )}
        </>
      )}
    </div>
  );
}

export function ApplicationsPage() {
  const { t } = useTranslation();
  const [tab, setTab] = useState<Tab>("installed");
  return (
    <div className="tool">
      <header className="tool__head">
        <h1>{t("modules.applications.name")}</h1>
        <p className="tool-meta">{t("modules.applications.tagline")}</p>
      </header>
      <Tabs<Tab>
        label={t("modules.applications.name")}
        value={tab}
        onChange={setTab}
        tabs={[
          { id: "installed", label: t("applications.tabs.installed") },
          { id: "updates", label: t("applications.tabs.updates") },
        ]}
      >
        {tab === "installed" ? <InstalledTab /> : <UpdatesTab />}
      </Tabs>
    </div>
  );
}
