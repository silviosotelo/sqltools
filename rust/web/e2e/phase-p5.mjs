// P5 checks on the installed app: profile a synthetic procedure from the editor and read the per-line
// times in Output. Creates and drops its own procedure and removes its profiler run.
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
    if (t !== before && /\d+ filas?|ORA-|error/i.test(t)) return;
    await cdp.wait(250);
  }
};

await cdp.reload();
await cdp.wait(1200);
await cdp.clickTitle("Conectar");
await cdp.waitFor(`/(Conectado|anamnesis@)/.test(document.querySelector("[data-e2e='conn-chip']")?.innerText ?? '')`, 20000);
await cdp.wait(1500);

// own procedure: two loops so several lines get time
await runSql("begin execute immediate 'drop procedure sqlt_p5_e2e'; exception when others then null; end;");
await runSql(
  "create or replace procedure sqlt_p5_e2e (p_rows in number) as l number := 0; begin for i in 1 .. p_rows loop l := l + i; end loop; for j in 1 .. p_rows loop l := l + j * 2; end loop; end;",
);

// profile it through the menu, with the call in the editor
await setEditor("begin sqlt_p5_e2e(20000); end;");
await cdp.click('.actionstrip button[aria-label="Más acciones"]');
await cdp.wait(400);
const menu = await J(`Array.from(document.querySelectorAll('[role=menuitem]')).map(b => b.innerText.trim())`);
check("the menu offers profiling the statement", menu.includes("Perfilar sentencia"), menu);
await cdp.evaluate(`Array.from(document.querySelectorAll('[role=menuitem]')).find(b => b.innerText.trim() === 'Perfilar sentencia')?.click()`);
await cdp.waitFor(`/linea\\(s\\) con tiempo|line\\(s\\) with time/.test(document.querySelector("[data-e2e='status-message']")?.innerText ?? '')`, 60000);
const summary = await status();
const outputText = await cdp.evaluate(`document.querySelector('.content')?.innerText ?? ''`);
check("profiling reports lines with time", /\d+ linea\(s\) con tiempo · [\d.]+ (ns|us|ms|s)/.test(summary), summary);
check("the per-line times are shown for the own unit", /SQLT_P5_E2E:\d+/.test(outputText), outputText.slice(0, 300).replace(/\s+/g, " "));

// clean up: the procedure and the profiler rows of this run
await runSql("begin execute immediate 'drop procedure sqlt_p5_e2e'; end;");
await runSql("delete from plsql_profiler_data where runid = (select max(runid) from plsql_profiler_runs)");
await runSql("delete from plsql_profiler_units where runid = (select max(runid) from plsql_profiler_runs)");
await runSql("delete from plsql_profiler_runs where runid = (select max(runid) from plsql_profiler_runs)");
cdp.close();
console.log(JSON.stringify(checks, null, 1));
const bad = Object.entries(checks).filter(([, v]) => !v.ok).map(([k]) => k);
console.log(bad.length ? "P5 CHECKS FAILED: " + bad.join("; ") : `P5 CHECKS OK (${Object.keys(checks).length})`);
process.exit(bad.length ? 1 : 0);
