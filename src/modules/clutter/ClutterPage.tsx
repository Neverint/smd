import { CheckmarkCircle48Regular, FolderOpen20Regular } from "@fluentui/react-icons";
import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Link } from "react-router";
import { moduleById } from "../../app/modules";
import { Button, ErrorNotice, ModuleHero, ProgressRing, Segmented, Tabs } from "../../components";
import { call, toBackendError, type BackendError } from "../../lib/backend";
import type { ClutterFile, DownloadsView } from "../../lib/types";
import { useFormat } from "../../lib/useFormat";
import { allButOne, useClutter } from "../../stores/clutter";
import "../modules.css";

type Tab = "large" | "old" | "duplicates" | "downloads";

function toggle(set: Set<string>, path: string): Set<string> {
  const next = new Set(set);
  if (next.has(path)) next.delete(path);
  else next.add(path);
  return next;
}

function FileRow({ file, checked, onToggle, disabled }: { file: ClutterFile; checked: boolean; onToggle: () => void; disabled?: boolean }) {
  const { t } = useTranslation();
  const f = useFormat();
  return (
    <li className="row">
      <input type="checkbox" checked={checked} disabled={disabled} onChange={onToggle} aria-label={file.path} />
      <div className="row__main">
        <span className="row__title">{file.name}</span>
        <span className="row__sub row__mono" title={file.path}>
          {file.path}
        </span>
        {file.modifiedMs !== null && <span className="row__sub">{t("clutter.lastChanged", { date: f.dateTime(file.modifiedMs) })}</span>}
      </div>
      <span className="row__size">{f.bytes(file.bytes)}</span>
    </li>
  );
}

function Downloads({ selected, setSelected, onLoaded }: { selected: Set<string>; setSelected: (s: Set<string>) => void; onLoaded: (v: DownloadsView) => void }) {
  const { t } = useTranslation();
  const f = useFormat();
  const [view, setView] = useState<DownloadsView | null>(null);
  const [error, setError] = useState<BackendError | null>(null);
  const [sort, setSort] = useState<"size" | "age">("size");

  useEffect(() => {
    call("clutter_downloads")
      .then((v) => {
        setView(v);
        onLoaded(v);
      })
      .catch((e) => setError(toBackendError(e)));
  }, [onLoaded]);

  const items = useMemo(() => {
    const list = [...(view?.items ?? [])];
    if (sort === "age") list.sort((a, b) => (a.modifiedMs ?? 0) - (b.modifiedMs ?? 0));
    return list;
  }, [view, sort]);

  if (error) return <ErrorNotice error={error} />;
  if (!view) return <p className="flow-muted">{t("common.loading")}</p>;
  return (
    <>
      <div className="toolbar">
        <span className="tool-meta">
          {t("clutter.downloadsHint")} {view.path}
        </span>
        <Segmented<"size" | "age">
          label={t("clutter.tabs.downloads")}
          value={sort}
          onChange={setSort}
          options={[
            { value: "size", label: t("clutter.sortBySize") },
            { value: "age", label: t("clutter.sortByAge") },
          ]}
        />
      </div>
      {items.length === 0 ? (
        <p className="flow-muted">{t("clutter.none")}</p>
      ) : (
        <ul className="rows">
          {items.map((i) => (
            <li key={i.path} className="row">
              <input type="checkbox" checked={selected.has(i.path)} onChange={() => setSelected(toggle(selected, i.path))} aria-label={i.name} />
              <div className="row__main">
                <span className="row__title">{i.name}</span>
                <span className="row__sub">
                  {[i.isDir ? t("common.files", { count: i.files }) : null, i.modifiedMs ? t("clutter.lastChanged", { date: f.dateTime(i.modifiedMs) }) : null].filter(Boolean).join(" · ")}
                </span>
              </div>
              <span className="row__size">{f.bytes(i.bytes)}</span>
            </li>
          ))}
        </ul>
      )}
    </>
  );
}

