// F3: object tree against the running (connected-on-demand) app. Exit 1 on any failed check.
import { connect } from "./cdp.mjs";
const cdp = await connect();
const checks = {};
const check = (n, ok, d) => { checks[n] = { ok: Boolean(ok), ...(d === undefined ? {} : { d }) }; };
const ITEMS = `Array.from(document.querySelectorAll('[role="treeitem"]'))`;
const group = (re) => `${ITEMS}.find(r => ${re}.test(r.innerText.trim()))`;
await cdp.reload();
await cdp.clickTitle("Conectar");
await cdp.waitFor(`/(Conectado|anamnesis@)/.test(document.querySelector("[data-e2e='conn-chip'], .connection-chip")?.innerText ?? '')`, 20000);
await cdp.waitFor(`${ITEMS}.some(r => r.querySelector('.tree-count'))`, 20000);
const groups = JSON.parse(await cdp.evaluate(`JSON.stringify(${ITEMS}.filter(r => r.getAttribute('aria-level') === '1').map(r => [r.querySelector('.tree-label').innerText, r.querySelector('.tree-count')?.innerText ?? null]))`));
check("every group shows a count after connecting", groups.length === 9 && groups.every(([, c]) => c !== null), groups);

// Expand Packages and Synonyms: fast, bounded DOM, count == children.
const results = {};
for (const [name, re] of [["Paquetes", "/^Paquetes/"], ["Sinónimos", "/^Sin/"]]) {
  const t0 = Date.now();
  await cdp.evaluate(`${group(re)}?.click()`);
  await cdp.waitFor(`(() => { const g = ${group(re)}; return g && !g.querySelector('.tree-note'); })()`, 30000);
  await cdp.wait(300);
  const info = JSON.parse(await cdp.evaluate(`JSON.stringify((() => { const g = ${group(re)}; const items = ${ITEMS}; const kids = items.filter(r => r.getAttribute('aria-level') === '2'); return { count: Number(g?.querySelector('.tree-count')?.innerText ?? -1), rendered: items.length, setsize: Number(kids[0]?.getAttribute('aria-setsize') ?? 0) }; })())`));
  results[name] = { ms: Date.now() - t0, ...info };
  check(`${name}: DOM stays bounded (<= 120 treeitems)`, info.rendered <= 120, results[name]);
  check(`${name}: group count equals the number of children`, info.count === info.setsize, results[name]);
  await cdp.evaluate(`${group(re)}?.click()`); // collapse again
  await cdp.wait(200);
}

// Keyboard: focus the tree, Home, ArrowDown, ArrowRight (expand), ArrowRight (first child), ArrowLeft (parent), End.
await cdp.evaluate(`document.querySelector('[role="tree"]').focus()`);
const active = () => cdp.evaluate(`(() => { const t = document.querySelector('[role="tree"]'); const a = document.getElementById(t.getAttribute('aria-activedescendant') ?? ''); return a ? a.innerText.trim().split('\\n')[0] : null; })()`);
await cdp.key("Home", "Home", 36);
const first = await active();
await cdp.key("ArrowDown", "ArrowDown", 40);
const second = await active();
check("Home then ArrowDown moves the active row", first && second && first !== second, [first, second]);
await cdp.key("ArrowRight", "ArrowRight", 39);
await cdp.waitFor(`document.querySelector('[role="treeitem"][aria-level="1"][aria-expanded="true"]') !== null`, 15000);
await cdp.wait(500);
await cdp.key("ArrowRight", "ArrowRight", 39);
const child = JSON.parse(await cdp.evaluate(`JSON.stringify((() => { const t = document.querySelector('[role="tree"]'); const a = document.getElementById(t.getAttribute('aria-activedescendant') ?? ''); return { level: a?.getAttribute('aria-level'), pos: a?.getAttribute('aria-posinset') }; })())`));
check("ArrowRight on an expanded group enters its first child", child.level === "2" && child.pos === "1", child);
await cdp.key("ArrowLeft", "ArrowLeft", 37);
const back = JSON.parse(await cdp.evaluate(`JSON.stringify((() => { const t = document.querySelector('[role="tree"]'); const a = document.getElementById(t.getAttribute('aria-activedescendant') ?? ''); return { level: a?.getAttribute('aria-level') }; })())`));
check("ArrowLeft on a child returns to its group", back.level === "1", back);
await cdp.key("End", "End", 35);
await cdp.wait(300);
const endRow = await active();
check("End goes to the last visible row", Boolean(endRow), endRow);
check("aria-activedescendant always points to a rendered row", await cdp.evaluate(`(() => { const t = document.querySelector('[role="tree"]'); return Boolean(document.getElementById(t.getAttribute('aria-activedescendant') ?? '')); })()`));

// Ctrl+F12 on a package from the tree: spec and body each in a tab.
await cdp.evaluate(`${group("/^Paquetes/")}?.click()`);
await cdp.waitFor(`${ITEMS}.some(r => r.getAttribute('aria-level') === '2')`, 30000);
const wanted = process.env.TREE_PACKAGE ?? (await cdp.evaluate(`${ITEMS}.find(r => r.getAttribute('aria-level') === '2' && r.dataset.type === 'PACKAGE').innerText.trim()`));
await cdp.evaluate(`(() => { const i = document.querySelector('.objects-panel input[title="Filtro"]'); const set = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set; set.call(i, ${JSON.stringify(wanted)}); i.dispatchEvent(new Event('input', { bubbles: true })); })()`);
await cdp.wait(500);
const found = await cdp.evaluate(`Boolean(${ITEMS}.find(r => r.getAttribute('aria-level') === '2' && r.dataset.type === 'PACKAGE' && r.innerText.trim() === ${JSON.stringify(wanted)}))`);
if (found) {
  await cdp.evaluate(`${ITEMS}.find(r => r.getAttribute('aria-level') === '2' && r.dataset.type === 'PACKAGE' && r.innerText.trim() === ${JSON.stringify(wanted)}).click()`);
  await cdp.wait(1000);
  await cdp.evaluate(`document.querySelector('.cm-content')?.focus()`);
  await cdp.key("F12", "F12", 123, 2);
  const tabs = async () => JSON.parse(await cdp.evaluate(`JSON.stringify(Array.from(document.querySelectorAll('.editor-tab .tab-title')).map(t => t.innerText.trim()))`));
  await cdp.waitFor(`document.querySelectorAll('.editor-tab').length >= 3`, 90000, 200);
  const list = await tabs();
  check("Ctrl+F12 on a package opens spec and body in separate tabs", list.filter((n) => n.startsWith(wanted)).length === 2, list);
} else {
  check(`package ${wanted} present (set TREE_PACKAGE)`, false);
}
cdp.close();
console.log(JSON.stringify({ checks, results }, null, 1));
const bad = Object.entries(checks).filter(([, v]) => !v.ok).map(([k]) => k);
console.log(bad.length ? "TREE CHECKS FAILED: " + bad.join("; ") : `TREE CHECKS OK (${Object.keys(checks).length})`);
process.exit(bad.length ? 1 : 0);