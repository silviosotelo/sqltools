import { useTranslation } from "../i18n";
import { EnvBadge } from "./EnvBadge";

export type StatusDot = "ok" | "warn" | "err" | "";

const EOL_LABEL: Record<string, string> = { "\n": "LF", "\r\n": "CRLF", "\r": "CR" };

/** `Oracle Database 12c Standard Edition Release 12.1.0.2.0 - 64bit Production` -> `Oracle 12c`. */
export function shortServer(banner: string): string {
  const match = /Oracle Database (\w+)/.exec(banner);
  return match ? `Oracle ${match[1]}` : banner;
}

export function StatusBar({
  dot,
  connLabel,
  message,
  eol,
  cursor,
  server,
  environment,
}: {
  dot: StatusDot;
  /** `user@alias`, or empty when there is no connection. */
  connLabel: string;
  message: string;
  eol: string;
  cursor: { line: number; col: number } | null;
  /** Full server banner (shown shortened, full text as tooltip). */
  server: string;
  environment?: { label: string; color: string } | null;
}) {
  const { t } = useTranslation();
  return (
    <footer className="statusbar">
      <div className="status">
        <span className="seg" title={server || undefined}>
          <span className={dot ? `dot ${dot}` : "dot"} />
          <b>{connLabel ? connLabel : t("noConnection")}</b>
          {connLabel && <EnvBadge environment={environment} />}
          {server && <span className="muted">· {shortServer(server)}</span>}
        </span>
        <span className="seg msg" role="status" data-e2e="status-message">
          {message}
        </span>
        <span className="sp" />
        {cursor && (
          <span className="seg">
            Ln {cursor.line}, Col {cursor.col}
          </span>
        )}
        <span className="seg">{EOL_LABEL[eol] ?? "LF"}</span>
      </div>
    </footer>
  );
}
