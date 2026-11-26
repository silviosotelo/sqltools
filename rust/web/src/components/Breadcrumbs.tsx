import { Icon } from "../icons";
import { useTranslation } from "../i18n";

/** connection > schema > type > object (or the untitled script): where the editor content comes from. */
export function Breadcrumbs({ parts }: { parts: string[] }) {
  const { t } = useTranslation();
  return (
    <nav className="breadcrumbs" aria-label={t("location")}>
      <ol>
        {parts.map((part, index) => (
          <li key={index} aria-current={index === parts.length - 1 ? "location" : undefined}>
            {index > 0 && (
              <span className="crumb-sep" aria-hidden="true">
                <Icon name="chevron" size={10} />
              </span>
            )}
            <span className="crumb">{part}</span>
          </li>
        ))}
      </ol>
    </nav>
  );
}