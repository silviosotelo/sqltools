import { useTranslation } from "../i18n";
import type { StatEntry } from "../types";

/** Side view: the session statistics of the connected user. */
export function StatsView({ stats }: { stats: StatEntry[] }) {
  const { t } = useTranslation();
  return (
    <section className="side-section side-fill" aria-labelledby="stats-title">
      <h2 id="stats-title" className="panel-title">
        {t("stats")}
      </h2>
      <div className="stats-scroll" tabIndex={0}>
        <table className="grid">
          <thead>
            <tr>
              <th>{t("stats")}</th>
              <th>{t("value")}</th>
            </tr>
          </thead>
          <tbody>
            {stats.map(([name, value]) => (
              <tr key={name}>
                <td>{name}</td>
                <td>{value}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  );
}