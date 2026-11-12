import { Info16Regular, LockClosed16Regular, Shield20Regular, ShieldCheckmark16Regular, Warning16Regular } from "@fluentui/react-icons";
import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { moduleById } from "../../app/modules";
import { Button, CheckList, ConfirmDialog, ErrorNotice, ModuleHero, SideSheet } from "../../components";
import { call } from "../../lib/backend";
import type { BackendSettings, CleanupCategory, CleanupGroup } from "../../lib/types";
import { useFormat } from "../../lib/useFormat";
import { isSelectable, selectedBytes, useCleanup } from "../../stores/cleanup";
import { CleanupDone, Scanning } from "../shared/ScanFlow";
import "../modules.css";

const GROUPS: CleanupGroup[] = ["system", "browsers", "recycleBin"];

function CategoryRow({ c, onReview }: { c: CleanupCategory; onReview: () => void }) {
  const { t } = useTranslation();
  const f = useFormat();
  const { selected, skipped, toggleCategory, scan } = useCleanup();
  const adminScanned = !!scan?.adminScanned;
  const selectable = isSelectable(c, adminScanned);
  const name = t(`cleanup.cat.${c.id}.name`);
  const skip = skipped[c.id]?.length ?? 0;
  const descId = `cat-${c.id}-desc`;

  return (
    <li className={`cat-row${selectable ? "" : " cat-row--disabled"}`}>
      <input type="checkbox" checked={!!selected[c.id] && selectable} disabled={!selectable} onChange={() => toggleCategory(c.id)} aria-label={name} aria-describedby={descId} />
      <div className="cat-row__main">
        <div className="cat-row__name">
          {name}
          <span className={`tag${c.safety === "advanced" ? " tag--caution" : ""}`}>{t(`cleanup.safety.${c.safety}`)}</span>
          {c.permanent && <span className="tag tag--critical">{t("cleanup.permanent")}</span>}
        </div>
        <div id={descId} className="cat-row__desc">
          {t(`cleanup.cat.${c.id}.desc`)}
        </div>
        {c.admin && !adminScanned && (
          <div className="cat-row__note">
            <LockClosed16Regular aria-hidden="true" /> {t("cleanup.needsAdmin")}
          </div>
        )}
        {c.admin && adminScanned && (
          <div className="cat-row__note">
            <ShieldCheckmark16Regular aria-hidden="true" /> {t("cleanup.adminReady")}
          </div>
        )}
        {!c.admin && c.running.length > 0 && c.bytes > 0 && (
          <div className="cat-row__note">
            <Warning16Regular aria-hidden="true" /> {t("cleanup.running", { apps: c.running.join(", ") })}
          </div>
        )}
        {c.unreadable && (
          <div className="cat-row__note">
            <Info16Regular aria-hidden="true" /> {t("cleanup.unreadable")}
          </div>
        )}
        {c.excluded > 0 && (
          <div className="cat-row__note">
            <Info16Regular aria-hidden="true" /> {t("cleanup.excluded", { count: c.excluded })}
          </div>
        )}
      </div>
      <div className="cat-row__side">
        {c.items.length > 0 && (
          <Button variant="subtle" onClick={onReview} aria-label={t("cleanup.reviewTitle", { name })}>
            {t("common.review")}
            {skip > 0 ? ` (−${skip})` : ""}
          </Button>
        )}
        <span className="cat-row__size">{c.bytes > 0 ? f.bytes(c.bytes) : c.unreadable ? t("cleanup.unknownSize") : t("cleanup.empty")}</span>
      </div>
    </li>
  );
}

function ReviewSheet({ category, onClose }: { category: CleanupCategory | null; onClose: () => void }) {
  const { t } = useTranslation();
  const f = useFormat();
  const { skipped, setSkipped } = useCleanup();
  const [note, setNote] = useState<string | null>(null);
  const id = category?.id ?? "";
  const skip = useMemo(() => new Set(skipped[id] ?? []), [skipped, id]);
  const items = useMemo(() => category?.items ?? [], [category]);
  const selected = useMemo(() => new Set(items.filter((i) => !skip.has(i.path)).map((i) => i.path)), [items, skip]);

  const alwaysSkip = async (path: string) => {
    const s = await call("settings_get");
    await call("settings_set", { settings: { ...s, exclusions: [...s.exclusions, path] } });
    setSkipped(id, [...skip, path]);
    setNote(t("cleanup.skipAdded"));
  };

  return (
    <SideSheet
      open={!!category}
      onClose={() => {
        setNote(null);
        onClose();
      }}
      title={category ? t("cleanup.reviewTitle", { name: t(`cleanup.cat.${category.id}.name`) }) : ""}
      footer={
        <Button variant="primary" onClick={onClose}>
          {t("common.done")}
        </Button>
      }
    >
      {category && (
        <>
          <CheckList
            label={t(`cleanup.cat.${category.id}.name`)}
            items={items.map((i) => ({
              id: i.path,
              label: i.name,
              size: f.bytes(i.bytes),
              description: [i.isDir ? t("common.files", { count: i.files }) : null, i.modifiedMs ? f.dateTime(i.modifiedMs) : null].filter(Boolean).join(" · "),
            }))}
            selected={selected}
            onChange={(next) => setSkipped(id, items.filter((i) => !next.has(i.path)).map((i) => i.path))}
            action={(item) => (
              <Button variant="subtle" onClick={() => void alwaysSkip(item.id)} aria-label={t("cleanup.alwaysSkipLabel", { name: item.label })}>
                {t("cleanup.alwaysSkip")}
              </Button>
            )}
          />
          {category.moreItems > 0 && <p className="review-more">{t("cleanup.more", { count: category.moreItems, size: f.bytes(category.moreBytes) })}</p>}
          {note && (
            <p className="review-more" role="status">
              {note}
            </p>
          )}
        </>
      )}
    </SideSheet>
  );
}

