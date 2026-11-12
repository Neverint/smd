import {
  ArrowUp20Regular,
  Document20Regular,
  DocumentMultiple20Regular,
  Folder20Regular,
  FolderOpen20Regular,
  FolderProhibited20Regular,
  HardDrive24Regular,
  Open20Regular,
} from "@fluentui/react-icons";
import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { moduleById } from "../../app/modules";
import { Button, ErrorNotice, IconButton, ModuleHero, UsageBar } from "../../components";
import type { SpaceSummary, SpaceView, ViewNode } from "../../lib/types";
import { useFormat } from "../../lib/useFormat";
import { useSpaceLens } from "../../stores/spaceLens";
import { Scanning } from "../shared/ScanFlow";
import { Treemap } from "./Treemap";
import "../modules.css";

function DrivePicker() {
  const { t } = useTranslation();
  const f = useFormat();
  const { drives, loadDrives, scan, pickFolder } = useSpaceLens();
  useEffect(() => {
    void loadDrives();
  }, [loadDrives]);

  return (
    <>
      <h2 className="section-title">{t("spaceLens.pick")}</h2>
      <div className="drive-grid">
        {drives.map((d) => {
          const used = d.totalBytes > 0 ? 100 - (d.freeBytes / d.totalBytes) * 100 : 0;
          return (
            <button key={d.letter} type="button" className="drive-card" onClick={() => void scan(d.root)}>
              <span className="drive-card__title">
                <HardDrive24Regular aria-hidden="true" />
                {d.letter} {d.label || (d.isSystem ? t("spaceLens.system") : "")}
              </span>
              <UsageBar value={used} label={`${d.letter} ${t("spaceLens.driveFree", { free: f.bytes(d.freeBytes), total: f.bytes(d.totalBytes) })}`} tone={used > 90 ? "critical" : used > 80 ? "warning" : "normal"} />
              <span className="drive-card__meta">{t("spaceLens.driveFree", { free: f.bytes(d.freeBytes), total: f.bytes(d.totalBytes) })}</span>
            </button>
          );
        })}
      </div>
      <Button icon={<FolderOpen20Regular />} onClick={() => void pickFolder()}>
        {t("spaceLens.chooseFolder")}
      </Button>
      <p className="inline-note">{t("spaceLens.readOnly")}</p>
    </>
  );
}

function NodeIcon({ n }: { n: ViewNode }) {
  if (n.denied) return <FolderProhibited20Regular aria-hidden="true" />;
  if (n.kind === "dir") return <Folder20Regular aria-hidden="true" />;
  if (n.kind === "others") return <DocumentMultiple20Regular aria-hidden="true" />;
  return <Document20Regular aria-hidden="true" />;
}

function SidePanel({ view, summary }: { view: SpaceView; summary: SpaceSummary }) {
  const { t } = useTranslation();
  const f = useFormat();
  const { open, reveal } = useSpaceLens();
  const node = view.node;
  const max = node.children[0]?.size || 1;
  const name = (n: ViewNode) => (n.kind === "others" ? t("spaceLens.others", { count: n.files }) : n.name);

  const extTotal = summary.extensions.reduce((s, e) => s + e.bytes, 0) || 1;
  return (
    <aside className="lens__side">
      <section aria-labelledby="largest-title">
        <h2 id="largest-title" className="section-title">
          {t("spaceLens.largest")}
        </h2>
        <ul className="node-list">
          {node.children.map((c) => {
            const row = (
              <>
                <NodeIcon n={c} />
                <span className="node-row__name">{c.denied ? `${name(c)} · ${t("spaceLens.noAccess")}` : name(c)}</span>
                <span className="node-row__size">{f.bytes(c.size)}</span>
                <span className="node-row__bar" aria-hidden="true">
                  <div style={{ width: `${(c.size / max) * 100}%` }} />
                </span>
              </>
            );
            return (
              <li key={c.id} className="node-item">
                {c.kind === "dir" && c.childCount > 0 ? (
                  <button type="button" className="node-row" onClick={() => void open(c.id)}>
                    <span className="sr-only">{t("spaceLens.openPrefix")} </span>
                    {row}
                  </button>
                ) : (
                  <div className="node-row">{row}</div>
                )}
                {c.kind !== "others" && <IconButton label={`${t("spaceLens.reveal")}: ${c.name}`} icon={<Open20Regular />} onClick={() => void reveal(c.id)} />}
              </li>
            );
          })}
          {node.hiddenCount > 0 && (
            <li className="node-row">
              <DocumentMultiple20Regular aria-hidden="true" />
              <span className="node-row__name">{t("spaceLens.hidden", { count: node.hiddenCount })}</span>
              <span className="node-row__size">{f.bytes(node.hiddenSize)}</span>
            </li>
          )}
        </ul>
      </section>
      {summary.extensions.length > 0 && (
        <section aria-labelledby="types-title">
          <h2 id="types-title" className="section-title">
            {t("spaceLens.byType")}
          </h2>
          <ul className="ext-list">
            {summary.extensions.map((e) => (
              <li key={e.ext}>
                <span>{e.ext === "*" ? t("spaceLens.otherTypes") : e.ext === "" ? t("spaceLens.noExt") : `.${e.ext}`}</span>
                <span className="node-row__size">
                  {f.bytes(e.bytes)} · {t("common.files", { count: e.files })}
                </span>
                <span className="node-row__bar" aria-hidden="true">
                  <div style={{ width: `${(e.bytes / extTotal) * 100}%` }} />
                </span>
              </li>
            ))}
          </ul>
        </section>
      )}
    </aside>
  );
}

