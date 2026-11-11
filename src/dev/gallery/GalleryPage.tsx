import { Broom24Regular, Delete20Regular, Sparkle24Regular } from "@fluentui/react-icons";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { MODULES, type ModuleId } from "../../app/modules";
import {
  Badge,
  Button,
  Card,
  CheckList,
  ConfirmDialog,
  ErrorNotice,
  IconButton,
  UsageBar,
  ModuleHero,
  ProgressRing,
  ScanButton,
  Segmented,
  SideSheet,
  Toggle,
  type CheckItem,
} from "../../components";
import { formatBytes } from "../../lib/format";
import "./gallery.css";

const ACCENTED = MODULES.filter((m) => m.accent);

const SAMPLE_ITEMS: CheckItem[] = [
  { id: "temp", label: "User temporary files", description: "Files apps leave behind in %TEMP%. Safe to remove.", size: formatBytes(1_288_490_189) },
  { id: "thumbs", label: "Thumbnail cache", description: "Windows rebuilds it when needed.", size: formatBytes(214_958_080) },
  { id: "edge", label: "Microsoft Edge cache", description: "Close Edge first. Passwords and history are not touched.", size: formatBytes(734_003_200) },
];

/** One full set of components, rendered inside a forced theme. */
function ThemePanel({ theme, module }: { theme: "light" | "dark"; module: ModuleId }) {
  const { t } = useTranslation();
  const [selected, setSelected] = useState<Set<string>>(new Set(["temp", "thumbs"]));
  const [seg, setSeg] = useState<"a" | "b" | "c">("a");
  const [on, setOn] = useState(true);
  const [sheet, setSheet] = useState(false);
  const [confirm, setConfirm] = useState(false);
  const def = MODULES.find((m) => m.id === module)!;

  return (
    <section className="gallery__panel" data-theme={theme} data-module={module} aria-label={`${theme} theme`}>
      <h2>{theme === "light" ? "Light" : "Dark"}</h2>

      <h3>Buttons</h3>
      <div className="gallery__row">
        <Button variant="primary">Primary</Button>
        <Button variant="accent">Accent</Button>
        <Button>Secondary</Button>
        <Button variant="subtle">Subtle</Button>
        <Button variant="primary" disabled>
          Disabled
        </Button>
        <IconButton label="Delete" icon={<Delete20Regular />} />
        <Button variant="danger" onClick={() => setConfirm(true)}>
          Danger (opens confirm)
        </Button>
      </div>
      <ConfirmDialog open={confirm} title="Empty the Recycle Bin?" confirmLabel="Delete" onCancel={() => setConfirm(false)} onConfirm={() => setConfirm(false)}>
        <p>The Recycle Bin holds 2.3 GB. Emptying it deletes those files for good.</p>
      </ConfirmDialog>

      <h3>Usage bars and error notice</h3>
      <div className="gallery__row" style={{ flexDirection: "column", alignItems: "stretch" }}>
        <UsageBar value={45} label="Normal" />
        <UsageBar value={84} label="Warning" tone="warning" />
        <UsageBar value={95} label="Critical" tone="critical" />
        <ErrorNotice error={{ code: "busy", message: "busy: cannot start a cleanup", details: "Busy(\"cannot start a cleanup\")" }} />
      </div>
      <div className="gallery__row">
        <Button variant="primary" size="lg" icon={<Broom24Regular />}>
          {t("common.run")}
        </Button>
        <ScanButton>{t("common.scan")}</ScanButton>
      </div>

      <h3>Badges</h3>
      <div className="gallery__row">
        <Badge tone="neutral">3</Badge>
        <Badge tone="info">12</Badge>
        <Badge tone="success">OK</Badge>
        <Badge tone="warning">2</Badge>
        <Badge tone="danger">1</Badge>
      </div>

      <h3>Progress rings</h3>
      <div className="gallery__row">
        <ProgressRing value={25} size={96} label="25 percent">
          25%
        </ProgressRing>
        <ProgressRing value={70} size={96} label="70 percent">
          70%
        </ProgressRing>
        <ProgressRing size={96} label="Scanning" />
      </div>

      <h3>Cards</h3>
      <div className="gallery__cards">
        <Card
          module={module}
          icon={<Sparkle24Regular />}
          title="Junk files"
          value={formatBytes(2_237_451_469)}
          description="Temporary files and caches that are safe to remove."
          actions={
            <>
              <Button>{t("common.review")}</Button>
              <Button variant="primary">{t("common.run")}</Button>
            </>
          }
        />
        <Card title="Disk health" description="All drives report Healthy." />
      </div>

      <h3>Controls</h3>
      <div className="gallery__row">
        <Segmented
          label="Sample"
          value={seg}
          onChange={setSeg}
          options={[
            { value: "a", label: "System" },
            { value: "b", label: "Light" },
            { value: "c", label: "Dark" },
          ]}
        />
        <Toggle checked={on} onChange={setOn} label="Quarantine mode" showLabel />
      </div>

      <h3>Check list</h3>
      <div className="smd-card">
        <CheckList label="Cleanup items" items={SAMPLE_ITEMS} selected={selected} onChange={setSelected} />
      </div>

      <h3>Module hero</h3>
      <ModuleHero icon={def.icon} title={t(`modules.${module}.name`)} tagline={t(`modules.${module}.tagline`)} />

      <h3>Side sheet</h3>
      <div className="gallery__row">
        <Button onClick={() => setSheet(true)}>Open side sheet</Button>
      </div>
      <SideSheet
        open={sheet}
        onClose={() => setSheet(false)}
        title="Review: Cleanup"
        footer={
          <>
            <Button onClick={() => setSheet(false)}>{t("common.cancel")}</Button>
            <Button variant="primary">{t("common.run")}</Button>
          </>
        }
      >
        <CheckList label="Cleanup items" items={SAMPLE_ITEMS} selected={selected} onChange={setSelected} />
      </SideSheet>

      <h3>Module accents</h3>
      <div className="gallery__swatches">
        {ACCENTED.map((m) => (
          <div key={m.id} className="gallery__swatch" data-module={m.id}>
            <span className="gallery__swatch-grad" />
            <span className="gallery__swatch-solid">Aa</span>
            <span className="gallery__swatch-name">{t(`modules.${m.id}.name`)}</span>
          </div>
        ))}
      </div>
    </section>
  );
}

/** /dev/gallery: every component in both themes, with a module accent picker. */
export function GalleryPage() {
  const [module, setModule] = useState<ModuleId>("cleanup");
  return (
    <div className="gallery">
      <div className="gallery__toolbar">
        <h1>Component gallery</h1>
        <label>
          Module accent{" "}
          <select value={module} onChange={(e) => setModule(e.target.value as ModuleId)}>
            {ACCENTED.map((m) => (
              <option key={m.id} value={m.id}>
                {m.id}
              </option>
            ))}
          </select>
        </label>
      </div>
      <div className="gallery__grid">
        <ThemePanel theme="light" module={module} />
        <ThemePanel theme="dark" module={module} />
      </div>
    </div>
  );
}
