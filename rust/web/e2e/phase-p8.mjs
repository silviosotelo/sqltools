// P8 checks on the installed app: projects (save current, reopen, delete) and the command window.
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
  await cdp.wait(200);
};
const openMenu = async (label) => {
  await cdp.click('.actionstrip button[aria-label="Más acciones"]');
  await cdp.wait(400);
  await cdp.evaluate(`Array.from(document.querySelectorAll('[role=menuitem]')).find(b => b.innerText.trim() === ${JSON.stringify(label)})?.click()`);
};

await cdp.reload();
await cdp.wait(1200);
await cdp.clickTitle("Conectar");
await cdp.waitFor(`/(Conectado|anamnesis@)/.test(document.querySelector("[data-e2e='conn-chip']")?.innerText ?? '')`, 20000);
await cdp.wait(1200);

// 1) a project stores the open buffer and gets it back
await setEditor("select 1 as proyecto from dual");
await openMenu("Proyectos…");
await cdp.waitFor(`Boolean(document.querySelector('.palette input[aria-label="Nombre del proyecto"]'))`, 8000);
check("the projects dialog opens with a name field", true);
await cdp.evaluate(`(() => { const i = document.querySelector('.palette input[aria-label="Nombre del proyecto"]'); Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(i, 'sqlt_p8_e2e'); i.dispatchEvent(new Event('input', { bubbles: true })); })()`);
await cdp.evaluate(`Array.from(document.querySelectorAll('.palette .row button')).find(b => b.innerText.trim() === 'Guardar el proyecto actual')?.click()`);
await cdp.waitFor(`/sqlt_p8_e2e/.test(document.querySelector('.palette')?.innerText ?? '')`, 15000);
const listed = await J(`Array.from(document.querySelectorAll('.palette .report-row')).map(r => r.innerText.replace(/\\s+/g, ' ').trim())`);
check("the project appears with its file count", listed.some((row) => /sqlt_p8_e2e · 1 archivo\(s\)/.test(row)), listed);

// 2) changing the editor and reopening the project restores it
await cdp.evaluate(`document.querySelector('.palette-backdrop')?.click()`);
await cdp.wait(300);
await setEditor("-- something else\nselect 2 from dual");
await openMenu("Proyectos…");
await cdp.waitFor(`Boolean(document.querySelector('.palette .report-row'))`, 8000);
await cdp.evaluate(`Array.from(document.querySelectorAll('.palette .report-row')).find(r => r.innerText.includes('sqlt_p8_e2e'))?.querySelector('button')?.click()`);
await cdp.waitFor(`/proyecto/.test(document.querySelector('.cm-content')?.innerText ?? '')`, 10000);
const restored = await cdp.evaluate(`document.querySelector('.cm-content')?.innerText ?? ''`);
check("opening the project restores its buffer", /select 1 as proyecto from dual/.test(restored), restored.replace(/\s+/g, " ").slice(0, 120));

// 3) the command window runs a statement and shows the result
await openMenu("Ventana de comandos");
await cdp.waitFor(`Boolean(document.querySelector('[data-e2e="command-log"]'))`, 8000);
await cdp.evaluate(`(() => { const i = document.querySelector('.command-window input'); Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(i, 'select 7 as siete from dual'); i.dispatchEvent(new Event('input', { bubbles: true })); i.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true })); })()`);
await cdp.waitFor(`/1 fila/.test(document.querySelector('[data-e2e="command-log"]')?.innerText ?? '')`, 20000);
const log = await cdp.evaluate(`document.querySelector('[data-e2e="command-log"]')?.innerText ?? ''`);
check("the command window runs SQL and shows the rows", /SQL> select 7 as siete from dual/.test(log) && /\b7\b/.test(log) && /1 fila/.test(log), log.replace(/\s+/g, " ").slice(0, 160));
await cdp.evaluate(`document.querySelector('.palette-backdrop')?.click()`);
await cdp.wait(300);

// 4) delete the project
await openMenu("Proyectos…");
await cdp.waitFor(`Boolean(document.querySelector('.palette .report-row'))`, 8000);
await cdp.evaluate(`Array.from(document.querySelectorAll('.palette .report-row')).find(r => r.innerText.includes('sqlt_p8_e2e'))?.querySelector('button:last-child')?.click()`);
await cdp.waitFor(`!/sqlt_p8_e2e/.test(document.querySelector('.palette')?.innerText ?? '')`, 10000);
check("deleting removes the project", !(await cdp.evaluate(`/sqlt_p8_e2e/.test(document.querySelector('.palette')?.innerText ?? '')`)));
cdp.close();
console.log(JSON.stringify(checks, null, 1));
const bad = Object.entries(checks).filter(([, v]) => !v.ok).map(([k]) => k);
console.log(bad.length ? "P8 CHECKS FAILED: " + bad.join("; ") : `P8 CHECKS OK (${Object.keys(checks).length})`);
process.exit(bad.length ? 1 : 0);
