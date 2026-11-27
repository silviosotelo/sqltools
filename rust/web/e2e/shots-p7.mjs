// P7 screenshots: files view and reports dialog, three sizes, both themes.
import { connect } from "./cdp.mjs";
import path from "node:path";
const outDir = "C:/Users/sotelos/sqltools/docs/screenshots";
const cdp = await connect();
await cdp.reload(); await cdp.wait(1400);
await cdp.clickTitle("Conectar");
await cdp.waitFor(`/(Conectado|anamnesis@)/.test(document.querySelector("[data-e2e='conn-chip']")?.innerText ?? '')`, 20000);
await cdp.wait(1200);
for (const theme of ["light", "dark"]) {
  await cdp.evaluate(`localStorage.setItem('sqlt.theme', ${JSON.stringify(theme)})`);
  await cdp.wait(500);
  await cdp.click('.activity .act[aria-label="Archivos"]');
  await cdp.waitFor(`document.querySelectorAll('.objects-panel .conn-item').length > 0`, 15000);
  await cdp.evaluate(`document.querySelector('.objects-panel .conn-item')?.click()`);
  await cdp.wait(1000);
  for (const [w, h] of [[1366, 768], [1024, 700], [1024, 640]]) {
    await cdp.setViewport(w, h); await cdp.wait(400);
    const file = path.join(outDir, `parity-p7-files-${theme}-${w}x${h}.png`);
    await cdp.screenshot(file);
    const o = JSON.parse(await cdp.evaluate(`JSON.stringify({ docW: document.documentElement.scrollWidth, winW: innerWidth, docH: document.documentElement.scrollHeight, winH: innerHeight, entries: document.querySelectorAll('.objects-panel ul[aria-label="Archivos"] .conn-item').length })`));
    console.log(theme, w + "x" + h, JSON.stringify(o));
  }
  await cdp.clearViewport();
  await cdp.click('.actionstrip button[aria-label="Más acciones"]');
  await cdp.wait(300);
  await cdp.evaluate(`Array.from(document.querySelectorAll('[role=menuitem]')).find(b => b.innerText.trim() === 'Informes…')?.click()`);
  await cdp.waitFor(`Boolean(document.querySelector('.palette[role="dialog"]'))`, 8000);
  const file = path.join(outDir, `parity-p7-reports-${theme}-1366x768.png`);
  await cdp.screenshot(file);
  console.log(theme, "reports", "ok");
  await cdp.evaluate(`document.querySelector('.palette-backdrop')?.click()`);
  await cdp.wait(300);
}
await cdp.evaluate(`localStorage.setItem('sqlt.theme','system')`);
cdp.close();
