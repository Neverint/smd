import {
  ArrowSync20Regular,
  CheckmarkCircle16Regular,
  DismissCircle16Regular,
  Edit20Regular,
  HardDrive24Regular,
  Info16Regular,
  Open20Regular,
  Warning16Regular,
} from "@fluentui/react-icons";
import type { TFunction } from "i18next";
import { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Link } from "react-router";
import { Button, ConfirmDialog, ErrorNotice, ProgressRing, SideSheet, Tabs, UsageBar } from "../../components";
import { call, stream, toBackendError, type BackendError } from "../../lib/backend";
import type { AuditView, DownloadsView, Finding, MoveProgress, MoveResult, PlanStep, PlanView, Policy, QuarantineBatch, Role } from "../../lib/types";
import { useFormat } from "../../lib/useFormat";
import "../modules.css";

type Tab = "layout" | "quarantine";
const ROLES: Role[] = ["system", "heavyGames", "lightGames", "disposable", "unassigned"];

function methodText(t: TFunction, f: Finding): string {
  switch (f.method.kind) {
    case "launcherMove":
      return t(`organizer.method.launcherMove.${f.method.launcher}`);
    case "reinstall":
      return t("organizer.method.reinstall", { launcher: t(`organizer.launcher.${f.method.launcher}`) });
    case "copyVerify":
      return t("organizer.method.copyVerify");
    case "relocateCache":
      return t("organizer.method.relocateCache", { variable: f.method.envVar });
  }
}

function RolesSheet({ open, audit, onClose, onSaved }: { open: boolean; audit: AuditView; onClose: () => void; onSaved: (a: AuditView) => void }) {
  const { t } = useTranslation();
  const [draft, setDraft] = useState<Policy>(audit.policy);
  const [error, setError] = useState<BackendError | null>(null);
  const letters = useMemo(() => Array.from(new Set([...audit.drives.map((d) => d.letter), ...draft.drives.map((d) => d.letter)])).sort(), [audit, draft]);

  const setRole = (letter: string, role: Role) => {
    const others = draft.drives.filter((d) => d.letter !== letter);
    const existing = draft.drives.find((d) => d.letter === letter);
    setDraft({ ...draft, drives: role === "unassigned" ? others : [...others, { ...(existing ?? { letter }), letter, role }] });
  };

  const save = async () => {
    try {
      onSaved(await call("organizer_policy_set", { policy: draft }));
      onClose();
    } catch (e) {
      setError(toBackendError(e));
    }
  };

  return (
    <SideSheet
      open={open}
      onClose={onClose}
      title={t("organizer.rolesTitle")}
      footer={
        <Button variant="primary" onClick={() => void save()}>
          {t("organizer.saveRoles")}
        </Button>
      }
    >
      <p className="review-more">{t("organizer.rolesHint")}</p>
      {error && <ErrorNotice error={error} />}
      <ul className="rows rows--plain">
        {letters.map((l) => (
          <li key={l} className="row">
            <span className="row__title" style={{ minWidth: 40 }}>
              {l}
            </span>
            <select className="search" aria-label={t("organizer.roleFor", { letter: l })} value={draft.drives.find((d) => d.letter === l)?.role ?? "unassigned"} onChange={(e) => setRole(l, e.target.value as Role)}>
              {ROLES.map((r) => (
                <option key={r} value={r}>
                  {t(`organizer.role.${r}`)}
                </option>
              ))}
            </select>
          </li>
        ))}
      </ul>
      <label className="row">
        <span className="row__main">{t("organizer.threshold")}</span>
        <input
          className="search"
          style={{ minWidth: 0, width: 90 }}
          type="number"
          min={1}
          max={2000}
          value={draft.gameThresholdGb}
          onChange={(e) => setDraft({ ...draft, gameThresholdGb: Math.max(1, Number(e.target.value) || 1) })}
        />
      </label>
    </SideSheet>
  );
}