export function ClutterPage() {
  const { t } = useTranslation();
  const f = useFormat();
  const def = moduleById("clutter");
  const { phase, progress, view, report, error, scan, pickAndScan, cancel, remove, reset } = useClutter();
  const [tab, setTab] = useState<Tab>("large");
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [dlSelected, setDlSelected] = useState<Set<string>>(new Set());
  const [downloads, setDownloads] = useState<DownloadsView | null>(null);

  const sizes = useMemo(() => {
    const m = new Map<string, number>();
    if (view) for (const file of [...view.result.large, ...view.result.old, ...view.result.duplicates.flatMap((g) => g.files)]) m.set(file.path, file.bytes);
    return m;
  }, [view]);

  if (phase === "scanning") {
    const p = progress;
    const comparing = p?.stage === "compare";
    return (
      <div className="page">
        <div className="flow-center" aria-live="polite">
          <ProgressRing value={comparing && p.toHash > 0 ? (p.hashed / p.toHash) * 100 : undefined} size={176} thickness={12} label={t("clutter.scanning")} />
          <h1 className="flow-title">{t("clutter.scanning")}</h1>
          <p className="flow-step">
            {p ? (comparing ? t("clutter.comparing", { done: f.bytes(p.hashed), total: f.bytes(p.toHash) }) : t("clutter.walking", { files: f.number(p.files), size: f.bytes(p.bytes) })) : " "}
          </p>
          <div className="flow-actions">
            <Button onClick={() => void cancel()}>{t("common.stop")}</Button>
          </div>
        </div>
      </div>
    );
  }

  if (phase === "removing") {
    return (
      <div className="page">
        <div className="flow-center">
          <ProgressRing size={140} label={t("clutter.doneTitle")} />
        </div>
      </div>
    );
  }

  if (phase === "done" && report) {
    return (
      <div className="page">
        <div className="flow-center">
          <div className="flow-done-icon" aria-hidden="true">
            <CheckmarkCircle48Regular />
          </div>
          <h1 className="flow-title">{t("clutter.doneTitle")}</h1>
          <div className="flow-big">{f.bytes(report.removedBytes)}</div>
          <div className="flow-lines">
            <p>{report.quarantine ? t("cleanup.doneQuarantine", { size: f.bytes(report.removedBytes), path: report.quarantine }) : t("clutter.doneBody", { size: f.bytes(report.removedBytes) })}</p>
            {report.failures.length > 0 && <p className="flow-muted">{t("cleanup.doneSkipped", { count: report.failures.length })}</p>}
            {report.undoAvailable && (
              <p className="flow-muted">
                {t("cleanup.undoHint")} <Link to="/activity">{t("cleanup.openActivity")}</Link>
              </p>
            )}
          </div>
          <div className="flow-actions">
            <Button variant="primary" size="lg" onClick={reset}>
              {t("clutter.newScan")}
            </Button>
          </div>
        </div>
      </div>
    );
  }

  if (phase === "results" && view) {
    const r = view.result;
    const onDownloads = tab === "downloads";
    const activeSel = onDownloads ? dlSelected : selected;
    const activeBytes = onDownloads
      ? [...dlSelected].reduce((s, p) => s + (downloads?.items.find((i) => i.path === p)?.bytes ?? 0), 0)
      : [...selected].reduce((s, p) => s + (sizes.get(p) ?? 0), 0);
    const list = (files: ClutterFile[], hint: string) => (
      <>
        <p className="tool-meta">{hint}</p>
        {files.length === 0 ? (
          <p className="flow-muted">{t("clutter.none")}</p>
        ) : (
          <ul className="rows">
            {files.map((file) => (
              <FileRow key={file.path} file={file} checked={selected.has(file.path)} onToggle={() => setSelected(toggle(selected, file.path))} />
            ))}
          </ul>
        )}
      </>
    );
    return (
      <div className="tool">
        <header className="tool__head">
          <h1>{t("modules.clutter.name")}</h1>
          <p className="tool-meta">
            {t("clutter.summary", { files: f.number(r.filesScanned), seconds: f.seconds(view.durationMs) })} · {t("clutter.scannedFolders", { folders: view.roots.join(", ") })}
          </p>
        </header>
        <Tabs<Tab>
          label={t("modules.clutter.name")}
          value={tab}
          onChange={setTab}
          tabs={[
            { id: "large", label: t("clutter.tabs.large"), count: r.large.length },
            { id: "old", label: t("clutter.tabs.old"), count: r.old.length },
            { id: "duplicates", label: t("clutter.tabs.duplicates"), count: r.duplicates.length },
            { id: "downloads", label: t("clutter.tabs.downloads") },
          ]}
        >
          <div className="tool-section">
            {tab === "large" && list(r.large, t("clutter.largeHint"))}
            {tab === "old" && list(r.old, t("clutter.oldHint"))}
            {tab === "duplicates" && (
              <>
                <div className="toolbar">
                  <span className="tool-meta">
                    {t("clutter.duplicatesHint")} {t("clutter.wasted", { size: f.bytes(r.duplicateBytes) })}
                  </span>
                  {r.duplicates.length > 0 && (
                    <Button title={t("clutter.keepOneHint")} onClick={() => setSelected(new Set([...selected, ...allButOne(r.duplicates)]))}>
                      {t("clutter.keepOne")}
                    </Button>
                  )}
                </div>
                {r.duplicates.length === 0 && <p className="flow-muted">{t("clutter.none")}</p>}
                {r.duplicates.map((g) => {
                  const remaining = g.files.filter((x) => !selected.has(x.path)).length;
                  return (
                    <section key={g.files[0].path} className="dup-group" aria-label={g.files[0].name}>
                      <h3 className="section-title">
                        {g.files[0].name} · {t("clutter.copies", { count: g.files.length })} · {f.bytes(g.bytes)}
                      </h3>
                      <ul className="rows">
                        {g.files.map((file) => (
                          // The last unticked copy cannot be ticked: one copy always stays.
                          <FileRow key={file.path} file={file} checked={selected.has(file.path)} disabled={!selected.has(file.path) && remaining <= 1} onToggle={() => setSelected(toggle(selected, file.path))} />
                        ))}
                      </ul>
                    </section>
                  );
                })}
              </>
            )}
            {tab === "downloads" && <Downloads selected={dlSelected} setSelected={setDlSelected} onLoaded={setDownloads} />}
          </div>
        </Tabs>
        {error && <ErrorNotice error={error} />}
        <div className="page__action">
          <Button
            variant="primary"
            size="lg"
            disabled={activeSel.size === 0}
            onClick={() => void remove(onDownloads ? (downloads?.scanId ?? 0) : view.scanId, [...activeSel])}
          >
            {activeSel.size > 0 ? t("clutter.remove", { count: activeSel.size, size: f.bytes(activeBytes) }) : t("clutter.selectSome")}
          </Button>
          <Button variant="subtle" onClick={reset}>
            {t("clutter.newScan")}
          </Button>
        </div>
      </div>
    );
  }

  return (
    <div className="page">
      <div className="page__body">
        <ModuleHero icon={def.icon} title={t("modules.clutter.name")} tagline={t("modules.clutter.tagline")} />
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
        <p className="inline-note">{t("clutter.yourFolders")}</p>
      </div>
      <div className="page__action">
        <Button variant="primary" size="lg" onClick={() => void scan(null)}>
          {t("clutter.scanYourFolders")}
        </Button>
        <Button icon={<FolderOpen20Regular />} onClick={() => void pickAndScan()}>
          {t("common.chooseFolder")}
        </Button>
      </div>
    </div>
  );
}
