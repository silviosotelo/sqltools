import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

/** Reads the colour tokens of `styles.css` and checks WCAG contrast of the pairs the UI really uses. */

const css = readFileSync(new URL("./styles.css", import.meta.url), "utf8");

function block(selector: string): Record<string, string> {
  const start = css.indexOf(`${selector} {`);
  if (start < 0) throw new Error(`block not found: ${selector}`);
  const end = css.indexOf("}", start);
  const tokens: Record<string, string> = {};
  for (const match of css.slice(start, end).matchAll(/(--[\w-]+):\s*([^;]+);/g)) {
    tokens[match[1]] = match[2].trim();
  }
  return tokens;
}

const light = block(":root");
const dark = { ...light, ...block('[data-theme="dark"]') };

function channel(value: number): number {
  const v = value / 255;
  return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
}

function luminance(hex: string): number {
  let h = hex.replace("#", "");
  if (h.length === 3) h = [...h].map((c) => c + c).join("");
  if (h.length === 8) h = h.slice(0, 6);
  const n = parseInt(h, 16);
  return 0.2126 * channel((n >> 16) & 255) + 0.7152 * channel((n >> 8) & 255) + 0.0722 * channel(n & 255);
}

export function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

const themes = { light, dark } as const;

// [foreground, background, minimum ratio]. 4.5 = text, 3 = boundary of a form control (WCAG 1.4.11).
const pairs: Array<[string, string, number]> = [
  ["--text", "--surface", 4.5],
  ["--text", "--bg", 4.5],
  ["--text", "--side", 4.5],
  ["--text-strong", "--surface", 4.5],
  ["--muted", "--surface", 4.5],
  ["--muted", "--bg", 4.5],
  ["--muted", "--side", 4.5],
  ["--muted", "--surface-sunken", 4.5],
  ["--muted", "--surface-alt", 4.5],
  ["--faint", "--surface", 4.5],
  ["--faint", "--bg", 4.5],
  ["--faint", "--side", 4.5],
  ["--faint", "--surface-alt", 4.5],
  ["--accent-text", "--accent-soft", 4.5],
  ["--accent-text", "--surface", 4.5],
  ["--accent-ink", "--accent", 4.5],
  ["--success", "--surface", 4.5],
  ["--success", "--side", 4.5],
  ["--warn", "--warn-soft", 4.5],
  ["--danger", "--surface", 4.5],
  ["--danger", "--err-soft", 4.5],
  ["--t-k", "--surface", 4.5],
  ["--t-s", "--surface", 4.5],
  ["--t-n", "--surface", 4.5],
  ["--t-c", "--surface", 4.5],
  ["--control-border", "--surface", 3],
  ["--control-border", "--bg", 3],
  ["--control-border", "--side", 3],
  ["--control-border", "--surface-alt", 3],
  ["--accent", "--surface", 3],
];

describe("colour tokens", () => {
  for (const [themeName, tokens] of Object.entries(themes)) {
    describe(themeName, () => {
      it("defines every token the pairs use", () => {
        const missing = [...new Set(pairs.flat().filter((x) => typeof x === "string" && x.startsWith("--")))].filter(
          (name) => !tokens[name],
        );
        expect(missing).toEqual([]);
      });
      for (const [fg, bg, min] of pairs) {
        it(`${fg} on ${bg} is at least ${min}:1`, () => {
          if (!tokens[fg] || !tokens[bg]) return; // reported by the "defines every token" test
          expect(contrast(tokens[fg], tokens[bg])).toBeGreaterThanOrEqual(min);
        });
      }
    });
  }
});
