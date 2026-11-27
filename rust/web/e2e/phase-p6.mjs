// P6 checks on the installed app: start a debug session on a procedure, step, watch a variable, stop.
// The unit is compiled with debug information by the test itself and dropped at the end.
import { connect } from "./cdp.mjs";

const cdp = await connect();
const checks = {};
const check = (n, ok, d) => { checks[n] = { ok: Boolean(ok), ...(d === undefined ? {} : { d }) }; };
const J = async (expr) => JSON.parse(await cdp.evaluate(`JSON.stringify(${expr})`));
const status = () => cdp.evaluate(`document.querySelector("[data-e2e='status-message']")?.innerText.trim() ?? ""`);
const setEditor = async (text) => {
  await cdp.evaluate(`document.querySelector('.cm-content')?.focus()`);
  await cdp.key("a", "KeyA", 65, 2);
  await cdp.send("Input.insertText", { text });
  await cdp.wait(200);
};
const runSql = async (sql) => {
  const before = await status();
  await setEditor(sql);
  await cdp.key("F5", "F5", 116);
  for (let i = 0; i < 80; i++) {
    const t = await status();
    if (t !== before && /\d+ filas?|ORA-|error/i.test(t)) return t;
    await cdp.wait(250);
  }
  return await status();
};

await cdp.reload();
await cdp.wait(1200);
await cdp.clickTitle("Conectar");
await cdp.waitFor(`/(Conectado|anamnesis@)/.test(document.querySelector("[data-e2e='conn-chip']")?.innerText ?? '')`, 20000);
await cdp.wait(1500);

// debug info is a compile-time property: enable it and compile the unit in this session
await runSql("begin execute immediate 'drop procedure sqlt_p6_e2e'; exception when others then null; end;");
await runSql("alter session set plsql_debug = true");
await runSql(
  "create or replace procedure sqlt_p6_e2e (p_rows in number) as\n  l number := 0;\nbegin\n  l := l + 1;\n  for i in 1 .. p_rows loop\n    l := l + i;\n  end loop;\n  l := l + 100;\nend;",
);

// start the debug session on a call to the unit, from the overflow menu
await setEditor("begin sqlt_p6_e2e(3); end;");
await cdp.click('.actionstrip button[aria-label="Más acciones"]');
await cdp.wait(400);
const menu = await J(`Array.from(document.querySelectorAll('[role=menuitem]')).map(b => b.innerText.trim())`);
check("the menu offers debugging the statement", menu.includes("Depurar sentencia"), menu);
await cdp.evaluate(`Array.from(document.querySelectorAll('[role=menuitem]')).find(b => b.innerText.trim() === 'Depurar sentencia')?.click()`);
await cdp.waitFor(`Boolean(document.querySelector('[data-e2e="debug-position"]'))`, 30000);
const started = await cdp.evaluate(`document.querySelector('[data-e2e="debug-position"]')?.innerText.trim() ?? ''`);
check("the debug session starts and shows its position", /interpreter starting|interpreter_starting|arranque/i.test(started), started);

// the reliable way in: a breakpoint at the unit's first executable line, then step (verified in the
// prototype). The breakpoint command exists; the UI for setting one is not part of P6.
const breakpoint = JSON.parse(
  await cdp.evaluate(`(async () => {
    const list = await window.__TAURI_INTERNALS__.invoke("list_connections");
    const id = list[0].id;
    try {
      const number = await window.__TAURI_INTERNALS__.invoke("debug_breakpoint", { connectionId: id, owner: "ANAMNESIS", name: "SQLT_P6_E2E", namespace: 1, line: 4 });
      return JSON.stringify({ ok: true, number });
    } catch (error) {
      return JSON.stringify({ ok: false, error: String(error) });
    }
  })()`),
);
check("a breakpoint can be set on the unit", breakpoint.ok === true, breakpoint);

const positionOf = () => cdp.evaluate(`document.querySelector('[data-e2e="debug-position"]')?.innerText.trim() ?? ''`);
const click = (label) => cdp.evaluate(`Array.from(document.querySelectorAll('.debug-strip button')).find(b => b.innerText.trim() === ${JSON.stringify(label)})?.click()`);
let inside = "";
for (let i = 0; i < 3; i++) {
  if ((await cdp.count(".debug-strip")) === 0) break;
  await click("Paso a paso");
  await cdp.wait(2500);
  const position = await positionOf();
  if (/SQLT_P6_E2E:\d+/.test(position)) {
    inside = position;
    break;
  }
}
check("stepping stops at the breakpoint inside the unit", /SQLT_P6_E2E:4/.test(inside), inside);