export function CleanupPage() {
  const { t } = useTranslation();
  const f = useFormat();
  const def = moduleById("cleanup");
  const { phase, progress, scan, selected, skipped, report, error, startScan, run, reset, adminScan, adminBusy, adminError } = useCleanup();
  const [reviewing, setReviewing] = useState<string | null>(null);
  const [confirming, setConfirming] = useState(false);
  const [settings, setSettings] = useState<BackendSettings | null>(null);

  useEffect(() => {
    call("settings_get")
      .then(setSettings)
      .catch(() => setSettings(null));
  }, [phase]);

  const total = selectedBytes(scan, selected, skipped);
  const bin = scan?.categories.find((c) => c.permanent);
  const binSelected = !!bin && !!selected[bin.id] && isSelectable(bin, scan?.adminScanned);
  const hasAdmin = !!scan?.categories.some((c) => c.admin);
  const permanentMode = settings?.deleteMode === "permanent";
  const stepName = progress?.step ? t(`cleanup.cat.${progress.step}.name`) : "";

  const onRun = () => {
    if (binSelected || permanentMode) setConfirming(true);
    else void run();
  };

  if (phase === "scanning") {
    return (
      <div className="page">
        <Scanning title={t("cleanup.scanning")} progress={progress} detail={stepName ? t("cleanup.stepRule", { name: stepName }) : undefined} />
      </div>
    );
  }
  if (phase === "running") {
    return (
      <div className="page">
        <Scanning title={t("cleanup.cleaning")} progress={null} />
      </div>
    );
  }
  if (phase === "done" && report) {
    return (
      <div className="page">
        <CleanupDone report={report} title={t("cleanup.doneTitle")}>
          <div className="flow-actions">
            <Button variant="primary" size="lg" onClick={() => void startScan()}>
              {t("common.scanAgain")}
            </Button>
          </div>
        </CleanupDone>
      </div>
    );
  }

  if (phase === "results" && scan) {
    const reviewCat = scan.categories.find((c) => c.id === reviewing) ?? null;
    const confirmBody = binSelected
      ? t("cleanup.confirmBinBody", { size: f.bytes(bin?.bytes ?? 0) })
      : t("cleanup.confirmPermanentBody", { size: f.bytes(total) });
    return (
      <div className="page">
        <div className="page__body">
          <div className="results">
            <div className="results__head">
              <div>
                <h1>{scan.reclaimableBytes > 0 || (bin?.bytes ?? 0) > 0 ? `${f.bytes(total)} ${t("cleanup.found")}` : t("cleanup.foundNone")}</h1>
                <div className="results__meta">{t("common.scannedIn", { seconds: f.seconds(scan.durationMs) })}</div>
              </div>
              {hasAdmin && !scan.adminScanned && (
                <div className="admin-offer">
                  <Button icon={<Shield20Regular />} disabled={adminBusy} onClick={() => void adminScan()}>
                    {adminBusy ? t("admin.waiting") : t("cleanup.adminScan")}
                  </Button>
                  <p className="inline-note">{t("cleanup.adminScanWhy")}</p>
                </div>
              )}
            </div>
            {adminError && <ErrorNotice error={adminError} />}
            {GROUPS.map((g) => {
              const cats = scan.categories.filter((c) => c.group === g);
              if (cats.length === 0) return null;
              return (
                <section key={g} className="cat-group" aria-labelledby={`group-${g}`}>
                  <h2 id={`group-${g}`} className="section-title">
                    {t(`cleanup.group.${g}`)}
                  </h2>
                  <ul className="cat-list">
                    {cats.map((c) => (
                      <CategoryRow key={c.id} c={c} onReview={() => setReviewing(c.id)} />
                    ))}
                  </ul>
                </section>
              );
            })}
          </div>
        </div>
        <div className="page__action">
          <Button variant="primary" size="lg" disabled={total === 0} onClick={onRun}>
            {total > 0 ? t("cleanup.clean", { size: f.bytes(total) }) : t("cleanup.nothingSelected")}
          </Button>
          <p className="inline-note">{permanentMode ? t("cleanup.permanentNote") : settings?.quarantine ? t("cleanup.quarantineNote", { days: settings.quarantineDays }) : t("cleanup.recycleNote")}</p>
        </div>
        <ReviewSheet category={reviewCat} onClose={() => setReviewing(null)} />
        <ConfirmDialog
          open={confirming}
          title={binSelected ? t("cleanup.confirmBinTitle") : t("cleanup.confirmPermanentTitle")}
          confirmLabel={t("cleanup.confirm")}
          onCancel={() => setConfirming(false)}
          onConfirm={() => {
            setConfirming(false);
            void run();
          }}
        >
          <p>{confirmBody}</p>
          {binSelected && permanentMode && <p>{t("cleanup.permanentNote")}</p>}
        </ConfirmDialog>
      </div>
    );
  }

  return (
    <div className="page">
      <div className="page__body">
        <ModuleHero icon={def.icon} title={t("modules.cleanup.name")} tagline={t("modules.cleanup.tagline")} />
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
        <Button variant="primary" size="lg" onClick={() => void startScan()}>
          {t("common.scan")}
        </Button>
      </div>
    </div>
  );
}
