import { connect } from "./cdp.mjs";
import path from "node:path";
const outDir = "C:/Users/sotelos/sqltools/docs/screenshots";
const cdp = await connect();
await cdp.reload(); await cdp.wait(1400);
await cdp.clickTitle("Conectar");
await cdp.waitFor(`/(Conectado|anamnesis@)/.test(document.querySelector("[data-e2e='conn-chip']")?.innerText ?? '')`, 20000);
await cdp.waitFor(`document.querySelectorAll('.tree-count').length > 0`, 20000);
await cdp.evaluate(`Array.from(document.querySelectorAll('[role=treeitem]')).find(r => /^Tablas/.test(r.innerText.trim()))?.click()`);
await cdp.waitFor(`document.querySelectorAll('[role=treeitem][data-type=TABLE][aria-level="2"]').length > 0`, 20000);
await cdp.evaluate(`(() => { const i = document.querySelector('.objects-panel input[title="Filtro"]'); Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(i, 'A11'); i.dispatchEvent(new Event('input', { bubbles: true })); })()`);
await cdp.wait(500);
await cdp.evaluate(`Array.from(document.querySelectorAll('[role=treeitem][data-type=TABLE][aria-level="2"]')).find(r => r.innerText.trim() === 'A11')?.click()`);
await cdp.wait(900);
await cdp.evaluate(`Array.from(document.querySelectorAll('.object-tabs button')).find(b => b.innerText.trim() === 'Dependencias')?.click()`);
await cdp.wait(2000);
for (const theme of ["light", "dark"]) {
  await cdp.evaluate(`localStorage.setItem('sqlt.theme', ${JSON.stringify(theme)})`);
  await cdp.reload(); await cdp.wait(1400);
  await cdp.clickTitle("Conectar");
  await cdp.waitFor(`/(Conectado|anamnesis@)/.test(document.querySelector("[data-e2e='conn-chip']")?.innerText ?? '')`, 20000);
  await cdp.waitFor(`document.querySelectorAll('.tree-count').length > 0`, 20000);
  await cdp.evaluate(`Array.from(document.querySelectorAll('[role=treeitem]')).find(r => /^Tablas/.test(r.innerText.trim()))?.click()`);
  await cdp.waitFor(`document.querySelectorAll('[role=treeitem][data-type=TABLE][aria-level="2"]').length > 0`, 20000);
  await cdp.evaluate(`(() => { const i = document.querySelector('.objects-panel input[title="Filtro"]'); Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(i, 'A11'); i.dispatchEvent(new Event('input', { bubbles: true })); })()`);
  await cdp.wait(500);
  await cdp.evaluate(`Array.from(document.querySelectorAll('[role=treeitem][data-type=TABLE][aria-level="2"]')).find(r => r.innerText.trim() === 'A11')?.click()`);
  await cdp.wait(900);
  await cdp.evaluate(`Array.from(document.querySelectorAll('.object-tabs button')).find(b => b.innerText.trim() === 'Dependencias')?.click()`);
  await cdp.wait(1800);
  for (const [w, h] of [[1366, 768], [1024, 700], [1024, 640]]) {
    await cdp.setViewport(w, h); await cdp.wait(500);
    const file = path.join(outDir, `parity-p1-dependencies-${theme}-${w}x${h}.png`);
    await cdp.screenshot(file);
    const o = JSON.parse(await cdp.evaluate(`JSON.stringify({ docW: document.documentElement.scrollWidth, winW: innerWidth, docH: document.documentElement.scrollHeight, winH: innerHeight, clipped: Array.from(document.querySelectorAll('.object-tabs button, .actionstrip button')).filter(e => { const r = e.getBoundingClientRect(); return r.right > innerWidth + 0.5 || r.bottom > innerHeight + 0.5; }).length })`));
    console.log(theme, w + "x" + h, JSON.stringify(o));
  }
  await cdp.clearViewport();
}
await cdp.evaluate(`localStorage.setItem('sqlt.theme','system')`);
cdp.close();
