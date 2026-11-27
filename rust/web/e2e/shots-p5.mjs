// P5 screenshots: the profiler output at the three sizes, both themes.
import { connect } from "./cdp.mjs";
import path from "node:path";
const outDir = "C:/Users/sotelos/sqltools/docs/screenshots";
const cdp = await connect();
const status = () => cdp.evaluate(`document.querySelector("[data-e2e='status-message']")?.innerText ?? ""`);
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
    if (t !== before && /\d+ filas?|ORA-/i.test(t)) return;
    await cdp.wait(250);
  }
};
await cdp.reload(); await cdp.wait(1400);
await cdp.clickTitle("Conectar");
await cdp.waitFor(`/(Conectado|anamnesis@)/.test(document.querySelector("[data-e2e='conn-chip']")?.innerText ?? '')`, 20000);
await cdp.wait(1500);
await runSql("begin execute immediate 'drop procedure sqlt_p5_shot'; exception when others then null; end;");
await runSql("create or replace procedure sqlt_p5_shot (p_rows in number) as l number := 0; begin for i in 1 .. p_rows loop l := l + i; end loop; for j in 1 .. p_rows loop l := l + j * 2; end loop; end;");
await setEditor("begin sqlt_p5_shot(20000); end;");
const profile = async () => {
  await cdp.click('.actionstrip button[aria-label="Más acciones"]');
  await cdp.wait(300);
  await cdp.evaluate(`Array.from(document.querySelectorAll('[role=menuitem]')).find(b => b.innerText.trim() === 'Perfilar sentencia')?.click()`);
  await cdp.waitFor(`/con tiempo|with time/.test(document.querySelector("[data-e2e='status-message']")?.innerText ?? '')`, 60000);
};
for (const theme of ["light", "dark"]) {
  await cdp.evaluate(`localStorage.setItem('sqlt.theme', ${JSON.stringify(theme)})`);
  await cdp.wait(500);
  for (const [w, h] of [[1366, 768], [1024, 700], [1024, 640]]) {
    await cdp.setViewport(w, h); await cdp.wait(400);
    await profile();
    const file = path.join(outDir, `parity-p5-profiler-${theme}-${w}x${h}.png`);
    await cdp.screenshot(file);
    const o = JSON.parse(await cdp.evaluate(`JSON.stringify({ docW: document.documentElement.scrollWidth, winW: innerWidth, docH: document.documentElement.scrollHeight, winH: innerHeight, output: (document.querySelector('.content')?.innerText ?? '').includes('SQLT_P5_SHOT') })`));
    console.log(theme, w + "x" + h, JSON.stringify(o));
  }
  await cdp.clearViewport();
}
await runSql("begin execute immediate 'drop procedure sqlt_p5_shot'; end;");
await runSql("delete from plsql_profiler_data where runid = (select max(runid) from plsql_profiler_runs)");
await runSql("delete from plsql_profiler_units where runid = (select max(runid) from plsql_profiler_runs)");
await runSql("delete from plsql_profiler_runs where runid = (select max(runid) from plsql_profiler_runs)");
await cdp.evaluate(`localStorage.setItem('sqlt.theme','system')`);
cdp.close();
