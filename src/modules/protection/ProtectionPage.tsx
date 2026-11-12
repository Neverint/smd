import { CheckmarkCircle20Regular, Open20Regular, ShieldCheckmark48Regular, ShieldError48Regular, Warning20Regular } from "@fluentui/react-icons";
import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button, ErrorNotice, ProgressRing, Segmented, Tabs, Toggle } from "../../components";
import { call, stream, toBackendError, type BackendError } from "../../lib/backend";
import type { Capability, DefenderScanResult, DefenderStatus, ProtectionView, ScanKind } from "../../lib/types";
import { useFormat } from "../../lib/useFormat";
import "../modules.css";

type Tab = "defender" | "firewall" | "permissions";

function overall(view: ProtectionView): "good" | "warning" | "critical" | "unknown" {
  const d = view.defender;
  if (!d) return "unknown";
  const active = (view.threats ?? []).some((t) => t.active);
  const passive = /passive|edr/i.test(d.runningMode);
  if (active || (!d.realTime && !passive)) return "critical";
  if (!passive && (d.signatureAgeDays ?? 0) >= 3) return "warning";
  if ((view.firewall ?? []).some((f) => !f.enabled)) return "warning";
  return "good";
}

function DefenderTab({ view, onChanged }: { view: ProtectionView; onChanged: (d: DefenderStatus | null) => void }) {
  const { t } = useTranslation();
  const f = useFormat();
  const [scanning, setScanning] = useState<ScanKind | null>(null);
  const [elapsed, setElapsed] = useState(0);
  const [result, setResult] = useState<DefenderScanResult | null>(null);
  const [updating, setUpdating] = useState(false);
  const [error, setError] = useState<BackendError | null>(null);
  const [kind, setKind] = useState<ScanKind>("quick");
  const d = view.defender;

  const scan = async () => {
    setScanning(kind);
    setElapsed(0);
    setResult(null);
    try {
      setResult(await stream("protection_scan", { kind }, (p) => setElapsed(p.elapsedMs)));
    } catch (e) {
      setError(toBackendError(e));
    } finally {
      setScanning(null);
    }
  };

  const updateSignatures = async () => {
    setUpdating(true);
    try {
      onChanged(await call("protection_update_signatures"));
    } catch (e) {
      setError(toBackendError(e));
    } finally {
      setUpdating(false);
    }
  };

  if (!d) {
    return (
      <div className="tool-card">
        <p>{t("protection.noDefender")}</p>
        <div>
          <Button icon={<Open20Regular />} onClick={() => void call("open_system_tool", { tool: "windowsSecurity" })}>
            {t("protection.openSecurity")}
          </Button>
        </div>
      </div>
    );
  }

  const threats = result?.threats ?? view.threats ?? [];
  const passive = /passive|edr/i.test(d.runningMode);
  return (
    <div className="tool-section">
      {error && <ErrorNotice error={error} />}
      <section className="tool-card">
        <ul className="facts">
          <li>
            {d.realTime || passive ? <CheckmarkCircle20Regular className="ok-icon" /> : <Warning20Regular className="warn-icon" />}
            {passive ? t("protection.passive") : d.realTime ? t("protection.realTimeOn") : t("protection.realTimeOff")}
          </li>
          <li>
            {(d.signatureAgeDays ?? 0) < 3 ? <CheckmarkCircle20Regular className="ok-icon" /> : <Warning20Regular className="warn-icon" />}
            {d.signaturesUpdatedMs ? t("protection.signatures", { date: f.dateTime(d.signaturesUpdatedMs) }) : t("protection.signaturesUnknown")}
          </li>
          {d.tamperProtected !== null && (
            <li>
              {d.tamperProtected ? <CheckmarkCircle20Regular className="ok-icon" /> : <Warning20Regular className="warn-icon" />}
              {d.tamperProtected ? t("protection.tamperOn") : t("protection.tamperOff")}
            </li>
          )}
          <li className="facts__muted">
            {t("protection.lastQuick", { date: d.quickScanEndMs ? f.dateTime(d.quickScanEndMs) : t("common.never") })} ·{" "}
            {t("protection.lastFull", { date: d.fullScanEndMs ? f.dateTime(d.fullScanEndMs) : t("common.never") })}
          </li>
        </ul>
        <div className="toolbar">
          <Segmented<ScanKind>
            label={t("protection.scanType")}
            value={kind}
            onChange={setKind}
            options={[
              { value: "quick", label: t("protection.scanKind.quick") },
              { value: "full", label: t("protection.scanKind.full") },
            ]}
          />
          <Button variant="primary" disabled={scanning !== null} onClick={() => void scan()}>
            {t("protection.scanNow")}
          </Button>
          <Button disabled={updating || scanning !== null} onClick={() => void updateSignatures()}>
            {updating ? t("protection.updating") : t("protection.updateSignatures")}
          </Button>
          <Button variant="subtle" icon={<Open20Regular />} onClick={() => void call("open_system_tool", { tool: "windowsSecurity" })}>
            {t("protection.openSecurity")}
          </Button>
        </div>
        <p className="tool-meta">{t(kind === "quick" ? "protection.quickHint" : "protection.fullHint")}</p>
      </section>

      {scanning && (
        <div className="flow-center" aria-live="polite">
          <ProgressRing size={120} label={t("protection.scanning", { kind: t(`protection.scanKind.${scanning}`) })} />
          <p className="flow-step">
            {t("protection.scanning", { kind: t(`protection.scanKind.${scanning}`) })} · {f.seconds(elapsed)} s
          </p>
          <Button onClick={() => void call("protection_cancel_scan")}>{t("common.stop")}</Button>
        </div>
      )}
      {result && !scanning && (
        <p className="activity-message" role="status">
          {result.cancelled ? t("protection.scanStopped") : result.threatsFound ? t("protection.scanFound") : t("protection.scanClean", { seconds: f.seconds(result.durationMs) })}
        </p>
      )}

      <section aria-labelledby="threats-title">
        <h2 id="threats-title" className="section-title">
          {t("protection.history")}
        </h2>
        {threats.length === 0 ? (
          <p className="flow-muted">{t("protection.noThreats")}</p>
        ) : (
          <ul className="rows">
            {threats.map((th, i) => (
              <li key={`${th.name}-${i}`} className="row">
                {th.active ? <Warning20Regular className="warn-icon" aria-hidden="true" /> : <CheckmarkCircle20Regular className="ok-icon" aria-hidden="true" />}
                <div className="row__main">
                  <span className="row__title">
                    {th.name}
                    {th.severity && <span className="tag">{t(`protection.severity.${th.severity}`, { defaultValue: th.severity })}</span>}
                  </span>
                  <span className="row__sub">
                    {t(`protection.threatStatus.${th.status}`, { defaultValue: th.status })}
                    {th.detectedMs ? ` · ${f.dateTime(th.detectedMs)}` : ""}
                  </span>
                  {th.resources.slice(0, 2).map((r) => (
                    <span key={r} className="row__sub row__mono" title={r}>
                      {r.replace(/^file:_/, "")}
                    </span>
                  ))}
                </div>
              </li>
            ))}
          </ul>
        )}
      </section>
    </div>
  );
}

