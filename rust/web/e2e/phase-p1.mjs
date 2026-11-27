// P1 checks on the installed app: dependencies, recompile (single and invalid) and the
// enable/disable actions. Objects are chosen by exact name, and the two names that only exist in the
// database are read from it first, so the test does not depend on hardcoded tables.
import { connect } from "./cdp.mjs";

const cdp = await connect();
const checks = {};
const check = (n, ok, d) => { checks[n] = { ok: Boolean(ok), ...(d === undefined ? {} : { d }) }; };
const J = async (expr) => JSON.parse(await cdp.evaluate(`JSON.stringify(${expr})`));
const status = () => cdp.evaluate(`document.querySelector("[data-e2e='status-message']")?.innerText.trim() ?? ""`);
const headerText = () => cdp.evaluate(`document.querySelector('.content header')?.innerText.replace(/\\s+/g, ' ').trim() ?? ""`);
const setField = (title, value) =>
  cdp.evaluate(`(() => { const i = document.querySelector('.objects-panel input[title=${JSON.stringify(title)}]'); if (!i) return false; Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(i, ${JSON.stringify(value)}); i.dispatchEvent(new Event('input', { bubbles: true })); return true; })()`);
const treeRow = (type, name) =>
  cdp.evaluate(`(() => { const r = Array.from(document.querySelectorAll('[role=treeitem][data-type=${JSON.stringify(type)}][aria-level="2"]')).find(x => x.innerText.trim() === ${JSON.stringify(name)}); if (!r) return false; r.click(); return true; })()`);
const expand = async (label) => {
  await cdp.evaluate(`(() => { const g = Array.from(document.querySelectorAll('[role=treeitem][aria-level="1"]')).find(r => r.innerText.trim().startsWith(${JSON.stringify(label)})); if (g && g.getAttribute('aria-expanded') === 'false') g.click(); })()`);
  await cdp.wait(600);
};
const waitStatus = async (re, ms) => {
  const deadline = Date.now() + ms;
  for (;;) {
    const value = await status();
    if (re.test(value)) return value;
    if (Date.now() > deadline) return value;
    await cdp.wait(200);
  }
};
async function runSql(sql) {
  await cdp.evaluate(`document.querySelector('.cm-content')?.focus()`);
  await cdp.key("a", "KeyA", 65, 2);
  await cdp.send("Input.insertText", { text: sql });
  await cdp.key("F5", "F5", 116);
  await cdp.waitFor(`document.querySelectorAll('.grid tbody tr').length > 0`, 20000);
  return cdp.evaluate(`document.querySelector('.grid tbody tr td:nth-child(2)')?.innerText.trim() ?? ""`);
}

await cdp.reload();
await cdp.wait(1200);
await cdp.clickTitle("Conectar");
await cdp.waitFor(`/(Conectado|anamnesis@)/.test(document.querySelector("[data-e2e='conn-chip']")?.innerText ?? '')`, 20000);
await cdp.waitFor(`document.querySelectorAll('.tree-count').length > 0`, 20000);
// The chip text can appear a moment before the session is usable: wait for the Run button to enable.
await cdp.waitFor(`(() => { const b = document.querySelector('.actionstrip button[aria-label^="Ejecutar"]'); return Boolean(b) && !b.disabled; })()`, 20000);

// Names that exist in this database right now (no hardcoded tables).
const constraintTable = await runSql(
  "select table_name from user_constraints where constraint_type = 'C' and rownum = 1",
);
const triggerTable = await runSql(
  "select table_name from user_triggers where rownum = 1",
);
check("the database answered the two probe queries", Boolean(constraintTable) && Boolean(triggerTable), {
  constraintTable,
  triggerTable,
});

// 1) Dependencies of a table that has dependents (A11 is the flow's fixed table and has two).
await expand("Tablas");
await setField("Filtro", "A11");
await cdp.wait(500);
check("the table row can be opened", await treeRow("TABLE", "A11"));
await cdp.wait(900);
check("the object panel shows A11", /\bA11\b/.test(await headerText()), await headerText());
const tabs = await J(`Array.from(document.querySelectorAll('.object-tabs button')).map(b => b.innerText.trim())`);
check("the object panel has a Dependencies tab", tabs.includes("Dependencias"), tabs);
await cdp.evaluate(`Array.from(document.querySelectorAll('.object-tabs button')).find(b => b.innerText.trim() === 'Dependencias')?.click()`);
await cdp.waitFor(`Array.from(document.querySelectorAll('.content .grid tbody td')).some(td => /Dependientes|Referenciados/.test(td.innerText))`, 15000);
const deps = await J(`Array.from(document.querySelectorAll('.content .grid tbody tr')).map(r => r.innerText.replace(/\\s+/g, ' ').trim())`);
check("A11 lists the objects that depend on it", deps.length > 0 && deps.some((d) => /Dependientes/.test(d)), deps.slice(0, 3));

