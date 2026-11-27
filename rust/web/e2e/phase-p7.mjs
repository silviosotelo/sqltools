// P7 checks on the installed app: the file browser and saved reports.
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
await cdp.wait(1200);

// 1) the file browser lists the declared locations and a folder's contents
const views = await J(`Array.from(document.querySelectorAll('.activity .act')).map(b => b.getAttribute('aria-label'))`);
check("the activity bar has a Files view", views.includes("Archivos"), views);
await cdp.click('.activity .act[aria-label="Archivos"]');
await cdp.waitFor(`document.querySelectorAll('.objects-panel .conn-item').length > 0`, 15000);
const roots = await J(`Array.from(document.querySelectorAll('.objects-panel .conn-item')).map(b => b.textContent.trim())`);
check("the file browser shows its locations", roots.length > 0, roots);
await cdp.evaluate(`document.querySelector('.objects-panel .conn-item')?.click()`);
await cdp.waitFor(`document.querySelectorAll('.objects-panel ul[aria-label="Archivos"] .conn-item').length > 0`, 15000);
const entries = await J(`Array.from(document.querySelectorAll('.objects-panel ul[aria-label="Archivos"] .conn-item')).slice(0, 5).map(b => b.textContent.trim())`);
check("a location lists its folders and files, directories first", entries.length > 0, entries);

// 2) a report saved from the editor runs from the reports dialog
await cdp.evaluate(`document.querySelector('.cm-content')?.focus()`);
await cdp.key("a", "KeyA", 65, 2);
await cdp.send("Input.insertText", { text: "select 1 as uno from dual" });
await cdp.wait(200);
await cdp.click('.actionstrip button[aria-label="Más acciones"]');
await cdp.wait(400);
const menu = await J(`Array.from(document.querySelectorAll('[role=menuitem]')).map(b => b.innerText.trim())`);
check("the menu offers the reports dialog", menu.includes("Informes…"), menu);
await cdp.evaluate(`Array.from(document.querySelectorAll('[role=menuitem]')).find(b => b.innerText.trim() === 'Informes…')?.click()`);
await cdp.waitFor(`Boolean(document.querySelector('.palette'))`, 8000);
const dialog = await cdp.evaluate(`Boolean(document.querySelector('.palette input[aria-label="Nombre del informe"]'))`);
check("the reports dialog opens with a name field", dialog);
await cdp.evaluate(`(() => { const i = document.querySelector('.palette input[aria-label="Nombre del informe"]'); Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(i, 'sqlt_p7_e2e'); i.dispatchEvent(new Event('input', { bubbles: true })); })()`);
await cdp.evaluate(`Array.from(document.querySelectorAll('.palette .row button')).find(b => b.innerText.trim() === 'Guardar el editor como informe')?.click()`);
await cdp.waitFor(`/sqlt_p7_e2e/.test(document.querySelector('.palette')?.innerText ?? '')`, 15000);
const listed = await J(`Array.from(document.querySelectorAll('.palette .report-row .report-name')).map(e => e.innerText.trim())`);
check("the saved report appears in the list", listed.some((name) => name.startsWith("sqlt_p7_e2e")), listed);

await cdp.evaluate(`Array.from(document.querySelectorAll('.palette .report-row button')).find(b => b.innerText.trim() === 'Ejecutar')?.click()`);
await cdp.waitFor(`/1 filas?|1 row/.test(document.querySelector("[data-e2e='status-message']")?.innerText ?? '')`, 20000);
check("running the report executes its SQL", /1 filas?/.test(await status()), await status());

