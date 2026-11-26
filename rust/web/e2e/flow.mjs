// Regression flow against the running app: connect, explore, open an object, load its DDL,
// run a query. Prints ONE JSON snapshot of DOM facts.
//   node flow.mjs                      print the snapshot
//   node flow.mjs --save golden.json   save it
//   node flow.mjs --compare golden.json  exit 1 if it differs (ignoring keys listed in --ignore a,b)
import fs from "node:fs";
import { connect } from "./cdp.mjs";

const args = process.argv.slice(2);
const opt = (name) => { const i = args.indexOf(name); return i >= 0 ? args[i + 1] : null; };
const ignore = (opt("--ignore") ?? "").split(",").filter(Boolean);

// Selectors tolerate the old and the new markup; later phases only ADD alternatives here.
const CHIP = "[data-e2e='conn-chip'], .connection-chip";
const OBJECT_ROWS = ".objects-panel tbody tr";
const TREE_ITEMS = "[role='treeitem']";

// Fixed target: a small table whose DDL loads fast. Some tables take ~18 s through the MetaDictionary.
const TARGET = "A11";
const TAB_TITLES = ".editor-tab .tab-title, [data-e2e='editor-tab']";

const cdp = await connect();
const out = { flow: "connect-explore-ddl-run" };

// The golden is taken from a freshly launched app: leftover tabs would change the facts.
const initialTabs = JSON.parse(await cdp.evaluate(`JSON.stringify(Array.from(document.querySelectorAll(${JSON.stringify(TAB_TITLES)})).map(t => t.innerText.trim()))`));
if (initialTabs.length > 1) {
  console.error("Restart the app before running the flow (found tabs: " + initialTabs.join(", ") + ")");
  process.exit(3);
}

await cdp.clickTitle("Conectar");
out.connected = Boolean(await cdp.waitFor(`/(Conectado|anamnesis@)/.test(document.querySelector(${JSON.stringify(CHIP)})?.innerText ?? '')`, 20000));
out.chip = await cdp.text(CHIP);

// Objects: old flat list (button "Cargar objetos") or the new tree (counts at connect).
const tree = await cdp.waitFor(`Boolean(document.querySelector("[role='tree']"))`, 1500);
if (tree) {
  out.explorer = "tree";
  out.objectGroups = await cdp.count(`${TREE_ITEMS}[aria-expanded]`);
  await cdp.waitFor(`document.querySelectorAll('.tree-count').length > 0`, 20000); // counts arrived: the tree is settled
  // Expand the tables group, filter to the target and open it.
  await cdp.evaluate(`Array.from(document.querySelectorAll(${JSON.stringify(TREE_ITEMS)})).find(r => /^Tablas/.test(r.innerText.trim()))?.click()`);
  await cdp.waitFor(`Array.from(document.querySelectorAll(${JSON.stringify(TREE_ITEMS)})).some(r => r.getAttribute('aria-level') === '2')`, 20000);
  await cdp.evaluate(`(() => { const i = document.querySelector('.objects-panel input[title="Filtro"]'); const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set; set.call(i, ${JSON.stringify(TARGET)}); i.dispatchEvent(new Event('input', { bubbles: true })); })()`);
  await cdp.wait(500);
  out.targetFound = await cdp.evaluate(`Array.from(document.querySelectorAll(${JSON.stringify(TREE_ITEMS)})).some(r => r.getAttribute('aria-level') === '2' && r.innerText.trim() === ${JSON.stringify(TARGET)})`);
  await cdp.evaluate(`Array.from(document.querySelectorAll(${JSON.stringify(TREE_ITEMS)})).find(r => r.getAttribute('aria-level') === '2' && r.innerText.trim() === ${JSON.stringify(TARGET)})?.click()`);
  await cdp.wait(1200);
} else {
  out.explorer = "list";
  await cdp.clickTitle("Cargar objetos");
  await cdp.waitFor(`document.querySelectorAll(${JSON.stringify(OBJECT_ROWS)}).length > 0`, 20000);
  out.objectRows = await cdp.count(OBJECT_ROWS);
  const idx = await cdp.evaluate(`Array.from(document.querySelectorAll(${JSON.stringify(OBJECT_ROWS)})).findIndex(r => r.children[0]?.innerText.trim() === ${JSON.stringify(TARGET)})`);
  out.targetFound = idx >= 0;
  await cdp.evaluate(`document.querySelectorAll(${JSON.stringify(OBJECT_ROWS)})[${idx}]?.click()`);
  await cdp.wait(1200);
}

// Load DDL for the selected object with Ctrl+F12.
await cdp.evaluate(`document.querySelector('.cm-content')?.focus()`);
await cdp.key("F12", "F12", 123, 2);
out.ddlLoaded = Boolean(await cdp.waitFor(`Array.from(document.querySelectorAll(${JSON.stringify(TAB_TITLES)})).some(t => t.innerText.trim().startsWith(${JSON.stringify(TARGET + " (")}))`, 60000, 100));
out.ddlStart = (await cdp.evaluate(`(document.querySelector('.cm-content')?.textContent ?? '').replace(/\\s+/g,' ').trim().slice(0, 40)`)) ?? "";
out.tabs = JSON.parse(await cdp.evaluate(`JSON.stringify(Array.from(document.querySelectorAll(${JSON.stringify(TAB_TITLES)})).map(t => t.innerText.trim()))`));

// Back to the scratch tab so the query does not overwrite the DDL tab.
await cdp.evaluate(`(document.querySelector('.editor-tab .tab-btn, [data-e2e="editor-tab"]'))?.click()`);
await cdp.wait(400);

// Run a query and read the grid.
await cdp.evaluate(`document.querySelector('.cm-content')?.focus()`);
await cdp.key("a", "KeyA", 65, 2);
await cdp.send("Input.insertText", { text: "select * from dual" });
await cdp.key("F5", "F5", 116);
await cdp.waitFor(`document.querySelectorAll('.grid tbody tr').length > 0`, 15000);
out.gridRows = await cdp.count(".grid tbody tr");
out.firstRow = await cdp.evaluate(`Array.from(document.querySelector('.grid tbody tr')?.children ?? []).map(c => c.innerText.trim()).join('|')`);
out.status = (await cdp.text("[data-e2e='status-message'], .status-bar span")) ?? "";
cdp.close();

const snapshot = JSON.stringify(out, null, 2);
console.log(snapshot);

const save = opt("--save");
if (save) { fs.writeFileSync(save, snapshot); console.log("saved", save); }
const compare = opt("--compare");
if (compare) {
  const golden = JSON.parse(fs.readFileSync(compare, "utf8"));
  // Volatile or whitespace-only differences do not count: elapsed ms and line breaks in innerText.
  const norm = (v) =>
    typeof v === "string" ? v.replace(/\s+/g, " ").replace(/\d+ ms/g, "N ms").trim() : v;
  const diffs = Object.keys({ ...golden, ...out })
    .filter((k) => !ignore.includes(k) && JSON.stringify(norm(golden[k])) !== JSON.stringify(norm(out[k])))
    .map((k) => `${k}: golden=${JSON.stringify(golden[k])} now=${JSON.stringify(out[k])}`);
  if (diffs.length) { console.error("DIFFERENT:\n  " + diffs.join("\n  ")); process.exit(1); }
  console.log("IDENTICAL to", compare);
}