function FirewallTab({ view }: { view: ProtectionView }) {
  const { t } = useTranslation();
  if (!view.firewall) return <p className="flow-muted">{t("performance.unavailable")}</p>;
  return (
    <div className="tool-section">
      <p className="tool-meta">{t("protection.firewallHint")}</p>
      <ul className="rows">
        {view.firewall.map((p) => (
          <li key={p.name} className="row">
            {p.enabled ? <CheckmarkCircle20Regular className="ok-icon" aria-hidden="true" /> : <Warning20Regular className="warn-icon" aria-hidden="true" />}
            <div className="row__main">
              <span className="row__title">{t(`protection.profile.${p.name}`, { defaultValue: p.name })}</span>
              <span className="row__sub">{t(`protection.profileHint.${p.name}`, { defaultValue: "" })}</span>
            </div>
            <span className="row__size">{p.enabled ? t("protection.on") : t("protection.off")}</span>
          </li>
        ))}
      </ul>
      <div>
        <Button icon={<Open20Regular />} onClick={() => void call("open_system_tool", { tool: "windowsSecurity" })}>
          {t("protection.openSecurity")}
        </Button>
      </div>
    </div>
  );
}

function PermissionsTab({ view, onChanged }: { view: ProtectionView; onChanged: (p: ProtectionView["permissions"]) => void }) {
  const { t } = useTranslation();
  const f = useFormat();
  const [cap, setCap] = useState<Capability>("webcam");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<BackendError | null>(null);
  const state = view.permissions.find((c) => c.capability === cap);

  const set = async (id: string, allowed: boolean) => {
    setBusy(true);
    try {
      onChanged(await call("permission_set", { capability: cap, id, allowed }));
      setError(null);
    } catch (e) {
      setError(toBackendError(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="tool-section">
      <div className="toolbar">
        <Segmented<Capability>
          label={t("protection.tabs.permissions")}
          value={cap}
          onChange={setCap}
          options={(["webcam", "microphone", "location"] as const).map((c) => ({ value: c, label: t(`protection.capability.${c}`) }))}
        />
        <span className="tool-meta">{state ? (state.allowed ? t("protection.capOn") : t("protection.capOff")) : ""}</span>
        <Button variant="subtle" icon={<Open20Regular />} onClick={() => void call("open_system_tool", { tool: "privacySettings" })}>
          {t("protection.openPrivacy")}
        </Button>
      </div>
      <p className="tool-meta">{t("protection.permissionsHint")}</p>
      {error && <ErrorNotice error={error} />}
      {!state || state.apps.length === 0 ? (
        <p className="flow-muted">{t("protection.noApps")}</p>
      ) : (
        <ul className="rows">
          {state.apps.map((a) => (
            <li key={a.id} className="row">
              <div className="row__main">
                <span className="row__title">
                  {a.packaged ? a.app : a.app.split("\\").pop()}
                  {a.inUse && <span className="tag tag--caution">{t("protection.inUse")}</span>}
                  {!a.packaged && <span className="tag">{t("protection.desktopApp")}</span>}
                </span>
                {!a.packaged && (
                  <span className="row__sub row__mono" title={a.app}>
                    {a.app}
                  </span>
                )}
                <span className="row__sub">{a.lastUsedMs ? t("protection.lastUsed", { date: f.dateTime(a.lastUsedMs) }) : t("protection.neverUsed")}</span>
              </div>
              {a.packaged ? (
                <Toggle checked={a.allowed} disabled={busy} onChange={(v) => void set(a.id, v)} label={t("protection.allowLabel", { app: a.app, what: t(`protection.capability.${cap}`) })} />
              ) : (
                <span className="row__size">{a.allowed ? t("protection.allowed") : t("protection.denied")}</span>
              )}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

export function ProtectionPage() {
  const { t } = useTranslation();
  const [view, setView] = useState<ProtectionView | null>(null);
  const [error, setError] = useState<BackendError | null>(null);
  const [tab, setTab] = useState<Tab>("defender");

  const load = useCallback(() => {
    call("protection_status")
      .then((v) => {
        setView(v);
        setError(null);
      })
      .catch((e) => setError(toBackendError(e)));
  }, []);
  useEffect(load, [load]);

  const status = view ? overall(view) : null;
  return (
    <div className="tool">
      <header className="tool__head protection-head">
        <div className={`status-badge status-badge--${status ?? "unknown"}`} aria-hidden="true">
          {status === "good" ? <ShieldCheckmark48Regular /> : <ShieldError48Regular />}
        </div>
        <div>
          <h1>{t("modules.protection.name")}</h1>
          <p className="tool-meta">{status ? t(`protection.overall.${status}`) : t("common.loading")}</p>
        </div>
      </header>
      {error && <ErrorNotice error={error} />}
      {view && (
        <Tabs<Tab>
          label={t("modules.protection.name")}
          value={tab}
          onChange={setTab}
          tabs={[
            { id: "defender", label: t("protection.tabs.defender") },
            { id: "firewall", label: t("protection.tabs.firewall") },
            { id: "permissions", label: t("protection.tabs.permissions") },
          ]}
        >
          {tab === "defender" && <DefenderTab view={view} onChanged={(d) => setView({ ...view, defender: d ?? view.defender })} />}
          {tab === "firewall" && <FirewallTab view={view} />}
          {tab === "permissions" && <PermissionsTab view={view} onChanged={(p) => setView({ ...view, permissions: p })} />}
        </Tabs>
      )}
    </div>
  );
}
