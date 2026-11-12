import { hierarchy, treemap, treemapSquarify, type HierarchyRectangularNode } from "d3-hierarchy";
import { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { ViewNode } from "../../lib/types";
import { useFormat } from "../../lib/useFormat";

interface Datum {
  id: number;
  name: string;
  kind: ViewNode["kind"];
  size: number;
  files: number;
  /** Index of the top-level box, for its color. */
  color: number;
  children?: Datum[];
}

const HEADER = 18;
const LABEL_MIN_W = 56;
const LABEL_MIN_H = 18;

/** Converts a view node; children past the per-folder limit become one grey box. */
function toDatum(n: ViewNode, color: number, depth: number): Datum {
  const kids = n.children.map((c, i) => toDatum(c, depth === 0 ? i % 8 : color, depth + 1));
  if (n.hiddenCount > 0) {
    kids.push({ id: -1 - n.id, name: "", kind: "others", size: n.hiddenSize, files: n.hiddenCount, color });
  }
  return { id: n.id, name: n.name, kind: n.kind, size: n.size, files: n.files, color, children: kids.length ? kids : undefined };
}

export interface TreemapProps {
  node: ViewNode;
  onOpen: (id: number) => void;
}

/** Squarified treemap of a folder, two levels deep. Folders at the first level are buttons. */
export function Treemap({ node, onOpen }: TreemapProps) {
  const { t } = useTranslation();
  const f = useFormat();
  const ref = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ w: 0, h: 0 });

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const ro = new ResizeObserver(([entry]) => {
      const r = entry.contentRect;
      setSize({ w: Math.floor(r.width), h: Math.floor(r.height) });
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const leaves = useMemo(() => {
    if (size.w < 10 || size.h < 10 || node.size === 0) return [];
    const root = hierarchy<Datum>(toDatum(node, 0, 0), (d) => d.children)
      .sum((d) => (d.children ? 0 : Math.max(d.size, 0)))
      .sort((a, b) => (b.value ?? 0) - (a.value ?? 0));
    const laid = treemap<Datum>()
      .tile(treemapSquarify)
      .size([size.w, size.h])
      .paddingInner(2)
      .paddingTop((n) => (n.depth === 1 && n.children && n.y1 - n.y0 > HEADER * 2 ? HEADER : n.depth === 1 && n.children ? 2 : 0))
      .paddingLeft((n) => (n.depth === 1 && n.children ? 2 : 0))
      .paddingRight((n) => (n.depth === 1 && n.children ? 2 : 0))
      .paddingBottom((n) => (n.depth === 1 && n.children ? 2 : 0))(root);
    return (laid.children ?? []) as HierarchyRectangularNode<Datum>[];
  }, [node, size]);

  const label = (d: Datum) => {
    if (d.kind === "others") return t("spaceLens.others", { count: d.files });
    return d.name;
  };
  const kindClass = (d: Datum) => (d.kind === "file" ? " tile--file" : d.kind === "others" ? " tile--others" : "");
  const tileLabel = (d: Datum, w: number, h: number) =>
    w >= LABEL_MIN_W &&
    h >= LABEL_MIN_H && (
      <span className="tile__label">
        {label(d)} <span className="tile__size">{f.bytes(d.size)}</span>
      </span>
    );

  return (
    <div ref={ref} className="treemap" role="group" aria-label={t("spaceLens.treemapLabel", { name: node.name })}>
      {leaves.map((d) => {
        const w = d.x1 - d.x0;
        const h = d.y1 - d.y0;
        if (w < 1 || h < 1) return null;
        const data = d.data;
        const style = {
          left: d.x0,
          top: d.y0,
          width: w,
          height: h,
          // Folders get a palette color; files and grouped small files keep their neutral class color.
          ...(data.kind === "dir" ? { ["--tile-color" as string]: `var(--lens-${data.color + 1})` } : {}),
        };
        // Inner boxes are drawn inside their folder, so a click anywhere in a folder opens that folder.
        const inner = (d.children ?? []).map((c) => {
          const cw = c.x1 - c.x0;
          const ch = c.y1 - c.y0;
          if (cw < 1 || ch < 1) return null;
          return (
            <span
              key={c.data.id}
              className={`tile tile--child${kindClass(c.data)}`}
              style={{ left: c.x0 - d.x0, top: c.y0 - d.y0, width: cw, height: ch }}
              title={`${label(c.data)} · ${f.bytes(c.data.size)}`}
            >
              {tileLabel(c.data, cw, ch)}
            </span>
          );
        });
        const title = `${label(data)} · ${f.bytes(data.size)}`;
        if (data.kind === "dir") {
          // A labelled tile is named by its own text ("Open" is added for screen readers only),
          // so speech users can say what they see; tiny tiles without text get an aria-label.
          const shown = tileLabel(data, w, h);
          return (
            <button key={data.id} type="button" className="tile" style={style} title={title} aria-label={shown ? undefined : t("spaceLens.open", { name: data.name, size: f.bytes(data.size) })} onClick={() => onOpen(data.id)}>
              {shown && <span className="sr-only">{t("spaceLens.openPrefix")} </span>}
              {shown}
              <span aria-hidden="true">{inner}</span>
            </button>
          );
        }
        return (
          <div key={data.id} className={`tile${kindClass(data)}`} style={style} title={title} aria-hidden="true">
            {tileLabel(data, w, h)}
          </div>
        );
      })}
    </div>
  );
}
