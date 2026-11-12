import { ArrowUndo20Regular, CheckmarkCircle20Regular, Shield16Regular, ShoppingBag20Regular, Warning16Regular } from "@fluentui/react-icons";
import type { TFunction } from "i18next";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { moduleById } from "../../app/modules";
import { Button, ConfirmDialog, ErrorNotice, ModuleHero, Segmented } from "../../components";
import { call, toBackendError, type BackendError } from "../../lib/backend";
import type { ApplyReport, RestorePoint, Tweak, TweakCategory, TweaksView } from "../../lib/types";
import { useFormat } from "../../lib/useFormat";
import "../modules.css";

type Filter = "all" | TweakCategory;
const FILTERS: Filter[] = ["all", "privacy", "ads", "ai", "explorer", "apps"];

/** Can be ticked: not already as the tweak wants it, and (for apps) installed. */
function selectable(tw: Tweak): boolean {
  return tw.state === "notApplied" || tw.state === "partial";
}

function StateTag({ tw }: { tw: Tweak }) {
  const { t } = useTranslation();
  if (tw.appx) {
    if (tw.state === "notInstalled") return <span className="tag">{tw.reinstall ? t("tweaks.state.removed") : t("tweaks.state.notInstalled")}</span>;
    return <span className="tag">{t("tweaks.state.installed")}</span>;
  }
  const cls = tw.state === "applied" ? "tag tag--good" : tw.state === "partial" ? "tag tag--caution" : "tag";
  return <span className={cls}>{t(`tweaks.state.${tw.state}`)}</span>;
}

function TweakRow({ tw, checked, busy, onToggle, onUndo }: { tw: Tweak; checked: boolean; busy: boolean; onToggle: () => void; onUndo: () => void }) {
  const { t, i18n } = useTranslation();
  const f = useFormat();
  const lang = i18n.resolvedLanguage?.startsWith("fr") ? "fr" : "en";
  const can = selectable(tw);
  const descId = `tw-${tw.id}-desc`;
  return (
    <li className={`cat-row${can ? "" : " cat-row--disabled"}`}>
      <input type="checkbox" checked={checked && can} disabled={!can || busy} onChange={onToggle} aria-label={tw.title[lang]} aria-describedby={descId} />
      <div className="cat-row__main">
        <div className="cat-row__name">
          {tw.title[lang]}
          <StateTag tw={tw} />
          {tw.risk !== "safe" && <span className={`tag${tw.risk === "caution" ? " tag--critical" : " tag--caution"}`}>{t(`tweaks.risk.${tw.risk}`)}</span>}
        </div>
        <div id={descId} className="cat-row__desc">
          {tw.description[lang]}
        </div>
        {tw.needsAdmin && (
          <div className="cat-row__note">
            <Shield16Regular aria-hidden="true" /> {t("tweaks.needsAdmin")}
          </div>
        )}
        {tw.appliedMs && !tw.appx && <div className="cat-row__note">{t("tweaks.setBySmd", { date: f.dateTime(tw.appliedMs) })}</div>}
      </div>
      <div className="cat-row__side">
        {tw.appliedMs && !tw.appx && tw.state !== "notApplied" && (
          <Button variant="subtle" icon={<ArrowUndo20Regular />} disabled={busy} onClick={onUndo} aria-label={t("tweaks.undoLabel", { name: tw.title[lang] })}>
            {t("tweaks.undo")}
          </Button>
        )}
        {tw.reinstall && tw.state === "notInstalled" && (
          <Button variant="subtle" icon={<ShoppingBag20Regular />} disabled={busy} onClick={onUndo} aria-label={t("tweaks.reinstallLabel", { name: tw.title[lang] })}>
            {t("tweaks.reinstall")}
          </Button>
        )}
      </div>
    </li>
  );
}

function restorePointText(t: TFunction, f: ReturnType<typeof useFormat>, rp: RestorePoint): string {
  switch (rp.status) {
    case "created":
      return t("tweaks.rp.created", { time: rp.timeMs ? f.dateTime(rp.timeMs) : "" });
    case "recent":
      return t("tweaks.rp.recent", { time: rp.timeMs ? f.dateTime(rp.timeMs) : "" });
    case "off":
      return t("tweaks.rp.off");
    case "failed":
      return t("tweaks.rp.failed", { message: rp.message });
  }
}

