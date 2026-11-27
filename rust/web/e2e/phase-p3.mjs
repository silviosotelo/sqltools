// P3 checks on the installed app: dictionary completion (columns by alias, columns of the tables in
// scope) and the object list the completion feeds on.
import { connect } from "./cdp.mjs";

const cdp = await connect();
const checks = {};
const check = (n, ok, d) => { checks[n] = { ok: Boolean(ok), ...(d === undefined ? {} : { d }) }; };
const J = async (expr) => JSON.parse(await cdp.evaluate(`JSON.stringify(${expr})`));
const setEditor = async (text) => {
  await cdp.evaluate(`document.querySelector('.cm-content')?.focus()`);
  await cdp.key("a", "KeyA", 65, 2);
  await cdp.send("Input.insertText", { text });
  await cdp.wait(200);
};
const openPopup = async () => {
  await cdp.evaluate(`document.querySelector('.cm-content')?.focus()`);
  await cdp.key(" ", "Space", 32, 2);
  await cdp.waitFor(`Boolean(document.querySelector('.cm-tooltip-autocomplete'))`, 8000);
  return J(`Array.from(document.querySelectorAll('.cm-tooltip-autocomplete li')).map(li => li.innerText.replace(/\\s+/g, ' ').trim())`);
};
const closePopup = async () => {
  await cdp.key("Escape", "Escape", 27);
  await cdp.wait(200);
};

await cdp.reload();
await cdp.wait(1200);
await cdp.clickTitle("Conectar");
await cdp.waitFor(`/(Conectado|anamnesis@)/.test(document.querySelector("[data-e2e='conn-chip']")?.innerText ?? '')`, 20000);
await cdp.wait(2500);

// 1) the command behind the completion returns every object type the tree shows
const connection = await cdp.evaluate(`document.querySelector("[data-e2e='conn-chip']")?.innerText ?? ''`);
const probe = JSON.parse(
  await cdp.evaluate(`(async () => {
    const list = await window.__TAURI_INTERNALS__.invoke("list_connections");
    const suggestions = await window.__TAURI_INTERNALS__.invoke("suggest_objects", { connectionId: list[0].id, schema: null });
    return JSON.stringify({ suggestions });
  })()`),
);
const suggestions = probe.suggestions ?? [];
const types = [...new Set(suggestions.map((o) => o.objectType))].sort();
check(
  "the object list is loaded for the owner (thousands of objects, several types)",
  suggestions.length > 1000 && types.length >= 6,
  { count: suggestions.length, types, chip: connection.replace(/\\s+/g, " ") },
);

// 2) `alias.` offers that table's columns
await setEditor("select * from a11 t where t.");
const byAlias = await openPopup();
check(
  "alias. offers the columns of the aliased table",
  byAlias.some((option) => /TIPO_CTO/.test(option)),
  byAlias.slice(0, 6),
);
await closePopup();

// 3) unqualified completion offers the columns of the table already named
await setEditor("select * from a11 where ");
const unqualified = await openPopup();
check(
  "unqualified completion offers the columns of the table in scope",
  unqualified.some((option) => /TIPO_CTO/.test(option)),
  unqualified.slice(0, 6),
);
await closePopup();

// 4) an unknown qualifier offers nothing instead of guessing
await setEditor("select * from a11 where zzz.");
await cdp.evaluate(`document.querySelector('.cm-content')?.focus()`);
await cdp.key(" ", "Space", 32, 2);
await cdp.wait(1500);
check(
  "an unknown qualifier does not invent columns",
  (await cdp.count(".cm-tooltip-autocomplete li")) === 0,
);
await closePopup();
cdp.close();
console.log(JSON.stringify(checks, null, 1));
const bad = Object.entries(checks).filter(([, v]) => !v.ok).map(([k]) => k);
console.log(bad.length ? "P3 CHECKS FAILED: " + bad.join("; ") : `P3 CHECKS OK (${Object.keys(checks).length})`);
process.exit(bad.length ? 1 : 0);