function PlanPanel({ plan, onBack, onDone }: { plan: PlanView; onBack: () => void; onDone: (r: MoveResult) => void }) {
  const { t } = useTranslation();
  const f = useFormat();
  const [confirming, setConfirming] = useState(false);
  const [progress, setProgress] = useState<MoveProgress | null>(null);
  const [error, setError] = useState<BackendError | null>(null);
  const runnable = plan.steps.filter((s) => s.ready && s.runsInApp);

  const run = async () => {
    setConfirming(false);
    setProgress({ step: 0, steps: runnable.length, bytesDone: 0, bytesTotal: plan.readyBytes });
    try {
      onDone(await stream("organizer_run", { planId: plan.planId, findingIds: runnable.map((s) => s.finding.id) }, setProgress));
    } catch (e) {
      setError(toBackendError(e));
    } finally {
      setProgress(null);
    }
  };

  if (progress) {
    const pct = progress.bytesTotal > 0 ? (progress.bytesDone / progress.bytesTotal) * 100 : undefined;
    return (
      <div className="flow-center" aria-live="polite">
        <ProgressRing value={pct} size={160} thickness={12} label={t("organizer.moving")}>
          {pct !== undefined && <span className="flow-ring-value">{Math.round(pct)} %</span>}
        </ProgressRing>
        <h2 className="flow-title">{t("organizer.moving")}</h2>
        <p className="flow-step">{t("organizer.movingLine", { done: f.bytes(progress.bytesDone), total: f.bytes(progress.bytesTotal) })}</p>
        <p className="flow-muted">{t("organizer.movingHint")}</p>
        <Button onClick={() => void call("organizer_cancel")}>{t("organizer.pause")}</Button>
      </div>
    );
  }

  const checkIcon = (ok: boolean) => (ok ? <CheckmarkCircle16Regular className="ok-icon" aria-hidden="true" /> : <DismissCircle16Regular className="warn-icon" aria-hidden="true" />);
  const step = (s: PlanStep, i: number) => (
    <li key={s.finding.id} className="plan-step">
      <div className="plan-step__head">
        <span className="plan-step__n">{i + 1}</span>
        <div className="row__main">
          <span className="row__title">{s.finding.name}</span>
          <span className="row__sub row__mono" title={s.finding.path}>
            {s.finding.path}
          </span>
          {s.target && (
            <span className="row__sub row__mono" title={s.target}>
              → {s.target}
            </span>
          )}
          <span className="row__sub">
            {methodText(t, s.finding)} · {s.runsInApp ? t("organizer.estimate", { minutes: Math.max(1, Math.round(s.estSeconds / 60)) }) : t("organizer.guided")}
          </span>
        </div>
        <span className="row__size">{f.bytes(s.finding.bytes)}</span>
      </div>
      <ul className="checks">
        {s.checks
          // A launcher moves its own games: only role, space and health matter for those steps.
          .filter((c) => s.runsInApp || ["targetRole", "freeSpace", "sourceHealth", "targetHealth"].includes(c.kind))
          .map((c) => (
            <li key={c.kind} className={c.ok ? "" : "checks__bad"}>
              {checkIcon(c.ok)} {t(c.ok ? `organizer.check.${c.kind}` : `organizer.checkBad.${c.kind}`, { detail: c.kind === "freeSpace" ? f.bytes(Number(c.detail)) : c.detail })}
            </li>
          ))}
      </ul>
      {!s.runsInApp && (
        <div className="guide">
          <p>{t(`organizer.guide.${s.finding.method.kind === "launcherMove" ? s.finding.method.launcher : "reinstall"}`, { target: s.finding.targetDrive ?? "" })}</p>
          {s.finding.method.kind === "launcherMove" && s.finding.method.launcher === "steam" && (
            <Button icon={<Open20Regular />} onClick={() => void call("open_system_tool", { tool: "steam" })}>
              {t("organizer.openSteam")}
            </Button>
          )}
          {s.finding.method.kind !== "launcherMove" || s.finding.method.launcher === "xbox" ? (
            <Button icon={<Open20Regular />} onClick={() => void call("open_system_tool", { tool: "installedApps" })}>
              {t("organizer.openApps")}
            </Button>
          ) : null}
        </div>
      )}
    </li>
  );

  return (
    <div className="tool-section">
      <div className="toolbar">
        <h2 className="section-title" style={{ flex: 1 }}>
          {t("organizer.planTitle")}
        </h2>
        <Button onClick={onBack}>{t("common.back")}</Button>
      </div>
      <p className="tool-meta">{t("organizer.planHint")}</p>
      {error && <ErrorNotice error={error} />}
      <ol className="plan">{plan.steps.map(step)}</ol>
      <div className="page__action">
        <Button variant="primary" size="lg" disabled={runnable.length === 0} onClick={() => setConfirming(true)}>
          {runnable.length > 0 ? t("organizer.move", { count: runnable.length, size: f.bytes(plan.readyBytes) }) : t("organizer.nothingToRun")}
        </Button>
        {runnable.length > 0 && <p className="inline-note">{t("organizer.estimateTotal", { minutes: Math.max(1, Math.round(plan.estSeconds / 60)) })}</p>}
      </div>
      <ConfirmDialog open={confirming} title={t("organizer.confirmTitle")} confirmLabel={t("organizer.confirm")} onCancel={() => setConfirming(false)} onConfirm={() => void run()}>
        <p>{t("organizer.confirmBody")}</p>
        <ul>
          {runnable.map((s) => (
            <li key={s.finding.id}>
              {s.finding.name} → {s.target}
            </li>
          ))}
        </ul>
      </ConfirmDialog>
    </div>
  );
}

