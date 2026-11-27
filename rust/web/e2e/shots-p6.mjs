// P6 screenshots: the debug strip at a breakpoint, three sizes, both themes.
import { connect } from "./cdp.mjs";
import path from "node:path";
const outDir = "C:/Users/sotelos/sqltools/docs/screenshots";
const cdp = await connect();
const status = () => cdp.evaluate(`document.querySelector("[data-e2e='status-message']")?.innerText ?? ""`);
const setEditor = async (t) => {
  await cdp.evaluate(`document.querySelector('.cm-content')?.focus()`);
  await cdp.key("a", "KeyA", 65, 2);
  await cdp.send("Input.insertText", { text: t });
  await cdp.wait(200);
};
const runSql = async (sql) => {
  const before = await status();
  await setEditor(sql);
  await cdp.key("F5", "F5", 116);
  for (let i = 0; i < 80; i++) { const t = await status(); if (t !== before && /\d+ filas?|ORA-/i.test(t)) return; await cdp.wait(250); }
};
await cdp.reload(); await cdp.wait(1400);
await cdp.clickTitle("Conectar");
await cdp.waitFor(`/(Conectado|anamnesis@)/.test(document.querySelector("[data-e2e='conn-chip']")?.innerText ?? '')`, 20000);
await cdp.wait(1500);
await runSql("begin execute immediate 'drop procedure sqlt_p6_shot'; exception when others then null; end;");
await runSql("alter session set plsql_debug = true");
await runSql("create or replace procedure sqlt_p6_shot (p_rows in number) as\n  l number := 0;\nbegin\n  l := l + 1;\n  for i in 1 .. p_rows loop\n    l := l + i;\n  end loop;\n  l := l + 100;\nend;");
await setEditor("begin sqlt_p6_shot(3); end;");
for (const theme of ["light", "dark"]) {
  await cdp.evaluate(`localStorage.setItem('sqlt.theme', ${JSON.stringify(theme)})`);
  await cdp.wait(500);
  await cdp.evaluate(`document.querySelector('.cm-content')?.focus()`);
  await cdp.click('.actionstrip button[aria-label="Más acciones"]');
  await cdp.wait(300);
  await cdp.evaluate(`Array.from(document.querySelectorAll('[role=menuitem]')).find(b => b.innerText.trim() === 'Depurar sentencia')?.click()`);
  await cdp.waitFor(`Boolean(document.querySelector('[data-e2e="debug-position"]'))`, 30000);
  await cdp.evaluate(`(async () => { const list = await window.__TAURI_INTERNALS__.invoke("list_connections"); await window.__TAURI_INTERNALS__.invoke("debug_breakpoint", { connectionId: list[0].id, owner: "ANAMNESIS", name: "SQLT_P6_SHOT", namespace: 1, line: 4 }); })()`);
  await cdp.evaluate(`Array.from(document.querySelectorAll('.debug-strip button')).find(b => b.innerText.trim() === 'Paso a paso')?.click()`);
  await cdp.wait(3000);
  await cdp.evaluate(`(() => { const i = document.querySelector('.debug-strip input[aria-label="Variable"]'); Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(i, 'L'); i.dispatchEvent(new Event('input', { bubbles: true })); i.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true })); })()`);
  await cdp.wait(1500);
  for (const [w, h] of [[1366, 768], [1024, 700], [1024, 640]]) {
    await cdp.setViewport(w, h); await cdp.wait(400);
    const file = path.join(outDir, `parity-p6-debugger-${theme}-${w}x${h}.png`);
    await cdp.screenshot(file);
    const o = JSON.parse(await cdp.evaluate(`JSON.stringify({ docW: document.documentElement.scrollWidth, winW: innerWidth, docH: document.documentElement.scrollHeight, winH: innerHeight, pos: document.querySelector('[data-e2e="debug-position"]')?.innerText.trim() })`));
    console.log(theme, w + "x" + h, JSON.stringify(o));
  }
  await cdp.clearViewport();
  await cdp.evaluate(`Array.from(document.querySelectorAll('.debug-strip button')).find(b => b.innerText.trim() === 'Detener')?.click()`);
  await cdp.wait(1000);
}
await runSql("begin execute immediate 'drop procedure sqlt_p6_shot'; end;");
await cdp.evaluate(`localStorage.setItem('sqlt.theme','system')`);
cdp.close();
