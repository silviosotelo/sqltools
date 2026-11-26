// Command registry check against the running app: counts, a stable digest of ids/names/shortcuts/availability,
// and that the command palette opens from the title bar button and from Ctrl+Shift+P.
//   node commands-check.mjs [--save golden-commands.json | --compare golden-commands.json]
import fs from "node:fs";
import { connect } from "./cdp.mjs";

const args = process.argv.slice(2);
const opt = (name) => { const i = args.indexOf(name); return i >= 0 ? args[i + 1] : null; };
const cdp = await connect();

const commands = JSON.parse(await cdp.evaluate(`window.__TAURI_INTERNALS__.invoke("list_commands").then(c => JSON.stringify(c))`));
const rows = commands.map((c) => `${c.id}|${c.name}|${c.implemented ? 1 : 0}|${[...c.keys].sort().join(",")}`).sort();
const digest = await cdp.evaluate(`(async () => { const data = new TextEncoder().encode(${JSON.stringify(rows.join("\n"))}); const hash = await crypto.subtle.digest('SHA-256', data); return Array.from(new Uint8Array(hash)).map(b => b.toString(16).padStart(2, '0')).join(''); })()`);
const summary = {
  total: commands.length,
  named: commands.filter((c) => c.name).length,
  implemented: commands.filter((c) => c.implemented).length,
  withShortcut: commands.filter((c) => c.name && c.keys.length > 0).length,
  digest,
};

// The palette opens from the title bar button...
await cdp.reload();
await cdp.click(".palette-btn");
summary.paletteFromButton = Boolean(await cdp.waitFor(`Boolean(document.querySelector('[role=\"combobox\"]'))`, 3000));
await cdp.key("Escape", "Escape", 27);
await cdp.wait(300);
// ... and from the accelerator.
await cdp.evaluate(`document.querySelector('.cm-content')?.focus()`);
await cdp.key("P", "KeyP", 80, 2 | 8);
summary.paletteFromShortcut = Boolean(await cdp.waitFor(`Boolean(document.querySelector('[role=\"combobox\"]'))`, 3000));
cdp.close();

console.log(JSON.stringify(summary, null, 2));
let failed = !(summary.total === 283 && summary.named === 226 && summary.implemented === 213 && summary.paletteFromButton && summary.paletteFromShortcut);
if (opt("--save")) { fs.writeFileSync(opt("--save"), JSON.stringify(summary, null, 2)); console.log("saved", opt("--save")); }
if (opt("--compare")) {
  const golden = JSON.parse(fs.readFileSync(opt("--compare"), "utf8"));
  const same = ["total", "named", "implemented", "withShortcut", "digest"].every((k) => golden[k] === summary[k]);
  console.log(same ? "IDENTICAL registry to " + opt("--compare") : "REGISTRY DIFFERS from " + opt("--compare"));
  failed = failed || !same;
}
console.log(failed ? "COMMANDS CHECK FAILED" : "COMMANDS CHECK OK");
process.exit(failed ? 1 : 0);