function LayoutTab({ audit, onAudit }: { audit: AuditView; onAudit: (a: AuditView) => void }) {
  const { t } = useTranslation();
  const f = useFormat();
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [plan, setPlan] = useState<PlanView | null>(null);
  const [result, setResult] = useState<MoveResult | null>(null);
  const [roles, setRoles] = useState(false);
  const [error, setError] = useState<BackendError | null>(null);

  const makePlan = async () => {
    try {
      setPlan(await call("organizer_plan", { ids: [...selected] }));
    } catch (e) {
      setError(toBackendError(e));
    }
  };

  const resume = async (id: number) => {
    try {
      setResult(await stream("organizer_resume", { journalId: id }, () => undefined));
    } catch (e) {
      setError(toBackendError(e));
    }
  };

  if (plan && !result) {
    return <PlanPanel plan={plan} onBack={() => setPlan(null)} onDone={(r) => setResult(r)} />;
  }
  if (result) {
    const done = result.steps.filter((s) => s.state === "done");
    return (
      <div className="tool-section">
        <h2 className="section-title">{result.cancelled ? t("organizer.paused") : t("organizer.doneTitle")}</h2>
        <ul className="rows">
          {result.steps.map((s) => (
            <li key={s.findingId} className="row">
              {s.state === "done" ? <CheckmarkCircle16Regular className="ok-icon" aria-hidden="true" /> : <Warning16Regular className="warn-icon" aria-hidden="true" />}
              <div className="row__main">
                <span className="row__title">{s.name}</span>
                <span className="row__sub row__mono">→ {s.target}</span>
                <span className="row__sub">{s.state === "done" ? t("organizer.stepDone") : (s.error ?? t(`organizer.state.${s.state}`))}</span>
              </div>
              <span className="row__size">{f.bytes(s.bytes)}</span>
            </li>
          ))}
        </ul>
        {done.length > 0 && result.undoAvailable && (
          <p className="flow-muted">
            {t("organizer.undoHint")} <Link to="/activity">{t("cleanup.openActivity")}</Link>
          </p>
        )}
        <div>
          <Button
            variant="primary"
            onClick={() => {
              setResult(null);
              setPlan(null);
              setSelected(new Set());
              void call("organizer_audit").then(onAudit);
            }}
          >
            {t("organizer.auditAgain")}
          </Button>
        </div>
      </div>
    );
  }

  const problems = audit.findings.filter((x) => x.severity !== "info");
  const infos = audit.findings.filter((x) => x.severity === "info");
  const row = (x: Finding) => (
    <li key={x.id} className="row">
      <input
        type="checkbox"
        checked={selected.has(x.id)}
        aria-label={x.name}
        onChange={() => {
          const next = new Set(selected);
          if (next.has(x.id)) next.delete(x.id);
          else next.add(x.id);
          setSelected(next);
        }}
      />
      <div className="row__main">
        <span className="row__title">
          {x.name}
          <span className={`tag${x.severity === "problem" ? " tag--critical" : x.severity === "warning" ? " tag--caution" : ""}`}>{t(`organizer.severity.${x.severity}`)}</span>
        </span>
        <span className="row__sub">{t(`organizer.reason.${x.reason}`, { from: x.drive, to: x.targetDrive ?? "—" })}</span>
        <span className="row__sub row__mono" title={x.path}>
          {x.path}
        </span>
        <span className="row__sub">{methodText(t, x)}</span>
      </div>
      <span className="row__size">{f.bytes(x.bytes)}</span>
    </li>
  );

  return (
    <div className="tool-section">
      {error && <ErrorNotice error={error} />}
      {audit.notices.map((n, i) => (
        <p key={i} className="cat-row__note">
          <Info16Regular aria-hidden="true" /> {n.kind === "letterChanged" ? t("organizer.notice.letterChanged", { from: n.from, to: n.to }) : t("organizer.notice.missing", { letter: n.letter })}
        </p>
      ))}
      {audit.unfinished.map((j) => (
        <div key={j.id} className="tool-card tool-card--warn">
          <span>{t("organizer.unfinished", { done: j.done, total: j.steps, size: f.bytes(j.bytes) })}</span>
          <div>
            <Button variant="primary" onClick={() => void resume(j.id)}>
              {t("organizer.resume")}
            </Button>
          </div>
        </div>
      ))}

      <div className="drive-grid drive-grid--wide">
        {audit.drives.map((d) => {
          const used = d.totalBytes ? 100 - (d.freeBytes / d.totalBytes) * 100 : 0;
          return (
            <section key={d.letter} className="drive-card" aria-label={`${d.letter} ${t(`organizer.role.${d.role}`)}`}>
              <span className="drive-card__title">
                <HardDrive24Regular aria-hidden="true" /> {d.letter} {d.label}
              </span>
              <span className={`tag role-tag role-tag--${d.role}`}>{t(`organizer.role.${d.role}`)}</span>
              <UsageBar value={used} label={d.letter} tone={d.role === "system" && used > 85 ? "critical" : d.role === "system" && used > 75 ? "warning" : "normal"} />
              <span className="drive-card__meta">{t("spaceLens.driveFree", { free: f.bytes(d.freeBytes), total: f.bytes(d.totalBytes) })}</span>
              <span className="drive-card__meta">
                {[d.diskName, d.health ? t(`hardware.health.${d.health}`) : null].filter(Boolean).join(" · ")}
              </span>
            </section>
          );
        })}
      </div>
      <div className="toolbar">
        <Button icon={<Edit20Regular />} onClick={() => setRoles(true)}>
          {t("organizer.editRoles")}
        </Button>
        <Button variant="subtle" icon={<Open20Regular />} onClick={() => void call("open_system_tool", { tool: "saveLocations" })}>
          {t("organizer.saveLocations")}
        </Button>
      </div>

      <section aria-labelledby="misplaced-title">
        <h2 id="misplaced-title" className="section-title">
          {problems.length ? t("organizer.misplaced", { count: problems.length, size: f.bytes(audit.misplacedBytes) }) : t("organizer.allGood")}
        </h2>
        {problems.length > 0 && <ul className="rows">{problems.map(row)}</ul>}
      </section>
      {infos.length > 0 && (
        <section aria-labelledby="info-title">
          <h2 id="info-title" className="section-title">
            {t("organizer.couldFitBetter")}
          </h2>
          <ul className="rows">{infos.map(row)}</ul>
        </section>
      )}
      <div className="page__action">
        <Button variant="primary" size="lg" disabled={selected.size === 0} onClick={() => void makePlan()}>
          {selected.size ? t("organizer.plan", { count: selected.size }) : t("organizer.selectToPlan")}
        </Button>
        <p className="inline-note">{t("organizer.planNote")}</p>
      </div>
      {roles && <RolesSheet open={roles} audit={audit} onClose={() => setRoles(false)} onSaved={onAudit} />}
    </div>
  );
}