function Results({ summary, view }: { summary: SpaceSummary; view: SpaceView }) {
  const { t } = useTranslation();
  const f = useFormat();
  const { open, reveal, scan, reset } = useSpaceLens();
  const parent = view.crumbs.length > 1 ? view.crumbs[view.crumbs.length - 2] : null;

  return (
    <div className="lens">
      <div className="lens__bar">
        <IconButton label={t("spaceLens.up")} icon={<ArrowUp20Regular />} disabled={!parent} onClick={() => parent && void open(parent.id)} />
        <nav className="crumbs" aria-label={t("spaceLens.path")}>
          {view.crumbs.map((c, i) => (
            <span key={c.id} style={{ display: "contents" }}>
              {i > 0 && (
                <span className="crumbs__sep" aria-hidden="true">
                  ›
                </span>
              )}
              <button type="button" aria-current={i === view.crumbs.length - 1 ? "location" : undefined} onClick={() => void open(c.id)}>
                {c.name}
              </button>
            </span>
          ))}
        </nav>
        <strong>{f.bytes(view.node.size)}</strong>
        {view.path && (
          <Button variant="subtle" icon={<Open20Regular />} onClick={() => void reveal(view.node.id)}>
            {t("spaceLens.reveal")}
          </Button>
        )}
        <Button onClick={() => void scan(summary.rootPath)}>{t("spaceLens.rescan")}</Button>
        <Button onClick={reset}>{t("spaceLens.newScan")}</Button>
      </div>
      <Treemap node={view.node} onOpen={(id) => void open(id)} />
      <SidePanel view={view} summary={summary} />
      <p className="lens__summary">
        {t("spaceLens.summary", { files: f.number(summary.files), dirs: f.number(summary.dirs), seconds: f.seconds(summary.durationMs) })}
        {summary.denied > 0 && ` · ${t("spaceLens.denied", { count: summary.denied })}`}
      </p>
    </div>
  );
}

export function SpaceLensPage() {
  const { t } = useTranslation();
  const f = useFormat();
  const def = moduleById("space-lens");
  const { phase, target, progress, summary, view, error, cancel, reset } = useSpaceLens();

  if (phase === "scanning") {
    return (
      <div className="page">
        <Scanning
          title={t("spaceLens.scanning", { path: target ?? "" })}
          progress={null}
          detail={progress ? t("spaceLens.counters", { files: f.number(progress.files), size: f.bytes(progress.bytes) }) : undefined}
        >
          {progress?.current && (
            <p className="scan-current" title={progress.current}>
              <bdi>{progress.current}</bdi>
            </p>
          )}
          <div className="flow-actions">
            <Button onClick={() => void cancel()}>{t("spaceLens.stop")}</Button>
          </div>
        </Scanning>
      </div>
    );
  }

  if (phase === "results" && summary && view) {
    return (
      <div className="page" style={{ height: "100%" }}>
        <Results summary={summary} view={view} />
      </div>
    );
  }

  return (
    <div className="page">
      <div className="page__body">
        <ModuleHero icon={def.icon} title={t("modules.space-lens.name")} tagline={t("modules.space-lens.tagline")} />
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
        <DrivePicker />
      </div>
    </div>
  );
}
