// P10 screenshots: the assistant dialog with an answer from the local mock.
import http from "node:http";
import path from "node:path";
import { connect } from "./cdp.mjs";
const outDir = "C:/Users/sotelos/sqltools/docs/screenshots";
const server = http.createServer((request, response) => {
  request.resume();
  request.on("end", () => {
    response.writeHead(200, { "Content-Type": "application/json" });
    response.end(JSON.stringify({ choices: [{ message: { content: "-- Devuelve los clientes activos\nselect * from clientes where estado = 'A';" } }] }));
  });
});
await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
const baseUrl = `http://127.0.0.1:${server.address().port}/v1`;
const cdp = await connect();
await cdp.reload(); await cdp.wait(1400);
await cdp.clickTitle("Conectar");
await cdp.waitFor(`/(Conectado|anamnesis@)/.test(document.querySelector("[data-e2e='conn-chip']")?.innerText ?? '')`, 20000);
await cdp.wait(1500);
for (const theme of ["light", "dark"]) {
  await cdp.evaluate(`localStorage.setItem('sqlt.theme', ${JSON.stringify(theme)})`);
  await cdp.wait(500);
  await cdp.click('.actionstrip button[aria-label="Más acciones"]');
  await cdp.wait(300);
  await cdp.evaluate(`Array.from(document.querySelectorAll('[role=menuitem]')).find(b => b.innerText.trim() === 'Asistente AI…')?.click()`);
  await cdp.waitFor(`Boolean(document.querySelector('.assistant [data-e2e="assistant-answer"]'))`, 10000);
  await cdp.evaluate(`document.querySelector('.assistant details summary')?.click()`);
  await cdp.wait(300);
  const set = (label, value) => cdp.evaluate(`(() => { const i = document.querySelector('.assistant input[aria-label=${JSON.stringify(label)}]'); Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(i, ${JSON.stringify(value)}); i.dispatchEvent(new Event('input', { bubbles: true })); })()`);
  await set("URL base", baseUrl);
  await set("Modelo", "mock-model");
  await set("Clave API", "demo");
  await cdp.evaluate(`Array.from(document.querySelectorAll('.assistant .row button')).find(b => b.innerText.trim() === 'Guardar ajustes')?.click()`);
  await cdp.wait(1200);
  await cdp.evaluate(`(() => { const t = document.querySelector('.assistant textarea'); Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set.call(t, "select * from clientes where estado = 'A'"); t.dispatchEvent(new Event('input', { bubbles: true })); })()`);
  await cdp.evaluate(`Array.from(document.querySelectorAll('.assistant .row button')).find(b => b.innerText.trim() === 'Ejecutar')?.click()`);
  await cdp.waitFor(`/clientes/.test(document.querySelector('[data-e2e="assistant-answer"]')?.innerText ?? '')`, 30000);
  for (const [w, h] of [[1366, 768], [1024, 700], [1024, 640]]) {
    await cdp.setViewport(w, h); await cdp.wait(400);
    const file = path.join(outDir, `parity-p10-assistant-${theme}-${w}x${h}.png`);
    await cdp.screenshot(file);
    const o = JSON.parse(await cdp.evaluate(`JSON.stringify({ docW: document.documentElement.scrollWidth, winW: innerWidth, docH: document.documentElement.scrollHeight, winH: innerHeight, answer: (document.querySelector('[data-e2e="assistant-answer"]')?.innerText ?? '').length })`));
    console.log(theme, w + "x" + h, JSON.stringify(o));
  }
  await cdp.clearViewport();
  await cdp.evaluate(`document.querySelector('.palette-backdrop')?.click()`);
  await cdp.wait(300);
}
await cdp.evaluate(`window.__TAURI_INTERNALS__.invoke("ai_set_key", { key: null })`).catch(() => {});
await cdp.evaluate(`window.__TAURI_INTERNALS__.invoke("ai_set_config", { config: { baseUrl: "https://api.openai.com/v1", model: "gpt-4o-mini", timeoutSecs: 60 } })`).catch(() => {});
await cdp.evaluate(`localStorage.setItem('sqlt.theme','system')`);
cdp.close();
server.close();