function QuarantineTab({ audit, onAudit }: { audit: AuditView; onAudit: (a: AuditView) => void }) {
  const { t } = useTranslation();
  const f = useFormat();
  const [batches, setBatches] = useState<QuarantineBatch[] | null>(null);
  const [scratch, setScratch] = useState<DownloadsView | null>(null);
  const [scratchSel, setScratchSel] = useState<Set<string>>(new Set());
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<BackendError | null>(null);
  const q = audit.quarantine;

  const load = useCallback(() => {
    call("quarantine_list")
      .then(setBatches)
      .catch((e) => setError(toBackendError(e)));
    call("clutter_scratch")
      .then(setScratch)
      .catch(() => setScratch(null));
  }, []);
  useEffect(load, [load]);

  const restore = async (b: QuarantineBatch) => {
    try {
      const r = await call("quarantine_restore", { manifest: b.manifest });
      setMessage(t("organizer.restored", { count: r.restored }) + (r.conflicts ? ` ${t("organizer.conflicts", { count: r.conflicts })}` : ""));
      load();
    } catch (e) {
      setError(toBackendError(e));
    }
  };

  const consent = async (allow: boolean) => {
    const freed = await call("quarantine_purge_consent", { allow }).catch(() => 0);
    if (allow) setMessage(t("organizer.purged", { size: f.bytes(freed) }));
    onAudit(await call("organizer_audit"));
    load();
  };

  const removeScratch = async () => {
    if (!scratch) return;
    try {
      const r = await call("clutter_remove", { scanId: scratch.scanId, paths: [...scratchSel] });
      setMessage(t("organizer.scratchRemoved", { size: f.bytes(r.removedBytes) }));
      setScratchSel(new Set());
      load();
    } catch (e) {
      setError(toBackendError(e));
    }
  };

  return (
    <div className="tool-section">
      {error && <ErrorNotice error={error} />}
      {message && (
        <p className="activity-message" role="status">
          {message}
        </p>
      )}
      <section className="tool-card">
        <h2 className="section-title">{t("organizer.quarantineTitle", { drive: q.drive ?? "S:" })}</h2>
        <p className="tool-meta">{q.active ? t("organizer.quarantineOn", { days: q.days }) : t("organizer.quarantineOff")}</p>
        <p>{t("organizer.quarantineSize", { size: f.bytes(q.bytes), count: q.batches })}</p>
        {q.oldBytes > 0 && q.purgeConsent === null && (
          <div className="tool-card tool-card--warn">
            <p>{t("organizer.purgeQuestion", { size: f.bytes(q.oldBytes), days: q.days })}</p>
            <div className="toolbar">
              <Button variant="primary" onClick={() => void consent(true)}>
                {t("organizer.purgeYes")}
              </Button>
              <Button onClick={() => void consent(false)}>{t("organizer.purgeNo")}</Button>
            </div>
          </div>
        )}
      </section>
      {batches && batches.length > 0 && (
        <ul className="rows">
          {batches.map((b) => (
            <li key={b.manifest} className="row">
              <div className="row__main">
                <span className="row__title">{f.dateTime(b.createdMs)}</span>
                <span className="row__sub">
                  {t("common.items", { count: b.items })} · {b.sample.join(", ")}
                </span>
              </div>
              <span className="row__size">{f.bytes(b.bytes)}</span>
              <Button icon={<ArrowSync20Regular />} onClick={() => void restore(b)}>
                {t("organizer.restore")}
              </Button>
            </li>
          ))}
        </ul>
      )}
      <section className="tool-card">
        <h2 className="section-title">{t("organizer.scratchTitle", { path: scratch?.path ?? `${q.drive ?? "S:"}\\Scratch` })}</h2>
        <p className="tool-meta">{t("organizer.scratchHint")}</p>
        {scratch && scratch.items.length === 0 && <p className="flow-muted">{t("organizer.scratchEmpty")}</p>}
        {scratch && scratch.items.length > 0 && (
          <>
            <ul className="rows rows--plain">
              {scratch.items.map((i) => (
                <li key={i.path} className="row">
                  <input type="checkbox" checked={scratchSel.has(i.path)} aria-label={i.name} onChange={() => setScratchSel((s) => (s.has(i.path) ? new Set([...s].filter((x) => x !== i.path)) : new Set([...s, i.path])))} />
                  <div className="row__main">
                    <span className="row__title">{i.name}</span>
                    <span className="row__sub">{i.modifiedMs ? t("clutter.lastChanged", { date: f.dateTime(i.modifiedMs) }) : ""}</span>
                  </div>
                  <span className="row__size">{f.bytes(i.bytes)}</span>
                </li>
              ))}
            </ul>
            <div>
              <Button disabled={scratchSel.size === 0} onClick={() => void removeScratch()}>
                {t("organizer.scratchRemove", { count: scratchSel.size })}
              </Button>
            </div>
          </>
        )}
      </section>
    </div>
  );
}

