// Screenshots of the running app for a visual check: node shots.mjs <prefix> [--views a,b]
import fs from "node:fs";
import path from "node:path";
import { connect } from "./cdp.mjs";

const prefix = process.argv[2] ?? "shot";
const outDir = process.env.SHOT_DIR ?? path.join(process.env.TEMP ?? ".", "opencode", "view");
fs.mkdirSync(outDir, { recursive: true });
const stamp = new Date().toISOString().slice(11, 19).replaceAll(":", "");

const cdp = await connect();
const sizes = [[1366, 768], [1024, 700]];
for (const theme of ["light", "dark"]) {
  await cdp.evaluate(`localStorage.setItem('sqlt.theme', ${JSON.stringify(theme)})`);
  await cdp.reload();
  await cdp.clickTitle("Conectar");
  await cdp.waitFor(`/(Conectado|anamnesis@)/.test(document.querySelector("[data-e2e='conn-chip'], .connection-chip")?.innerText ?? '')`, 20000);
  await cdp.wait(2500); // let connect() finish its follow-up requests before loading objects
  await cdp.clickTitle("Cargar objetos");
  const loaded = await cdp.waitFor(`document.querySelectorAll('.objects-panel tbody tr').length > 0`, 30000);
  if (!loaded) throw new Error(`objects did not load in the ${theme} pass`);
  for (const [w, h] of sizes) {
    await cdp.setViewport(w, h);
    await cdp.wait(500);
    const file = path.join(outDir, `${prefix}-${theme}-${w}x${h}-${stamp}.png`);
    await cdp.screenshot(file);
    const overflow = await cdp.evaluate(`JSON.stringify({ docW: document.documentElement.scrollWidth, winW: window.innerWidth, docH: document.documentElement.scrollHeight, winH: window.innerHeight })`);
    console.log(file, overflow);
  }
  await cdp.clearViewport();
}
cdp.close();
