// P9 screenshots: the new object form and the scheduler view.
import { connect } from "./cdp.mjs";
import path from "node:path";
const outDir = "C:/Users/sotelos/sqltools/docs/screenshots";
const cdp = await connect();
const openMenu = async (label) => {
  await cdp.click('.actionstrip button[aria-label="Más acciones"]');
  await cdp.wait(400);
  await cdp.evaluate(`Array.from(document.querySelectorAll('[role=menuitem]')).find(b => b.innerText.trim() === ${JSON.stringify(label)})?.click()`);
};
await cdp.reload(); await cdp.wait(1400);
await cdp.clickTitle("Conectar");
await cdp.waitFor(`/(Conectado|anamnesis@)/.test(document.querySelector("[data-e2e='conn-chip']")?.innerText ?? '')`, 20000);
await cdp.wait(2000);
for (const theme of ["light", "dark"]) {
  await cdp.evaluate(`localStorage.setItem('sqlt.theme', ${JSON.stringify(theme)})`);
  await cdp.wait(500);
  await cdp.click('.activity .act[aria-label="Tareas programadas"]');
  await cdp.waitFor(`document.querySelectorAll('.objects-panel .history-row').length > 0`, 20000);
  for (const [w, h] of [[1366, 768], [1024, 700], [1024, 640]]) {
    await cdp.setViewport(w, h); await cdp.wait(400);
    const file = path.join(outDir, `parity-p9-scheduler-${theme}-${w}x${h}.png`);
    await cdp.screenshot(file);
    const o = JSON.parse(await cdp.evaluate(`JSON.stringify({ docW: document.documentElement.scrollWidth, winW: innerWidth, docH: document.documentElement.scrollHeight, winH: innerHeight, jobs: document.querySelectorAll('.objects-panel .history-row').length })`));
    console.log(theme, w + "x" + h, JSON.stringify(o));
  }
  await cdp.clearViewport();
  await openMenu("Nuevo objeto…");
  await cdp.waitFor(`Boolean(document.querySelector('[data-e2e="ddl-preview"]'))`, 8000);
  await cdp.evaluate(`(() => { const i = document.querySelector('.new-object input[aria-label="Nombre"]'); Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(i, 'CLIENTES'); i.dispatchEvent(new Event('input', { bubbles: true })); })()`);
  await cdp.evaluate(`Array.from(document.querySelectorAll('.new-object .row button')).find(b => b.innerText.trim() === 'DDL')?.click()`);
  await cdp.wait(500);
  await cdp.setViewport(1366, 768); await cdp.wait(300);
  await cdp.screenshot(path.join(outDir, `parity-p9-new-object-${theme}-1366x768.png`));
  console.log(theme, "new object ok");
  await cdp.clearViewport();
  await cdp.evaluate(`document.querySelector('.palette-backdrop')?.click()`);
  await cdp.wait(300);
}
await cdp.evaluate(`localStorage.setItem('sqlt.theme','system')`);
cdp.close();
