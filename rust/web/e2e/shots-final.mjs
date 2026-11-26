// Final screenshots of the real app: Objects (tree with a package open), Connections, History, Stats and the
// palette, in both themes. node shots-final.mjs
import { connect } from "./cdp.mjs";
import path from "node:path";
const outDir = process.env.SHOT_DIR ?? "C:/Users/sotelos/sqltools/docs/screenshots";
const cdp = await connect();
const shot = async (name, w = 1366, h = 768) => {
  await cdp.setViewport(w, h); await cdp.wait(500);
  const file = path.join(outDir, `workbench-${name}-${w}x${h}.png`);
  await cdp.screenshot(file);
  const o = JSON.parse(await cdp.evaluate(`JSON.stringify({ docW: document.documentElement.scrollWidth, winW: innerWidth, docH: document.documentElement.scrollHeight, winH: innerHeight })`));
  console.log(path.basename(file), o.docW === o.winW && o.docH === o.winH ? "no overflow" : "OVERFLOW " + JSON.stringify(o));
};
const setVal = (sel, v) => cdp.evaluate(`(() => { const i = document.querySelector(${JSON.stringify(sel)}); Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(i, ${JSON.stringify(v)}); i.dispatchEvent(new Event('input', { bubbles: true })); })()`);
for (const theme of ["light", "dark"]) {
  await cdp.evaluate(`localStorage.setItem('sqlt.theme', ${JSON.stringify(theme)})`);
  await cdp.reload(); await cdp.wait(1500);
  await cdp.clickTitle("Conectar");
  await cdp.waitFor(`/(Conectado|anamnesis@)/.test(document.querySelector("[data-e2e='conn-chip']")?.innerText ?? '')`, 20000);
  await cdp.waitFor(`document.querySelectorAll('.tree-count').length > 0`, 20000);
  // run a query so history/stats/results have content
  await cdp.evaluate(`document.querySelector('.cm-content')?.focus()`);
  await cdp.key("a", "KeyA", 65, 2);
  await cdp.send("Input.insertText", { text: "select owner, object_type, count(*) n from all_objects where rownum <= 500 group by owner, object_type order by 3 desc" });
  await cdp.key("F5", "F5", 116);
  await cdp.waitFor(`document.querySelectorAll('.grid tbody tr').length > 0`, 20000);
  // Objects: expand packages, open the first one's spec and body
  await cdp.evaluate(`Array.from(document.querySelectorAll('[role=treeitem]')).find(r => /^Paquetes/.test(r.innerText.trim()))?.click()`);
  await cdp.waitFor(`document.querySelectorAll('[role=treeitem][data-type=PACKAGE][aria-level="2"]').length > 0`, 30000);
  await setVal('.objects-panel input[title="Filtro"]', "AS_PDF3_MOD");
  await cdp.wait(500);
  await cdp.evaluate(`document.querySelector('[role=treeitem][data-type=PACKAGE][aria-level="2"]')?.click()`);
  await cdp.wait(800);
  await cdp.evaluate(`document.querySelector('.cm-content')?.focus()`);
  await cdp.key("F12", "F12", 123, 2);
  await cdp.waitFor(`document.querySelectorAll('.editor-tab').length >= 3`, 90000, 200);
  await cdp.wait(800);
  await shot(`objects-${theme}`);
  if (theme === "light") await shot(`objects-${theme}`, 1024, 640);
  await cdp.click('.activity .act[aria-label="Historial"]'); await cdp.wait(600);
  await shot(`history-${theme}`);
  await cdp.click('.activity .act[aria-label="Estadísticas"]'); await cdp.wait(1200);
  await shot(`stats-${theme}`);
  await cdp.click('.activity .act[aria-label="Conexiones"]'); await cdp.wait(500);
  await shot(`connections-${theme}`);
  await cdp.click('.activity .act[aria-label="Objetos"]'); await cdp.wait(300);
  await cdp.click(".palette-btn"); await cdp.wait(400);
  await cdp.send("Input.insertText", { text: "form" }); await cdp.wait(500);
  await shot(`palette-${theme}`);
  await cdp.key("Escape", "Escape", 27);
  await cdp.clearViewport();
}
await cdp.evaluate(`localStorage.setItem('sqlt.theme','system')`);
cdp.close();