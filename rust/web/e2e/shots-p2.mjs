import { connect } from "./cdp.mjs";
import path from "node:path";
const outDir = "C:/Users/sotelos/sqltools/docs/screenshots";
const cdp = await connect();
const setEditor = async (text) => {
  await cdp.evaluate(`document.querySelector('.cm-content')?.focus()`);
  await cdp.key("a", "KeyA", 65, 2);
  await cdp.send("Input.insertText", { text });
  await cdp.wait(150);
};
const status = () => cdp.evaluate(`document.querySelector("[data-e2e='status-message']")?.innerText ?? ""`);
const run = async (sql) => {
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
await run("drop table sqlt_p2_shot purge");
await run("create table sqlt_p2_shot (n number, name varchar2(20))");
await run("insert into sqlt_p2_shot values (1, 'uno')");
await run("insert into sqlt_p2_shot values (2, 'dos')");
await run("commit");
await run("select n, name from sqlt_p2_shot order by n");
for (const theme of ["light", "dark"]) {
  await cdp.evaluate(`localStorage.setItem('sqlt.theme', ${JSON.stringify(theme)})`);
  await cdp.wait(600);
  for (const [w, h] of [[1366, 768], [1024, 700], [1024, 640]]) {
    await cdp.setViewport(w, h); await cdp.wait(400);
    const file = path.join(outDir, `parity-p2-editable-grid-${theme}-${w}x${h}.png`);
    await cdp.screenshot(file);
    const o = JSON.parse(await cdp.evaluate(`JSON.stringify({ docW: document.documentElement.scrollWidth, winW: innerWidth, docH: document.documentElement.scrollHeight, winH: innerHeight, clipped: Array.from(document.querySelectorAll('.qbe, .panel-bar button')).filter(e => { const r = e.getBoundingClientRect(); return r.right > innerWidth + 0.5 || r.bottom > innerHeight + 0.5; }).length })`));
    console.log(theme, w + "x" + h, JSON.stringify(o));
  }
  await cdp.clearViewport();
}
await run("drop table sqlt_p2_shot purge");
await cdp.evaluate(`localStorage.setItem('sqlt.theme','system')`);
cdp.close();
