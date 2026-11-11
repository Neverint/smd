import { ErrorCircle20Regular } from "@fluentui/react-icons";
import { useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import type { BackendError } from "../lib/backend";
import { Button } from "./Button";

export interface ErrorNoticeProps {
  error: BackendError;
  actions?: ReactNode;
}

/** Friendly message for a backend error, with the technical details one click away. */
export function ErrorNotice({ error, actions }: ErrorNoticeProps) {
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);
  const copy = () => {
    const text = `${error.code}: ${error.message}\n${error.details}`;
    navigator.clipboard
      ?.writeText(text)
      .then(() => setCopied(true))
      .catch(() => setCopied(false));
  };
  return (
    <div className="smd-error" role="alert">
      <ErrorCircle20Regular className="smd-error__icon" aria-hidden="true" />
      <div className="smd-error__text">
        <strong>{t("errors.title")}</strong>
        <span>{t(`errors.${error.code}`)}</span>
      </div>
      <div className="smd-error__actions">
        <Button variant="subtle" onClick={copy}>
          {copied ? t("common.copied") : t("common.copyDetails")}
        </Button>
        {actions}
      </div>
    </div>
  );
}