export function TweaksPage() {
  const { t, i18n } = useTranslation();
  const f = useFormat();
  const def = moduleById("tweaks");
  const lang = i18n.resolvedLanguage?.startsWith("fr") ? "fr" : "en";
  const [view, setView] = useState<TweaksView | null>(null);
  const [error, setError] = useState<BackendError | null>(null);
  const [filter, setFilter] = useState<Filter>("all");
  const [picked, setPicked] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [noPoint, setNoPoint] = useState<ApplyReport | null>(null);
  const [report, setReport] = useState<ApplyReport | null>(null);

  const load = useCallback(() => {
    call("tweaks_list")
      .then((v) => {
        setView(v);
        setError(null);
      })
      .catch((e) => setError(toBackendError(e)));
  }, []);
  useEffect(load, [load]);

  const shown = useMemo(() => (view?.tweaks ?? []).filter((tw) => filter === "all" || tw.category === filter), [view, filter]);
  const chosen = useMemo(() => (view?.tweaks ?? []).filter((tw) => picked.has(tw.id) && selectable(tw)), [view, picked]);
  const adminCount = chosen.filter((tw) => tw.needsAdmin).length;

  const toggle = (id: string) =>
    setPicked((p) => {
      const n = new Set(p);
      if (n.has(id)) n.delete(id);
      else n.add(id);
      return n;
    });

  const apply = async (withoutRestorePoint: boolean, enableRestore: boolean) => {
    setConfirming(false);
    setNoPoint(null);
    setBusy(true);
    try {
      const r = await call("tweaks_apply", { ids: chosen.map((c) => c.id), withoutRestorePoint, enableRestore });
      setView(r.view);
      if (r.needsConfirm) {
        setNoPoint(r);
      } else {
        setReport(r);
        setPicked(new Set());
      }
      setError(null);
    } catch (e) {
      setError(toBackendError(e));
    } finally {
      setBusy(false);
    }
  };

  const undo = async (id: string) => {
    setBusy(true);
    try {
      setView(await call("tweak_undo", { id }));
      setError(null);
    } catch (e) {
      setError(toBackendError(e));
    } finally {
      setBusy(false);
    }
  };

  const titleOf = (id: string) => view?.tweaks.find((x) => x.id === id)?.title[lang] ?? id;

  return (
    <div className="page">
      <div className="page__body">
        <ModuleHero icon={def.icon} title={t("modules.tweaks.name")} tagline={t("modules.tweaks.tagline")} />
        <p className="tool-meta">{t("tweaks.intro")}</p>
        {error && <ErrorNotice error={error} />}
        {report && (
          <div className="activity-message" role="status">
            <p>
              <CheckmarkCircle20Regular className="ok-icon" aria-hidden="true" /> {t("tweaks.applied", { count: report.applied.length })}
            </p>
            {report.restorePoint && <p>{restorePointText(t, f, report.restorePoint)}</p>}
            {report.failed.map((x) => (
              <p key={x.id} className="cat-row__note">
                <Warning16Regular aria-hidden="true" /> {t("tweaks.failedOne", { name: titleOf(x.id), message: x.message })}
              </p>
            ))}
            {report.restartWindows && <p>{t("tweaks.restartWindows")}</p>}
            {report.restartExplorer && (
              <p>
                {t("tweaks.restartExplorerHint")}{" "}
                <Button variant="subtle" onClick={() => void call("restart_explorer").catch((e) => setError(toBackendError(e)))}>
                  {t("tweaks.restartExplorer")}
                </Button>
              </p>
            )}
          </div>
        )}
        {view?.adminSession && (
          <p className="inline-note">
            <Shield16Regular aria-hidden="true" /> {t("admin.sessionOpen")}{" "}
            <Button
              variant="subtle"
              onClick={() =>
                void call("admin_session_end")
                  .then(load)
                  .catch(() => undefined)
              }
            >
              {t("admin.endSession")}
            </Button>
          </p>
        )}
        <Segmented<Filter> label={t("tweaks.filter")} value={filter} onChange={setFilter} options={FILTERS.map((x) => ({ value: x, label: t(`tweaks.category.${x}`) }))} />
        {!view ? (
          !error && <p className="flow-muted">{t("common.loading")}</p>
        ) : (
          <ul className="cat-list">
            {shown.map((tw) => (
              <TweakRow key={tw.id} tw={tw} checked={picked.has(tw.id)} busy={busy} onToggle={() => toggle(tw.id)} onUndo={() => void undo(tw.id)} />
            ))}
          </ul>
        )}
      </div>
      <div className="page__action">
        <Button variant="primary" size="lg" disabled={chosen.length === 0 || busy} onClick={() => setConfirming(true)}>
          {busy ? t("tweaks.applying") : chosen.length ? t("tweaks.apply", { count: chosen.length }) : t("tweaks.nothingSelected")}
        </Button>
        <p className="inline-note">{adminCount > 0 ? t("tweaks.adminNote", { count: adminCount }) : t("tweaks.userNote")}</p>
      </div>
      <ConfirmDialog open={confirming} title={t("tweaks.confirmTitle", { count: chosen.length })} confirmLabel={t("tweaks.confirm")} onCancel={() => setConfirming(false)} onConfirm={() => void apply(false, false)}>
        <ul className="confirm-list">
          {chosen.map((tw) => (
            <li key={tw.id}>
              {tw.title[lang]}
              {tw.needsAdmin ? ` (${t("tweaks.adminShort")})` : ""}
            </li>
          ))}
        </ul>
        {adminCount > 0 && <p>{t("tweaks.confirmAdmin")}</p>}
        <p>{chosen.some((tw) => tw.appx) ? t("tweaks.confirmApps") : t("tweaks.confirmUndo")}</p>
      </ConfirmDialog>
      <ConfirmDialog open={!!noPoint} title={t("tweaks.noPointTitle")} confirmLabel={t("tweaks.continueWithout")} onCancel={() => setNoPoint(null)} onConfirm={() => void apply(true, false)}>
        {noPoint?.restorePoint && <p>{restorePointText(t, f, noPoint.restorePoint)}</p>}
        <p>{t("tweaks.noPointBody")}</p>
        {noPoint?.restorePoint?.status === "off" && (
          <p>
            <Button onClick={() => void apply(false, true)}>{t("tweaks.turnOnRestore")}</Button>
          </p>
        )}
      </ConfirmDialog>
    </div>
  );
}
