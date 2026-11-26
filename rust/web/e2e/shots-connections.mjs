// Screenshots of the Connections view with a filled environment: node shots-connections.mjs <prefix>
import { connect } from "./cdp.mjs";
import path from "node:path";
const prefix = process.argv[2] ?? "workbench-connections";
const outDir = process.env.SHOT_DIR ?? "C:/Users/sotelos/sqltools/docs/screenshots";
const cdp = await connect();
for (const theme of ["light", "dark"]) {
  await cdp.evaluate(`localStorage.setItem('sqlt.theme', ${JSON.stringify(theme)})`);
  await cdp.reload(); await cdp.wait(1500);
  await cdp.click('.activity .act[aria-label="Conexiones"]'); await cdp.wait(300);
  await cdp.evaluate(`(() => { const f = Array.from(document.querySelectorAll('.objects-panel .field-row')).find(x => x.querySelector('span')?.innerText.trim() === 'Etiqueta de entorno'); const i = f?.querySelector('input'); Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(i, 'PROD'); i.dispatchEvent(new Event('input', { bubbles: true })); })()`);
  await cdp.evaluate(`document.querySelector('.swatch[data-color="red"]')?.click()`);
  for (const [w, h] of [[1366, 768], [1024, 640]]) {
    await cdp.setViewport(w, h); await cdp.wait(400);
    await cdp.evaluate(`document.querySelector('.conn-view')?.scrollTo(0, 99999)`);
    await cdp.wait(200);
    const file = path.join(outDir, `${prefix}-${theme}-${w}x${h}.png`);
    await cdp.screenshot(file);
    console.log(file, await cdp.evaluate(`JSON.stringify({ docW: document.documentElement.scrollWidth, winW: innerWidth, docH: document.documentElement.scrollHeight, winH: innerHeight })`));
  }
  await cdp.clearViewport();
}
await cdp.evaluate(`localStorage.setItem('sqlt.theme','system')`);
cdp.close();