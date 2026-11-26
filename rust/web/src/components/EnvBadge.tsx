import { safeColor, type Environment } from "../environment";

/** The only place an environment is rendered: title-bar chip, connection list, objects panel and status bar. */
export function EnvBadge({ environment }: { environment?: { label: string; color: string } | Environment | null }) {
  if (!environment || !environment.label) return null;
  return (
    <span className="env-badge" data-color={safeColor(environment.color)} data-e2e="env-badge">
      {environment.label}
    </span>
  );
}