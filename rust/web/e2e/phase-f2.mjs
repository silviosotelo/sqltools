// F2: switching side views must not touch the document, its undo history or the open tabs.
import { connect } from "./cdp.mjs";
const cdp = await connect();
const checks = {};
const check = (n, ok, d) => { checks[n] = { ok: Boolean(ok), ...(d === undefined ? {} : { d }) }; };
await cdp.reload();
await cdp.wait(1200);
const text = () => cdp.evaluate(`document.querySelector('.cm-content')?.innerText ?? ''`);
const view = (name) => cdp.click(`.activity .act[aria-label="${name}"]`);
await cdp.evaluate(`document.querySelector('.cm-content')?.focus()`);
await cdp.key("a", "KeyA", 65, 2);
await cdp.send("Input.insertText", { text: "select 42 from dual" });
await cdp.wait(300);
const typed = await text();
for (const name of ["Conexiones", "Objetos", "Conexiones", "Objetos"]) { await view(name); await cdp.wait(250); }
check("editor text survives view switches", (await text()) === typed, await text());
check("still one tab", (await cdp.count(".editor-tab")) === 1);
await cdp.evaluate(`document.querySelector('.cm-content')?.focus()`);
await cdp.key("z", "KeyZ", 90, 2);
await cdp.wait(300);
check("undo history survives (Ctrl+Z changes the text)", (await text()) !== typed, await text());
cdp.close();
console.log(JSON.stringify(checks, null, 1));
const bad = Object.entries(checks).filter(([, v]) => !v.ok).map(([k]) => k);
console.log(bad.length ? "F2 CHECKS FAILED: " + bad.join("; ") : `F2 CHECKS OK (${Object.keys(checks).length})`);
process.exit(bad.length ? 1 : 0);