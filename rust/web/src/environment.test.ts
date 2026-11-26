import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { ENV_COLORS, safeColor, validateEnvironment } from "./environment";
import { contrast } from "./contrast";

describe("validateEnvironment", () => {
  it("trims the label", () => {
    expect(validateEnvironment({ label: "  PROD ", color: "red" })).toEqual({ ok: true, value: { label: "PROD", color: "red" } });
  });
  it("accepts exactly 12 characters and rejects 13 (same cases as Rust)", () => {
    expect(validateEnvironment({ label: "123456789012", color: "red" }).ok).toBe(true);
    expect(validateEnvironment({ label: "1234567890123", color: "red" }).ok).toBe(false);
    expect(validateEnvironment({ label: "ñandú-pre-01", color: "green" }).ok).toBe(true);
    expect(validateEnvironment({ label: "日本語日本語日本語日本語日", color: "green" }).ok).toBe(false);
  });
  it("rejects an empty label and an unknown color", () => {
    expect(validateEnvironment({ label: "   ", color: "red" })).toEqual({ ok: false, error: "label" });
    expect(validateEnvironment({ label: "X", color: "pink" })).toEqual({ ok: false, error: "color" });
  });
  it("safeColor falls back to neutral", () => {
    expect(safeColor("pink")).toBe("neutral");
    expect(safeColor("cyan")).toBe("cyan");
  });
});

const css = readFileSync(new URL("./styles.css", import.meta.url), "utf8");
const dark = css.slice(css.indexOf('[data-theme="dark"] {'));
const light = css.slice(0, css.indexOf('[data-theme="dark"] {'));
const token = (scope: string, name: string) => {
  const m = scope.match(new RegExp(`${name}:\\s*(#[0-9a-fA-F]{3,8})`));
  if (!m) throw new Error(`token not found: ${name}`);
  return m[1];
};

describe("environment badge tokens", () => {
  for (const [theme, scope] of [["light", light], ["dark", dark]] as const) {
    it(`every env color pair has contrast >= 4.5 in ${theme}`, () => {
      for (const color of ENV_COLORS) {
        const ratio = contrast(token(scope, `--env-${color}-fg`), token(scope, `--env-${color}-bg`));
        expect(ratio, `${theme} ${color}`).toBeGreaterThanOrEqual(4.5);
      }
    });
  }
});