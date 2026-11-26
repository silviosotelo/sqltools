// axe-core over CDP against the running app, per view and per theme.
//   node axe-cdp.mjs                      all registered views, both themes
//   node axe-cdp.mjs --views shell,results  only those views
// Exit code 1 if any violation is found (any impact). Views are registered below; each phase adds its own.
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { connect } from "./cdp.mjs";

const here = path.dirname(fileURLToPath(import.meta.url));
const axeSource = fs.readFileSync(path.join(here, "..", "node_modules", "axe-core", "axe.min.js"), "utf8");
const args = process.argv.slice(2);
const viewsArg = args.indexOf("--views") >= 0 ? args[args.indexOf("--views") + 1].split(",") : null;

const CHIP = "[data-e2e='conn-chip'], .connection-chip";

async function ensureConnected(cdp) {
  const connected = await cdp.evaluate(`/(Conectado|anamnesis@)/.test(document.querySelector(${JSON.stringify(CHIP)})?.innerText ?? '')`);
  if (connected) return;
  await cdp.clickTitle("Conectar");
  await cdp.waitFor(`/(Conectado|anamnesis@)/.test(document.querySelector(${JSON.stringify(CHIP)})?.innerText ?? '')`, 20000);
}

// Each view: how to reach it. `steps(cdp)` runs after the page is ready in the chosen theme.
const VIEWS = {
  shell: { steps: async () => {} },
  results: {
    steps: async (cdp) => {
      await ensureConnected(cdp);
      await cdp.evaluate(`document.querySelector('.cm-content')?.focus()`);
      await cdp.key("a", "KeyA", 65, 2);
      await cdp.send("Input.insertText", { text: "select * from dual" });
      await cdp.key("F5", "F5", 116);
      await cdp.waitFor(`document.querySelectorAll('.grid tbody tr').length > 0`, 15000);
    },
  },
  objects: {
    steps: async (cdp) => {
      await ensureConnected(cdp);
      if (!(await cdp.evaluate(`Boolean(document.querySelector("[role='tree']"))`))) {
        await cdp.clickTitle("Cargar objetos");
        await cdp.waitFor(`document.querySelectorAll('.objects-panel tbody tr').length > 0`, 20000);
      }
    },
  },
};

const names = viewsArg ?? Object.keys(VIEWS);
const cdp = await connect();
let total = 0;
const report = [];

for (const theme of ["light", "dark"]) {
  await cdp.evaluate(`localStorage.setItem('sqlt.theme', ${JSON.stringify(theme)})`);
  for (const name of names) {
    const view = VIEWS[name];
    if (!view) { console.error("unknown view", name); process.exit(2); }
    await cdp.reload();
    await view.steps(cdp);
    await cdp.wait(300);
    await cdp.evaluate(axeSource);
    const result = JSON.parse(await cdp.evaluate(`(async () => { const r = await window.axe.run(document); return JSON.stringify(r.violations.map(v => ({ id: v.id, impact: v.impact, nodes: v.nodes.length, sample: v.nodes.slice(0, 2).map(n => n.target.join(' ') + ' :: ' + (n.failureSummary || '').split('\\n')[1]) }))); })()`));
    const dataTheme = await cdp.evaluate(`document.documentElement.dataset.theme`);
    report.push({ theme, requested: theme, applied: dataTheme, view: name, violations: result });
    total += result.length;
    console.log(`${theme.padEnd(5)} ${name.padEnd(12)} applied=${dataTheme} violations=${result.length}`);
    for (const v of result) console.log(`   ${v.impact}:${v.id} nodes=${v.nodes}\n      ${v.sample.join("\n      ")}`);
  }
}
cdp.close();
fs.writeFileSync(path.join(here, "axe-report.json"), JSON.stringify(report, null, 1));
console.log(total === 0 ? `\nAXE OK: 0 violations in ${names.length} view(s) x 2 themes` : `\nAXE FAILED: ${total} violation group(s)`);
process.exit(total === 0 ? 0 : 1);
