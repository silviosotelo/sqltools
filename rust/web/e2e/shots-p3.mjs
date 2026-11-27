// P3 screenshots: the completion popup open, in both themes, at the three sizes. Also checks that the
// page does not scroll and nothing is clipped.
import { connect } from "./cdp.mjs";
import path from "node:path";
const outDir = "C:/Users/sotelos/sqltools/docs/screenshots";
const cdp = await connect();
await cdp.reload(); await cdp.wait(1400);
await cdp.clickTitle("Conectar");
await cdp.waitFor(`/(Conectado|anamnesis@)/.test(document.querySelector("[data-e2e='conn-chip']")?.innerText ?? '')`, 20000);
await cdp.wait(2500);
await cdp.evaluate(`document.querySelector('.cm-content')?.focus()`);
await cdp.key("a", "KeyA", 65, 2);
await cdp.send("Input.insertText", { text: "select * from a11 where " });
await cdp.wait(300);
for (const theme of ["light", "dark"]) {
  await cdp.evaluate(`localStorage.setItem('sqlt.theme', ${JSON.stringify(theme)})`);
  await cdp.wait(500);
  for (const [w, h] of [[1366, 768], [1024, 700], [1024, 640]]) {
    await cdp.setViewport(w, h); await cdp.wait(400);
    await cdp.evaluate(`document.querySelector('.cm-content')?.focus()`);
    await cdp.key(" ", "Space", 32, 2);
    await cdp.waitFor(`Boolean(document.querySelector('.cm-tooltip-autocomplete'))`, 6000);
    await cdp.wait(500);
    const file = path.join(outDir, `parity-p3-completion-${theme}-${w}x${h}.png`);
    await cdp.screenshot(file);
    const o = JSON.parse(await cdp.evaluate(`JSON.stringify({ docW: document.documentElement.scrollWidth, winW: innerWidth, docH: document.documentElement.scrollHeight, winH: innerHeight, popup: Boolean(document.querySelector('.cm-tooltip-autocomplete')), clipped: (() => { const r = document.querySelector('.cm-tooltip-autocomplete')?.getBoundingClientRect(); return r ? (r.right > innerWidth + 0.5 || r.bottom > innerHeight + 0.5) : false; })() })`));
    console.log(theme, w + "x" + h, JSON.stringify(o));
    await cdp.key("Escape", "Escape", 27);
    await cdp.wait(200);
  }
  await cdp.clearViewport();
}
await cdp.evaluate(`localStorage.setItem('sqlt.theme','system')`);
cdp.close();
