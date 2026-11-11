import type { FluentIcon } from "@fluentui/react-icons";
import { motion } from "motion/react";

export interface ModuleHeroProps {
  icon: FluentIcon;
  title: string;
  tagline: string;
}

/** Module header: glow, illustration orb, title and a one-line explanation. Colors come from [data-module]. */
export function ModuleHero({ icon: Icon, title, tagline }: ModuleHeroProps) {
  return (
    <header className="smd-hero">
      <div className="smd-hero__glow" aria-hidden="true" />
      <motion.div
        className="smd-hero__orb"
        aria-hidden="true"
        initial={{ opacity: 0, scale: 0.9, y: 8 }}
        animate={{ opacity: 1, scale: 1, y: 0 }}
        transition={{ duration: 0.4, ease: [0, 0, 0, 1] }}
      >
        <Icon />
      </motion.div>
      <h1 className="smd-hero__title">{title}</h1>
      <p className="smd-hero__tagline">{tagline}</p>
    </header>
  );
}