export function DriveOrganizerPage() {
  const { t } = useTranslation();
  const f = useFormat();
  const [audit, setAudit] = useState<AuditView | null>(null);
  const [error, setError] = useState<BackendError | null>(null);
  const [tab, setTab] = useState<Tab>("layout");

  const load = useCallback(() => {
    call("organizer_audit")
      .then((a) => {
        setAudit(a);
        setError(null);
      })
      .catch((e) => setError(toBackendError(e)));
  }, []);
  useEffect(load, [load]);

  return (
    <div className="tool">
      <header className="tool__head">
        <h1>{t("modules.drive-organizer.name")}</h1>
        <p className="tool-meta">{t("modules.drive-organizer.tagline")}</p>
        {audit && (
          <p className="tool-meta">
            {t(`organizer.status.${audit.status}`)} · {t("organizer.auditLine", { games: audit.games, seconds: f.seconds(audit.durationMs) })}
          </p>
        )}
      </header>
      {error && <ErrorNotice error={error} />}
      {!audit && !error && (
        <div className="flow-center">
          <ProgressRing size={96} label={t("organizer.auditing")} />
          <p className="flow-step">{t("organizer.auditing")}</p>
        </div>
      )}
      {audit && (
        <Tabs<Tab>
          label={t("modules.drive-organizer.name")}
          value={tab}
          onChange={setTab}
          tabs={[
            { id: "layout", label: t("organizer.tabs.layout"), count: audit.findings.filter((x) => x.severity !== "info").length },
            { id: "quarantine", label: t("organizer.tabs.quarantine") },
          ]}
        >
          {tab === "layout" ? <LayoutTab audit={audit} onAudit={setAudit} /> : <QuarantineTab audit={audit} onAudit={setAudit} />}
        </Tabs>
      )}
    </div>
  );
}
