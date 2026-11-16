import { useTranslation } from "./i18n";

export default function App() {
  const { t, language, setLanguage } = useTranslation();
  return (
    <main>
      <h1>{t("app.title")}</h1>
      <p>{t("app.subtitle")}</p>
      <label>
        {t("app.language")}{" "}
        <select
          value={language}
          onChange={(event) => setLanguage(event.target.value as "es" | "en")}
        >
          <option value="es">Español</option>
          <option value="en">English</option>
        </select>
      </label>
    </main>
  );
}