// a local variable of the suspended frame
const hasInput = (await cdp.count('.debug-strip input[aria-label="Variable"]')) > 0;
if (hasInput) {
  await cdp.evaluate(`(() => { const i = document.querySelector('.debug-strip input[aria-label="Variable"]'); Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(i, 'L'); i.dispatchEvent(new Event('input', { bubbles: true })); i.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true })); })()`);
  await cdp.waitFor(`/L=/.test(document.querySelector('.debug-strip')?.innerText ?? '')`, 15000);
  const watch = await cdp.evaluate(`document.querySelector('.debug-strip .debug-watch')?.innerText.trim() ?? ''`);
  check("a local variable can be watched", /^L=\d+$/.test(watch), watch);
} else {
  check("a local variable can be watched", false, "no watch input");
}

// stop: the strip disappears and no session is left hanging
await cdp.evaluate(`Array.from(document.querySelectorAll('.debug-strip button')).find(b => b.innerText.trim() === 'Detener')?.click()`);
await cdp.waitFor(`!document.querySelector('.debug-strip')`, 15000);
check("stopping removes the debug strip", (await cdp.count(".debug-strip")) === 0);

await runSql("begin execute immediate 'drop procedure sqlt_p6_e2e'; end;");

// 5) step INTO a real callee and OUT back to the caller (not just the breakflag constants)
await runSql("begin execute immediate 'drop procedure sqlt_p6_callee'; exception when others then null; end;");
await runSql("begin execute immediate 'drop procedure sqlt_p6_caller'; exception when others then null; end;");
await runSql("alter session set plsql_debug = true");
await runSql("create or replace procedure sqlt_p6_callee is\nbegin\n  dbms_output.put_line('inside');\nend;");
await runSql("create or replace procedure sqlt_p6_caller is\nbegin\n  sqlt_p6_callee;\n  dbms_output.put_line('after');\nend;");

await setEditor("begin sqlt_p6_caller; end;");
await cdp.click('.actionstrip button[aria-label="Más acciones"]');
await cdp.wait(400);
await cdp.evaluate(`Array.from(document.querySelectorAll('[role=menuitem]')).find(b => b.innerText.trim() === 'Depurar sentencia')?.click()`);
await cdp.waitFor(`Boolean(document.querySelector('[data-e2e="debug-position"]'))`, 30000);
const bpCaller = JSON.parse(
  await cdp.evaluate(`(async () => {
    const list = await window.__TAURI_INTERNALS__.invoke("list_connections");
    try {
      const number = await window.__TAURI_INTERNALS__.invoke("debug_breakpoint", { connectionId: list[0].id, owner: "ANAMNESIS", name: "SQLT_P6_CALLER", namespace: 1, line: 3 });
      return JSON.stringify({ ok: true, number });
    } catch (error) {
      return JSON.stringify({ ok: false, error: String(error) });
    }
  })()`),
);
check("a breakpoint can be set on the caller", bpCaller.ok === true, bpCaller);

const positionOf2 = () => cdp.evaluate(`document.querySelector('[data-e2e="debug-position"]')?.innerText.trim() ?? ''`);
const click2 = (label) => cdp.evaluate(`Array.from(document.querySelectorAll('.debug-strip button')).find(b => b.innerText.trim() === ${JSON.stringify(label)})?.click()`);

await click2("Paso a paso");
await cdp.wait(2500);
const atCall = await positionOf2();
check("stepping stops at the call line of the caller", /SQLT_P6_CALLER:3/.test(atCall), atCall);

await click2("Entrar");
await cdp.wait(3000);
const insideCallee = await positionOf2();
check("step into moves to the callee", /SQLT_P6_CALLEE:\d+/.test(insideCallee), insideCallee);

await click2("Salir");
await cdp.wait(3000);
const backInCaller = await positionOf2();
check("step out returns to the caller after the call", /SQLT_P6_CALLER:(3|4)/.test(backInCaller), backInCaller);

await click2("Detener");
await cdp.wait(1500);
await runSql("begin execute immediate 'drop procedure sqlt_p6_callee'; end;");
await runSql("begin execute immediate 'drop procedure sqlt_p6_caller'; end;");

cdp.close();
console.log(JSON.stringify(checks, null, 1));
const bad = Object.entries(checks).filter(([, v]) => !v.ok).map(([k]) => k);
console.log(bad.length ? "P6 CHECKS FAILED: " + bad.join("; ") : `P6 CHECKS OK (${Object.keys(checks).length})`);
process.exit(bad.length ? 1 : 0);