// 2) Context menu offers Dependencies and Recompile on a tree object.
await cdp.evaluate(`(() => { const row = Array.from(document.querySelectorAll('[role=treeitem][data-type=TABLE][aria-level="2"]')).find(x => x.innerText.trim() === 'A11'); const r = row.getBoundingClientRect(); row.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, clientX: r.left + 40, clientY: r.top + 8 })); })()`);
await cdp.wait(300);
const menu = await J(`Array.from(document.querySelectorAll('.context-menu button')).map(b => b.innerText.trim())`);
check("the context menu offers Dependencies and Recompile", menu.includes("Dependencias") && menu.includes("Recompilar"), menu);
await cdp.evaluate(`document.querySelector('.menu-backdrop')?.click()`);
await cdp.wait(300);

// 3) Recompile from the object header: a package compiles without errors.
await setField("Filtro", "AS_PDF3_MOD");
await expand("Paquetes");
await cdp.waitFor(`document.querySelectorAll('[role=treeitem][data-type=PACKAGE][aria-level="2"]').length > 0`, 25000);
check("the package row can be opened", await treeRow("PACKAGE", "AS_PDF3_MOD"));
await cdp.wait(900);
check("the object panel shows the package", /AS_PDF3_MOD/.test(await headerText()), await headerText());
await cdp.evaluate(`Array.from(document.querySelectorAll('.content header button')).find(b => b.innerText.trim() === 'Recompilar')?.click()`);
const compiled = await waitStatus(/Compilado sin errores|Errores de compilación/, 40000);
check("recompiling a valid package reports no errors", /Compilado sin errores/.test(compiled), compiled);

// 4) Recompile-invalid: the button carries the count and reports the outcome.
const invalidButton = await J(`(() => { const b = document.querySelector('.objects-panel button[aria-label="Recompilar objetos inválidos"]'); return { present: Boolean(b), title: b?.title ?? '', disabled: b?.disabled ?? null }; })()`);
check("the objects panel has a recompile-invalid button with its count", invalidButton.present && /\(\d+\)/.test(invalidButton.title), invalidButton);
if (invalidButton.present && invalidButton.disabled === false) {
  await cdp.evaluate(`document.querySelector('.objects-panel button[aria-label="Recompilar objetos inválidos"]')?.click()`);
  const outcome = await waitStatus(/Recompilados \d+; quedan \d+ inválidos/, 90000);
  check("recompiling invalid objects reports attempted and remaining", /Recompilados \d+; quedan \d+ inválidos/.test(outcome), outcome);
}

// 5) Enable/disable actions on a real table with a constraint and on a real trigger.
await setField("Filtro", constraintTable);
await expand("Tablas");
await cdp.wait(500);
check(`the table ${constraintTable} can be opened`, await treeRow("TABLE", constraintTable));
await cdp.wait(1200);
await cdp.evaluate(`Array.from(document.querySelectorAll('.object-tabs button')).find(b => b.innerText.trim() === 'Restricciones')?.click()`);
await cdp.waitFor(`document.querySelectorAll('.content .grid tbody tr').length > 0`, 20000);
const constraintButtons = await cdp.count(".content .grid .link-btn");
check("each constraint row has an enable/disable action", constraintButtons > 0, { table: constraintTable, buttons: constraintButtons });

await setField("Filtro", triggerTable);
await expand("Tablas");
await cdp.wait(500);
check(`the table ${triggerTable} can be opened`, await treeRow("TABLE", triggerTable));
await cdp.wait(1200);
await cdp.evaluate(`Array.from(document.querySelectorAll('.object-tabs button')).find(b => b.innerText.trim() === 'Triggers')?.click()`);
await cdp.waitFor(`document.querySelectorAll('.content .grid .link-btn').length > 0`, 20000);
const triggerButtons = await cdp.count(".content .grid .link-btn");
check("trigger rows have an enable/disable action", triggerButtons > 0, { table: triggerTable, buttons: triggerButtons });
cdp.close();
console.log(JSON.stringify(checks, null, 1));
const bad = Object.entries(checks).filter(([, v]) => !v.ok).map(([k]) => k);
console.log(bad.length ? "P1 CHECKS FAILED: " + bad.join("; ") : `P1 CHECKS OK (${Object.keys(checks).length})`);
process.exit(bad.length ? 1 : 0);
