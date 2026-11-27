// P8 screenshots: command window with a result and the projects dialog, three sizes, both themes.
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
  await openMenu("Ventana de comandos");
  await cdp.waitFor(`Boolean(document.querySelector('[data-e2e="command-log"]'))`, 8000);
  await cdp.evaluate(`(() => { const i = document.querySelector('.command-window input'); Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(i, 'select owner, count(*) n from all_objects group by owner order by 2 desc'); i.dispatchEvent(new Event('input', { bubbles: true })); i.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true })); })()`);
  await cdp.waitFor(`/fila/.test(document.querySelector('[data-e2e="command-log"]')?.innerText ?? '')`, 30000);
  for (const [w, h] of [[1366, 768], [1024, 700], [1024, 640]]) {
    await cdp.setViewport(w, h); await cdp.wait(400);
    const file = path.join(outDir, `parity-p8-command-window-${theme}-${w}x${h}.png`);
    await cdp.screenshot(file);
    const o = JSON.parse(await cdp.evaluate(`JSON.stringify({ docW: document.documentElement.scrollWidth, winW: innerWidth, docH: document.documentElement.scrollHeight, winH: innerHeight, log: (document.querySelector('[data-e2e="command-log"]')?.innerText ?? '').split('\\n').length })`));
    console.log(theme, w + "x" + h, JSON.stringify(o));
  }
  await cdp.clearViewport();
  await cdp.evaluate(`document.querySelector('.palette-backdrop')?.click()`);
  await cdp.wait(300);
  await openMenu("Proyectos…");
  await cdp.waitFor(`Boolean(document.querySelector('.palette input[aria-label="Nombre del proyecto"]'))`, 8000);
  await cdp.setViewport(1366, 768); await cdp.wait(300);
  const file = path.join(outDir, `parity-p8-projects-${theme}-1366x768.png`);
  await cdp.screenshot(file);
  console.log(theme, "projects ok");
  await cdp.clearViewport();
  await cdp.evaluate(`document.querySelector('.palette-backdrop')?.click()`);
  await cdp.wait(300);
}
await cdp.evaluate(`localStorage.setItem('sqlt.theme','system')`);
cdp.close();
