// P2 checks on the installed app: editable grid (single table with ROWID), buffered edit, save,
// conflict-free refresh and QBE. Creates and drops its own table.
import { connect } from "./cdp.mjs";

const cdp = await connect();
const checks = {};
const check = (n, ok, d) => { checks[n] = { ok: Boolean(ok), ...(d === undefined ? {} : { d }) }; };
const J = async (expr) => JSON.parse(await cdp.evaluate(`JSON.stringify(${expr})`));
const status = () => cdp.evaluate(`document.querySelector("[data-e2e='status-message']")?.innerText.trim() ?? ""`);
const setEditor = async (text) => {
  await cdp.evaluate(`document.querySelector('.cm-content')?.focus()`);
  await cdp.key("a", "KeyA", 65, 2);
  await cdp.send("Input.insertText", { text });
  await cdp.wait(150);
};
const doneAfter = (before) => `(() => {
  const t = document.querySelector("[data-e2e='status-message']")?.innerText ?? "";
  const finished = /\d+ filas? \u00b7 \d+ ms|ORA-\d+|error/i.test(t);
  return finished && t !== ${JSON.stringify(before)} ? t : "";
})()`;
const runSql = async (sql) => {
  const before = await status();
  await setEditor(sql);
  await cdp.key("F5", "F5", 116);
  return cdp.waitFor(doneAfter(before), 40000);
};
const gridRows = () => cdp.count(".grid tbody tr");

await cdp.reload();
await cdp.wait(1200);
await cdp.clickTitle("Conectar");
await cdp.waitFor(`/(Conectado|anamnesis@)/.test(document.querySelector("[data-e2e='conn-chip']")?.innerText ?? '')`, 20000);
await cdp.wait(1500);

// own table so nothing of the user's data is touched
await runSql("drop table sqlt_p2_e2e purge");
await runSql("create table sqlt_p2_e2e (n number, name varchar2(20))");
await runSql("insert into sqlt_p2_e2e values (1, 'uno')");
await runSql("insert into sqlt_p2_e2e values (2, 'dos')");
await runSql("commit");

// 1) a single-table select turns the grid editable and hides the ROWID
await runSql("select n, name from sqlt_p2_e2e order by n");
check("a single-table select produces an editable grid", (await cdp.count(".grid.editable")) === 1);
const headers = await J(`Array.from(document.querySelectorAll('.grid thead th')).map(h => h.innerText.trim())`);
check("the ROWID column is not shown", !headers.some((h) => /sqlt_rowid/i.test(h)), headers);
check("both rows are there", (await gridRows()) === 2, await gridRows());

// 2) edit a cell, buffer it, save it
await cdp.evaluate(`(() => { const td = document.querySelectorAll('.grid tbody tr')[0].querySelectorAll('td')[2]; td.dispatchEvent(new MouseEvent('dblclick', { bubbles: true })); })()`);
await cdp.waitFor(`Boolean(document.querySelector('.edit-input'))`, 5000);
const initial = await cdp.evaluate(`document.querySelector('.edit-input')?.value ?? ''`);
check("the editor opens with the current value", initial === "uno", initial);
await cdp.evaluate(`(() => { const i = document.querySelector('.edit-input'); Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(i, 'editado'); i.dispatchEvent(new Event('input', { bubbles: true })); })()`);
await cdp.evaluate(`Array.from(document.querySelectorAll('.palette .row button')).find(b => b.innerText.trim() === 'Aplicar')?.click()`);
await cdp.wait(400);
const pending = await cdp.evaluate(`document.querySelector('.pending-edits')?.innerText.trim() ?? ''`);
check("the change is buffered, not sent", /1/.test(pending) && (await cdp.count('.palette .edit-input')) === 0, pending);
await cdp.evaluate(`Array.from(document.querySelectorAll('.panel-bar button')).find(b => b.innerText.trim() === 'Guardar cambios')?.click()`);
const saved = await cdp.waitFor(
  `(() => { const t = document.querySelector("[data-e2e='status-message']")?.innerText ?? ""; return /filas guardadas/.test(t) ? t : ""; })()`,
  20000,
);
check("saving reports rows saved", /1 filas guardadas/.test(String(saved)), saved);
await cdp.wait(1200);
const savedValue = await cdp.evaluate(`document.querySelectorAll('.grid tbody tr')[0].querySelectorAll('td')[2]?.innerText.trim() ?? ''`);
check("the grid shows the saved value", savedValue === "editado", savedValue);

// 3) NULL through the editor
await cdp.evaluate(`(() => { const td = document.querySelectorAll('.grid tbody tr')[0].querySelectorAll('td')[2]; td.dispatchEvent(new MouseEvent('dblclick', { bubbles: true })); })()`);
await cdp.waitFor(`Boolean(document.querySelector('.edit-input'))`, 5000);
await cdp.evaluate(`Array.from(document.querySelectorAll('.palette .row button')).find(b => b.innerText.trim() === 'NULL')?.click()`);
await cdp.wait(300);
await cdp.evaluate(`Array.from(document.querySelectorAll('.panel-bar button')).find(b => b.innerText.trim() === 'Guardar cambios')?.click()`);
await cdp.waitFor(
  `(() => { const t = document.querySelector("[data-e2e='status-message']")?.innerText ?? ""; return /filas guardadas/.test(t) ? t : ""; })()`,
  20000,
);
await cdp.wait(1500);
const nullCell = await cdp.evaluate(`document.querySelectorAll('.grid tbody tr')[0].querySelectorAll('td')[2]?.className ?? ''`);
check("a cell can be set to NULL", /null/.test(nullCell), nullCell);

// 4) QBE filters the same query
await runSql("select n, name from sqlt_p2_e2e order by n");
await cdp.evaluate(`(() => { const i = document.querySelector('.qbe'); Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(i, 'n = 2'); i.dispatchEvent(new Event('input', { bubbles: true })); })()`);
await cdp.evaluate(`document.querySelector('.qbe').dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }))`);
await cdp.waitFor(`document.querySelectorAll('.grid tbody tr').length === 1`, 20000);
check("QBE filters the result to one row", (await gridRows()) === 1, await gridRows());

// 5) anything else stays read-only
await runSql("select a.n, b.n from sqlt_p2_e2e a, sqlt_p2_e2e b");
check(
  "a join is read-only",
  (await cdp.count(".grid.editable")) === 0 && (await cdp.count(".qbe")) === 1,
  { editable: await cdp.count(".grid.editable"), qbe: await cdp.count(".qbe") },
);
await runSql("drop table sqlt_p2_e2e purge");
check("the table is gone", (await cdp.evaluate(`document.querySelector("[data-e2e='status-message']")?.innerText ?? ''`)).length > 0);
cdp.close();
console.log(JSON.stringify(checks, null, 1));
const bad = Object.entries(checks).filter(([, v]) => !v.ok).map(([k]) => k);
console.log(bad.length ? "P2 CHECKS FAILED: " + bad.join("; ") : `P2 CHECKS OK (${Object.keys(checks).length})`);
process.exit(bad.length ? 1 : 0);