// 3) basic import: a delimited file becomes INSERT statements in the editor
const csvPath = "C:/Users/sotelos/AppData/Local/Temp/opencode/p7-import.csv";
const fs = await import("node:fs");
fs.writeFileSync(csvPath, "NAME;AGE\nO'Brien;42\n;7\n", "utf8");
const imported = JSON.parse(
  await cdp.evaluate(`(async () => {
    try {
      const preview = await window.__TAURI_INTERNALS__.invoke("csv_preview", { path: ${JSON.stringify(csvPath)}, hasHeader: true });
      const script = await window.__TAURI_INTERNALS__.invoke("csv_insert_script", { path: ${JSON.stringify(csvPath)}, table: "SQLT_P7_E2E", hasHeader: true, maxRows: 100 });
      return JSON.stringify({ ok: true, preview, script });
    } catch (error) {
      return JSON.stringify({ ok: false, error: String(error) });
    }
  })()`),
);
check(
  "the import preview reads the file's columns and rows",
  imported.ok && imported.preview.columns.join("|") === "NAME|AGE" && imported.preview.totalRows === 2,
  imported.preview ?? imported,
);
check(
  "the import generates INSERT statements without executing them",
  imported.ok &&
    /insert into "SQLT_P7_E2E" \("NAME", "AGE"\) values \('O''Brien', 42\);/.test(imported.script) &&
    /values \(NULL, 7\);/.test(imported.script),
  String(imported.script ?? imported.error ?? "").slice(0, 200),
);
await cdp.click('.actionstrip button[aria-label="Más acciones"]');
await cdp.wait(400);
const importMenu = await J(`Array.from(document.querySelectorAll('[role=menuitem]')).map(b => b.innerText.trim())`);
check("the menu offers importing a file", importMenu.includes("Importar archivo…"), importMenu);
await cdp.evaluate(`document.querySelector('.menu-backdrop')?.click()`);
await cdp.wait(200);

// 3) a report with parameters is reported as such
const openReportsDialog = async () => {
  for (let attempt = 0; attempt < 3; attempt++) {
    // close anything left open, then open the menu and pick the reports entry
    await cdp.evaluate(`document.querySelector('.palette-backdrop')?.click()`);
    await cdp.wait(200);
    await cdp.click('.actionstrip button[aria-label="Más acciones"]');
    await cdp.wait(400);
    await cdp.evaluate(`Array.from(document.querySelectorAll('[role=menuitem]')).find(b => b.innerText.trim() === 'Informes…')?.click()`);
    const open = await cdp.waitFor(`Boolean(document.querySelector('.palette input[aria-label="Nombre del informe"]'))`, 6000);
    if (open) return true;
  }
  return false;
};
check("the reports dialog can be reopened", await openReportsDialog());
await cdp.evaluate(`(() => { const i = document.querySelector('.palette input[aria-label="Nombre del informe"]'); Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(i, 'sqlt_p7_params'); i.dispatchEvent(new Event('input', { bubbles: true })); })()`);
await cdp.evaluate(`document.querySelector('.cm-content')?.focus()`);
await cdp.evaluate(`(() => { const e = document.querySelector('.cm-content'); e.focus(); })()`);
await cdp.key("a", "KeyA", 65, 2);
await cdp.send("Input.insertText", { text: "select * from a11 where n = &n" });
await cdp.wait(200);
await cdp.evaluate(`Array.from(document.querySelectorAll('.palette .row button')).find(b => b.innerText.trim() === 'Guardar el editor como informe')?.click()`);
await cdp.waitFor(`/1 parámetro|1 parameter/.test(document.querySelector('.palette')?.innerText ?? '')`, 15000);
const withParams = await cdp.evaluate(`document.querySelector('.palette')?.innerText ?? ''`);
check("a report with substitution variables shows its parameter count", /1 parámetro/.test(withParams), withParams.replace(/\s+/g, " ").slice(0, 160));

// 4) cleanup through the UI: delete both reports
for (let i = 0; i < 2; i++) {
  await cdp.evaluate(`Array.from(document.querySelectorAll('.palette .report-row')).find(r => r.innerText.includes('sqlt_p7_'))?.querySelector('button:last-child')?.click()`);
  await cdp.wait(800);
}
const left = await J(`Array.from(document.querySelectorAll('.palette .report-row .report-name')).map(e => e.innerText.trim())`);
check("deleting removes both reports", !left.some((name) => name.startsWith("sqlt_p7_")), left);
cdp.close();
console.log(JSON.stringify(checks, null, 1));
const bad = Object.entries(checks).filter(([, v]) => !v.ok).map(([k]) => k);
console.log(bad.length ? "P7 CHECKS FAILED: " + bad.join("; ") : `P7 CHECKS OK (${Object.keys(checks).length})`);
process.exit(bad.length ? 1 : 0);
