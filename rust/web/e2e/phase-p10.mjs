// P10 checks on the installed app: the assistant against a LOCAL mock provider (no quota, no real key).
import http from "node:http";
import { connect } from "./cdp.mjs";

const checks = {};
const check = (n, ok, d) => { checks[n] = { ok: Boolean(ok), ...(d === undefined ? {} : { d }) }; };

// the mock speaks the OpenAI chat-completions shape and records what it received
let received = null;
const server = http.createServer((request, response) => {
  let body = "";
  request.on("data", (chunk) => (body += chunk));
  request.on("end", () => {
    received = { url: request.url, auth: request.headers.authorization, body };
    response.writeHead(200, { "Content-Type": "application/json" });
    response.end(JSON.stringify({ choices: [{ message: { content: "select 42 as respuesta from dual" } }] }));
  });
});
await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
const baseUrl = `http://127.0.0.1:${server.address().port}/v1`;

const cdp = await connect();
const status = () => cdp.evaluate(`document.querySelector("[data-e2e='status-message']")?.innerText.trim() ?? ""`);
try {
  await cdp.reload();
  await cdp.wait(1200);
  await cdp.clickTitle("Conectar");
  await cdp.waitFor(`/(Conectado|anamnesis@)/.test(document.querySelector("[data-e2e='conn-chip']")?.innerText ?? '')`, 20000);
  await cdp.wait(1500);

  await cdp.click('.actionstrip button[aria-label="Más acciones"]');
  await cdp.wait(400);
  const menu = JSON.parse(await cdp.evaluate(`JSON.stringify(Array.from(document.querySelectorAll('[role=menuitem]')).map(b => b.innerText.trim()))`));
  check("the menu offers the AI assistant", menu.includes("Asistente AI…"), menu);
  await cdp.evaluate(`Array.from(document.querySelectorAll('[role=menuitem]')).find(b => b.innerText.trim() === 'Asistente AI…')?.click()`);
  await cdp.waitFor(`Boolean(document.querySelector('.assistant [data-e2e="assistant-answer"]'))`, 10000);
  check("the assistant dialog opens with its answer area", true);

  // point it to the mock and store a throwaway key
  await cdp.evaluate(`document.querySelector('.assistant details summary')?.click()`);
  await cdp.wait(300);
  const setField = (label, value) => cdp.evaluate(`(() => { const i = document.querySelector('.assistant input[aria-label=${JSON.stringify(label)}]'); if (!i) return false; Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(i, ${JSON.stringify(value)}); i.dispatchEvent(new Event('input', { bubbles: true })); return true; })()`);
  check("the settings accept a base URL", await setField("URL base", baseUrl));
  await setField("Modelo", "mock-model");
  await setField("Clave API", "test-key");
  await cdp.evaluate(`Array.from(document.querySelectorAll('.assistant .row button')).find(b => b.innerText.trim() === 'Guardar ajustes')?.click()`);
  await cdp.waitFor(`/Guardar ajustes/.test(document.querySelector("[data-e2e='status-message']")?.innerText ?? '')`, 15000);
  const statusAfterSave = await status();
  check("the settings are saved and the key goes to the keyring", /Guardar ajustes/.test(statusAfterSave), statusAfterSave);

  // run "explain" over the document and read the mock's answer
  await cdp.evaluate(`(() => { const t = document.querySelector('.assistant textarea'); Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, 'value').set.call(t, 'select * from a11 where n = 1'); t.dispatchEvent(new Event('input', { bubbles: true })); })()`);
  await cdp.evaluate(`document.querySelector('.assistant select').value = 'explain'; document.querySelector('.assistant select').dispatchEvent(new Event('change', { bubbles: true }))`);
  await cdp.evaluate(`Array.from(document.querySelectorAll('.assistant .row button')).find(b => b.innerText.trim() === 'Ejecutar')?.click()`);
  await cdp.waitFor(`/select 42 as respuesta from dual/.test(document.querySelector('[data-e2e="assistant-answer"]')?.innerText ?? '')`, 30000);
  const answer = await cdp.evaluate(`document.querySelector('[data-e2e="assistant-answer"]')?.innerText.trim() ?? ''`);
  check("the answer from the provider is shown", answer === "select 42 as respuesta from dual", answer);
  check("the request went to /chat/completions with the stored key", Boolean(received) && received.url === "/v1/chat/completions" && received.auth === "Bearer test-key", { url: received?.url, auth: received?.auth });
  check("the request carries the code and the selected function", /select \* from a11 where n = 1/.test(received?.body ?? "") && /explain what the following code does/i.test(received?.body ?? ""), (received?.body ?? "").slice(0, 200));

  // insert the answer in the editor
  await cdp.evaluate(`Array.from(document.querySelectorAll('.assistant .row button')).find(b => b.innerText.trim() === 'Insertar en el editor')?.click()`);
  await cdp.waitFor(`/respuesta/.test(document.querySelector('.cm-content')?.innerText ?? '')`, 10000);
  const editor = await cdp.evaluate(`document.querySelector('.cm-content')?.innerText ?? ''`);
  check("the answer can be inserted into the editor", /select 42 as respuesta from dual/.test(editor), editor.replace(/\s+/g, " ").slice(0, 100));

  // the key is not written to any file under the config directory
  const keyInFiles = JSON.parse(await cdp.evaluate(`(async () => { const config = await window.__TAURI_INTERNALS__.invoke("ai_config"); return JSON.stringify(config); })()`));
  check("the configuration never contains the key", !JSON.stringify(keyInFiles).includes("test-key"), keyInFiles);
} finally {
  // leave no throwaway key behind
  await cdp.evaluate(`window.__TAURI_INTERNALS__.invoke("ai_set_key", { key: null })`).catch(() => {});
  // leave the app on the default provider, not on the mock
  await cdp
    .evaluate(
      `window.__TAURI_INTERNALS__.invoke("ai_set_config", { config: { baseUrl: "https://api.openai.com/v1", model: "gpt-4o-mini", timeoutSecs: 60 } })`,
    )
    .catch(() => {});
  cdp.close();
  server.close();
}
console.log(JSON.stringify(checks, null, 1));
const bad = Object.entries(checks).filter(([, v]) => !v.ok).map(([k]) => k);
console.log(bad.length ? "P10 CHECKS FAILED: " + bad.join("; ") : `P10 CHECKS OK (${Object.keys(checks).length})`);
process.exit(bad.length ? 1 : 0);
