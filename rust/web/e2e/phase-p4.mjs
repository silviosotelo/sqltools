// P4 checks on the installed app: To-Do view and the snapshot comparison behind the menu.
// The file pickers are native dialogs that CDP cannot drive, so the commands are invoked directly with a
// temp path and the UI entry points are only checked for presence.
import { connect } from "./cdp.mjs";

const cdp = await connect();
const checks = {};
const check = (n, ok, d) => { checks[n] = { ok: Boolean(ok), ...(d === undefined ? {} : { d }) }; };
const J = async (expr) => JSON.parse(await cdp.evaluate(`JSON.stringify(${expr})`));
const status = () => cdp.evaluate(`document.querySelector("[data-e2e='status-message']")?.innerText.trim() ?? ""`);

await cdp.reload();
await cdp.wait(1200);
await cdp.clickTitle("Conectar");
await cdp.waitFor(`/(Conectado|anamnesis@)/.test(document.querySelector("[data-e2e='conn-chip']")?.innerText ?? '')`, 20000);
await cdp.wait(2000);

// 1) a fifth side view lists the TODO/FIXME notes of the schema
const views = await J(`Array.from(document.querySelectorAll('.activity .act')).map(b => b.getAttribute('aria-label'))`);
check("the activity bar has a To-Do view", views.includes("To-Do"), views);
await cdp.click('.activity .act[aria-label="To-Do"]');
await cdp.waitFor(`document.querySelectorAll('.objects-panel .history-row').length > 0`, 30000);
const notes = await J(`Array.from(document.querySelectorAll('.objects-panel .history-row')).slice(0, 3).map(r => r.innerText.replace(/\\s+/g, ' ').trim())`);
check("the To-Do view lists notes with object and line", notes.length > 0 && notes.every((n) => /(TODO|FIXME) .*:\d+/.test(n)), notes);
await cdp.evaluate(`document.querySelector('.objects-panel .history-row')?.click()`);
await cdp.waitFor(`document.querySelectorAll('.editor-tab').length >= 2`, 30000);
const tabs = await J(`Array.from(document.querySelectorAll('.editor-tab .tab-title')).map(t => t.innerText.trim())`);
check("clicking a note opens its object source", tabs.length >= 2 && tabs.some((t) => /\(.*\)$/.test(t)), tabs);

// 2) the menu offers the two comparison entry points
await cdp.click('.actionstrip button[aria-label="Más acciones"]');
await cdp.wait(500);
const menu = await J(`Array.from(document.querySelectorAll('[role=menuitem]')).map(b => b.innerText.trim())`);
check(
  "the menu offers saving a snapshot and comparing with one",
  menu.some((item) => /snapshot de fuentes/.test(item)) && menu.some((item) => /Comparar con snapshot/.test(item)),
  menu,
);
await cdp.evaluate(`document.querySelector('.menu-backdrop')?.click()`);
await cdp.wait(300);

// 3) the comparison commands work against the real database
const snapshotPath = "C:\\\\Users\\\\sotelos\\\\AppData\\\\Local\\\\Temp\\\\opencode\\\\p4-e2e-snapshot.json";
const probe = JSON.parse(
  await cdp.evaluate(`(async () => {
    const list = await window.__TAURI_INTERNALS__.invoke("list_connections");
    const id = list[0].id;
    const saved = await window.__TAURI_INTERNALS__.invoke("save_sources_snapshot", { connectionId: id, schema: null, path: ${JSON.stringify(snapshotPath)} });
    const result = await window.__TAURI_INTERNALS__.invoke("compare_with_snapshot", { connectionId: id, schema: null, path: ${JSON.stringify(snapshotPath)} });
    return JSON.stringify({ saved, identical: result.entries.filter(e => e.status === "identical").length, total: result.entries.length, scriptLines: result.script.split("\\n").filter(Boolean).length });
  })()`),
);
check(
  "a snapshot is saved and compared against itself with no differences",
  probe.saved > 500 && probe.identical === probe.total && probe.scriptLines <= 1,
  probe,
);
cdp.close();
console.log(JSON.stringify(checks, null, 1));
const bad = Object.entries(checks).filter(([, v]) => !v.ok).map(([k]) => k);
console.log(bad.length ? "P4 CHECKS FAILED: " + bad.join("; ") : `P4 CHECKS OK (${Object.keys(checks).length})`);
process.exit(bad.length ? 1 : 0);
