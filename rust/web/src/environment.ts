export const ENV_COLORS = ["neutral", "cyan", "green", "amber", "red", "violet"] as const;
export type EnvColor = (typeof ENV_COLORS)[number];
export interface Environment {
  label: string;
  color: EnvColor;
}

export const ENV_LABEL_MAX = 12;

/** Same rules as `Environment::validated` in Rust: trimmed label of 1..=12 characters, palette color. */
export function validateEnvironment(
  input: { label: string; color: string },
): { ok: true; value: Environment } | { ok: false; error: "label" | "color" } {
  const label = input.label.trim();
  const length = [...label].length; // code points, like Rust's chars().count()
  if (length === 0 || length > ENV_LABEL_MAX) return { ok: false, error: "label" };
  if (!(ENV_COLORS as readonly string[]).includes(input.color)) return { ok: false, error: "color" };
  return { ok: true, value: { label, color: input.color as EnvColor } };
}

/** A stored color outside the palette (hand-edited file) renders as neutral instead of breaking. */
export function safeColor(color: string): EnvColor {
  return (ENV_COLORS as readonly string[]).includes(color) ? (color as EnvColor) : "neutral";
}