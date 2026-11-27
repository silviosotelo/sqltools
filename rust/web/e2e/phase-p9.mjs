// P9 checks on the installed app: the DDL forms (preview + insert, never execute) and the scheduler view.
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
const runSql = async (sql) => {
  const before = await status();
  await setEditor(sql);
  await cdp.key("F5", "F5", 116);
  for (let i = 0; i < 80; i++) {
    const t = await status();
    if (t !== before && /\d+ filas?|ORA-|error/i.test(t)) return t;
    await cdp.wait(250);
  }
  return await status();
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
await cdp.waitFor(`(() => { const b = document.querySelector('.actionstrip button[aria-label^="Ejecutar"]'); return Boolean(b) && !b.disabled; })()`, 20000);

// 1) the scheduler view lists a real DBMS_SCHEDULER job
await runSql("begin execute immediate 'drop job sqlt_p9_e2e'; exception when others then null; end;");
await runSql("begin dbms_scheduler.create_job(job_name => 'SQLT_P9_E2E', job_type => 'PLSQL_BLOCK', job_action => 'begin null; end;', enabled => false); end;");
const views = await J(`Array.from(document.querySelectorAll('.activity .act')).map(b => b.getAttribute('aria-label'))`);
check("the activity bar has a Scheduler view", views.includes("Tareas programadas"), views);
await cdp.click('.activity .act[aria-label="Tareas programadas"]');
await cdp.waitFor(`/SQLT_P9_E2E/.test(document.querySelector('.objects-panel')?.innerText ?? '')`, 20000);
const jobs = await J(`Array.from(document.querySelectorAll('.objects-panel .history-row')).map(r => r.innerText.replace(/\\s+/g, ' ').trim())`);
check("the scheduler view shows the job with its state", jobs.some((job) => /SQLT_P9_E2E .* deshabilitada/.test(job)), jobs.slice(0, 3));

// 2) the table form previews the DDL and only inserts it into the editor
await openMenu("Nuevo objeto…");
await cdp.waitFor(`Boolean(document.querySelector('[data-e2e="ddl-preview"]'))`, 8000);
await cdp.evaluate(`(() => { const i = document.querySelector('.new-object input[aria-label="Nombre"]'); Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(i, 'SQLT_P9_E2E'); i.dispatchEvent(new Event('input', { bubbles: true })); })()`);
await cdp.evaluate(`Array.from(document.querySelectorAll('.new-object .row button')).find(b => b.innerText.trim() === 'DDL')?.click()`);
await cdp.wait(500);
const tablePreview = await cdp.evaluate(`document.querySelector('[data-e2e="ddl-preview"]')?.innerText ?? ''`);
check("the table form builds CREATE TABLE with the PK", /create table SQLT_P9_E2E/.test(tablePreview) && /ID NUMBER NOT NULL/.test(tablePreview) && /PRIMARY KEY \(ID\)/.test(tablePreview), tablePreview.replace(/\s+/g, " ").slice(0, 140));

const beforeInsert = await status();
await cdp.evaluate(`Array.from(document.querySelectorAll('.new-object .row button')).find(b => b.innerText.trim() === 'Insertar en el editor')?.click()`);
await cdp.waitFor(`/create table SQLT_P9_E2E/.test(document.querySelector('.cm-content')?.innerText ?? '')`, 10000);
const editorText = await cdp.evaluate(`document.querySelector('.cm-content')?.innerText ?? ''`);
const afterInsert = await status();
check("inserting puts the DDL in the editor", /create table SQLT_P9_E2E/.test(editorText), editorText.replace(/\s+/g, " ").slice(0, 100));
check("the form never executes the statement", !/0 filas|1 fila/.test(afterInsert) && afterInsert !== beforeInsert, { beforeInsert, afterInsert });

// 3) the sequence form builds CREATE SEQUENCE
await openMenu("Nuevo objeto…");
await cdp.waitFor(`Boolean(document.querySelector('[data-e2e="ddl-preview"]'))`, 8000);
await cdp.evaluate(`(() => { const s = document.querySelector('.new-object select[aria-label="Nuevo objeto…"]'); Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, 'value').set.call(s, 'sequence'); s.dispatchEvent(new Event('change', { bubbles: true })); })()`);
await cdp.evaluate(`(() => { const i = document.querySelector('.new-object input[aria-label="Nombre"]'); Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(i, 'SQLT_P9_SEQ'); i.dispatchEvent(new Event('input', { bubbles: true })); })()`);
await cdp.evaluate(`Array.from(document.querySelectorAll('.new-object .row button')).find(b => b.innerText.trim() === 'DDL')?.click()`);
await cdp.wait(500);
const sequencePreview = await cdp.evaluate(`document.querySelector('[data-e2e="ddl-preview"]')?.innerText ?? ''`);
check("the sequence form builds CREATE SEQUENCE", /create sequence SQLT_P9_SEQ start with 1 increment by 1/.test(sequencePreview), sequencePreview.replace(/\s+/g, " ").slice(0, 120));
await cdp.evaluate(`document.querySelector('.palette-backdrop')?.click()`);
await cdp.wait(300);

await runSql("begin execute immediate 'drop job sqlt_p9_e2e'; end;");
cdp.close();
console.log(JSON.stringify(checks, null, 1));
const bad = Object.entries(checks).filter(([, v]) => !v.ok).map(([k]) => k);
console.log(bad.length ? "P9 CHECKS FAILED: " + bad.join("; ") : `P9 CHECKS OK (${Object.keys(checks).length})`);
process.exit(bad.length ? 1 : 0);
