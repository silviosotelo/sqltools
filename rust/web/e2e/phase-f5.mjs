// F5 checks on the installed app: bottom tabs, the 21 old toolbar controls, side views, palette, transaction flag.
import { connect } from "./cdp.mjs";
const cdp = await connect();
const checks = {};
const check = (n, ok, d) => { checks[n] = { ok: Boolean(ok), ...(d === undefined ? {} : { d }) }; };
const J = async (expr) => JSON.parse(await cdp.evaluate(`JSON.stringify(${expr})`));
const txnText = () => cdp.evaluate(`document.querySelector("[data-e2e='txn-status']")?.innerText.trim() ?? ""`);
const pressed = () => cdp.evaluate(`Array.from(document.querySelectorAll('.activity .act[aria-pressed="true"]')).map(b => b.getAttribute('aria-label')).join(',')`);
const tabCount = () => cdp.evaluate(`document.querySelectorAll('.editor-tab').length`);
const lineCount = () => cdp.evaluate(`document.querySelectorAll('.cm-line').length`);
const editorText = () => cdp.evaluate(`document.querySelector('.cm-content')?.innerText ?? ''`);
const focusEditor = () => cdp.evaluate(`document.querySelector('.cm-content')?.focus()`);
const setEditor = async (text) => { await focusEditor(); await cdp.key("a", "KeyA", 65, 2); await cdp.send("Input.insertText", { text }); await cdp.wait(200); };

// Runs a palette command: by clicking its option (exact name), or with the keyboard (ArrowDown to it, Enter).
async function palette(name, how = "click") {
  await focusEditor();
  await cdp.key("P", "KeyP", 80, 2 | 8);
  await cdp.waitFor(`Boolean(document.querySelector('[role="combobox"]'))`, 3000);
  await cdp.send("Input.insertText", { text: name });
  await cdp.wait(250);
  if (how === "click") {
    const ok = await cdp.evaluate(`(() => { const o = Array.from(document.querySelectorAll('[role=option]')).find(e => e.querySelector('span').innerText === ${JSON.stringify(name)}); if (!o) return false; o.click(); return true; })()`);
    await cdp.wait(500);
    return ok;
  }
  for (let i = 0; i < 20; i++) {
    const active = await cdp.evaluate(`document.querySelector('[role=option][aria-selected="true"] span')?.innerText ?? ''`);
    if (active === name) break;
    await cdp.key("ArrowDown", "ArrowDown", 40);
  }
  await cdp.key("Enter", "Enter", 13);
  await cdp.waitFor(`!document.querySelector('[role="combobox"]')`, 5000, 50);
  await cdp.wait(150);
  return true;
}

await cdp.reload();
await cdp.wait(1500);

// 1) Bottom tabs: exactly Results, Plan, Object, Output.
const tabs = await J(`Array.from(document.querySelectorAll('.panel-bar .tabs button')).map(b => b.innerText.trim())`);
check("bottom tabs are Resultados, Plan, Objeto, Salida", tabs.join("|") === "Resultados|Plan de ejecución|Objeto|Salida", tabs);

// 2) Every control of the old toolbar has a home. Inventory taken from `git show 7e02fbd^:rust/web/src/App.tsx`:
//    17 titled buttons + the "Conexiones" <select> + the "Solo lectura" checkbox (`connections` appeared twice:
//    the gear button and the select).
const homes = {
  Ejecutar: `Boolean(document.querySelector('.actionstrip button[aria-label^="Ejecutar"]'))`,
  Cancelar: `Boolean(document.querySelector('.actionstrip button[aria-label="Cancelar"]'))`,
  Commit: `Boolean(document.querySelector('.actionstrip button[aria-label="Commit"]'))`,
  Rollback: `Boolean(document.querySelector('.actionstrip button[aria-label="Rollback"]'))`,
  Formatear: `Boolean(document.querySelector('.actionstrip button[aria-label="Formatear"]'))`,
  Plan: `Boolean(document.querySelector('.actionstrip button[aria-label="Plan"]'))`,
  Historial: `Boolean(document.querySelector('.activity .act[aria-label="Historial"]'))`,
  "Estadísticas": `Boolean(document.querySelector('.activity .act[aria-label="Estadísticas"]'))`,
  "Exportar CSV": `Boolean(document.querySelector('.panel-bar [title="Exportar CSV"], .panel-bar [aria-label="Exportar CSV"]'))`,
  Comandos: `Boolean(document.querySelector('.palette-btn'))`,
  Conectar: `Boolean(document.querySelector('.actionstrip button[aria-label="Conectar"], .actionstrip button[aria-label="Desconectar"]'))`,
  Conexiones: `Boolean(document.querySelector('.activity .act[aria-label="Conexiones"]'))`,
  "Solo lectura": `Boolean(document.querySelector('.actionstrip .check-inline input[type="checkbox"]'))`,
};
const lost = [];
for (const [label, expr] of Object.entries(homes)) if (!(await cdp.evaluate(expr))) lost.push(label);
const strip = await J(`Array.from(document.querySelectorAll('.actionstrip button, .actionstrip input')).map(e => e.getAttribute('aria-label') || e.closest('label')?.innerText.trim() || '')`);
check("action strip has run, cancel, format, plan, commit, rollback, read-only, connect, more", [/^Ejecutar/, /^Cancelar$/, /^Formatear$/, /^Plan$/, /^Commit$/, /^Rollback$/, /^Solo lectura$/, /^Conectar$/, /^Más acciones$/].every((re) => strip.some((l) => re.test(l))), strip);
const act = await J(`Array.from(document.querySelectorAll('.activity .act')).map(b => b.getAttribute('aria-label'))`);
check("activity bar has Historial, Estadísticas, Conexiones, Atajos, Ajustes", ["Historial", "Estadísticas", "Conexiones", "Atajos de teclado", "Ajustes de la aplicación"].every((l) => act.includes(l)), act);
await cdp.click('.actionstrip button[aria-label="Más acciones"]');
await cdp.wait(300);
const more = await J(`Array.from(document.querySelectorAll('[role=menuitem]')).map(b => b.innerText.trim())`);
for (const label of ["Guardar workspace", "Abrir workspace", "Actualizar", "Buscar actualizaciones"]) if (!more.includes(label)) lost.push(label);
const focusedInMenu = await cdp.evaluate(`document.activeElement?.getAttribute('role')`);
check("opening the menu focuses its first item", focusedInMenu === "menuitem", focusedInMenu);
await cdp.key("Escape", "Escape", 27);
await cdp.wait(200);
check("Escape closes the menu and returns focus to its button", (await cdp.evaluate(`document.querySelectorAll('[role=menu]').length`)) === 0 && (await cdp.evaluate(`document.activeElement?.getAttribute('aria-label')`)) === "Más acciones");
check("every control of the old toolbar has a home", lost.length === 0, lost);
// Menu keyboard model: items are not tab stops, disabled ones stay arrow-navigable, Tab leaves the menu.
await cdp.click('.actionstrip button[aria-label="Más acciones"]');
await cdp.wait(300);
const menuA11y = await J(`(() => { const items = Array.from(document.querySelectorAll('[role=menuitem]')); return { tabs: items.map(i => i.getAttribute('tabindex')), disabled: items.filter(i => i.getAttribute('aria-disabled') === 'true').map(i => i.innerText.trim()) }; })()`);
check("menu items are not tab stops and disabled ones use aria-disabled", menuA11y.tabs.every((v) => v === "-1") && menuA11y.disabled.length >= 0, menuA11y);
await cdp.key("End", "End", 35);
const lastItem = await cdp.evaluate(`document.activeElement?.getAttribute('role')`);
await cdp.key("Tab", "Tab", 9);
await cdp.wait(250);
check("End reaches the last menu item and Tab closes the menu", lastItem === "menuitem" && (await cdp.count('[role=menu]')) === 0, { lastItem });



const crumbsOff = await J(`Array.from(document.querySelectorAll('.breadcrumbs li')).map(l => l.innerText.trim())`);
check("breadcrumbs without a connection: Sin conexión > Script sin título", crumbsOff.join("|") === "Sin conexión|Script sin título", crumbsOff);

// 3) Palette: combobox pattern, 11 commands from different families, Enter and click paths.
await focusEditor();
await cdp.key("P", "KeyP", 80, 2 | 8);
await cdp.waitFor(`Boolean(document.querySelector('[role="combobox"]'))`, 3000);
const cb = await J(`(() => { const c = document.querySelector('[role=combobox]'); return { expanded: c.getAttribute('aria-expanded'), controls: c.getAttribute('aria-controls'), listbox: Boolean(document.getElementById(c.getAttribute('aria-controls'))), active: c.getAttribute('aria-activedescendant'), activeExists: Boolean(document.getElementById(c.getAttribute('aria-activedescendant') ?? '')), focused: document.activeElement === c }; })()`);
check("combobox: expanded, controls a listbox, active option exists, has focus", cb.expanded === "true" && cb.listbox && cb.activeExists && cb.focused, cb);
await cdp.key("Escape", "Escape", 27);
await cdp.wait(300);
check("Esc closes the palette, empties the query and returns focus to the editor", (await cdp.count('[role=combobox]')) === 0 && (await cdp.evaluate(`document.activeElement?.closest('.cm-editor') !== null`)));

// 2b) The palette owns the keyboard while it is open: Ctrl+A must select its text, not run the
// editor's SelectAll (which would close the palette and act on the document).
await cdp.evaluate(`document.querySelector('.cm-content')?.focus()`);
await cdp.key("P", "KeyP", 80, 2 | 8);
await cdp.waitFor(`Boolean(document.querySelector('[role="combobox"]'))`, 3000);
await cdp.send("Input.insertText", { text: "format" });
await cdp.wait(300);
await cdp.key("a", "KeyA", 65, 2);
await cdp.wait(400);
const stillOpen = (await cdp.count('[role=combobox]')) === 1;
const selectedText = await cdp.evaluate(`(() => { const i = document.querySelector('[role=combobox]'); return i.value.slice(i.selectionStart, i.selectionEnd); })()`);
check("Ctrl+A inside the palette selects its text and does not run an editor command", stillOpen && selectedText === "format", { stillOpen, selectedText });
const noMatch = await cdp.evaluate(`Boolean(document.querySelector('[role=listbox]'))`); // with text there are options
await cdp.send("Input.insertText", { text: "zzzz" });
await cdp.wait(300);
const emptyState = await J(`(() => { const s = document.querySelector('.palette-empty'); return { text: s?.innerText.trim() ?? null, role: s?.getAttribute('role') ?? null, insideListbox: Boolean(s?.closest('[role=listbox]')), listbox: Boolean(document.querySelector('[role=listbox]')) }; })()`);
check("no matches is announced from a status region outside the listbox", emptyState.text === "Sin coincidencias" && emptyState.role === "status" && !emptyState.insideListbox && !emptyState.listbox, { noMatchWithText: noMatch, ...emptyState });
await cdp.key("Escape", "Escape", 27);
await cdp.wait(300);

await setEditor("select a,b from t where x=1");
const before = await editorText();
await palette("Edit.FormatSQL", "keys");
const after = await editorText();
check("Edit.FormatSQL (keyboard: arrows + Enter) reformats the editor", after !== before && after.includes("\n"), { before, after });

const tabs0 = await tabCount();
await palette("File.New");
check("File.New adds a tab", (await tabCount()) === tabs0 + 1, [tabs0, await tabCount()]);
await palette("File.Close");
check("File.Close removes it", (await tabCount()) === tabs0);
await palette("View.History");
check("View.History opens the History side view", (await pressed()) === "Historial", await pressed());
await palette("View.ObjectTree");
check("View.ObjectTree opens the Objects side view", (await pressed()) === "Objetos", await pressed());
await palette("View.Properties");
check("View.Properties selects the Objeto bottom tab", await cdp.evaluate(`document.querySelector('.panel-bar .tabs button.active')?.innerText.trim() === 'Objeto'`));
await palette("View.Toolbar");
check("View.Toolbar hides the action strip", (await cdp.count(".actionstrip")) === 0);
await palette("View.Toolbar");
check("View.Toolbar shows it again", (await cdp.count(".actionstrip")) === 1);
// Running a command from the palette must not leave the focus on <body>.
const focusAfterPalette = async (name) => {
  await palette(name);
  await cdp.wait(250);
  return cdp.evaluate(`(() => { const a = document.activeElement; return (!a || a === document.body) ? 'BODY' : (a.closest('.cm-editor') ? 'editor' : a.tagName); })()`);
};
check("running a view command from the palette keeps the focus out of <body>", (await focusAfterPalette("View.StatusBar")) !== "BODY");
await palette("View.StatusBar");

await palette("View.StatusBar");
const statusHidden = (await cdp.count(".statusbar")) === 0;
await palette("View.StatusBar");
check("View.StatusBar toggles the status bar", statusHidden && (await cdp.count(".statusbar")) === 1);
await setEditor("one\ntwo");
const lines = await lineCount();
await palette("Edit.DupLineOrSelection");
check("Edit.DupLineOrSelection duplicates a line", (await lineCount()) === lines + 1, [lines, await lineCount()]);
await palette("Edit.DeleteLine");
check("Edit.DeleteLine deletes a line", (await lineCount()) === lines, [lines, await lineCount()]);
await palette("Edit.Find");
check("Edit.Find opens the search panel", (await cdp.count(".cm-search")) > 0);
await cdp.key("Escape", "Escape", 27);

// 4) Connected: history, stats and the transaction flag.
await cdp.reload();
await cdp.wait(1200);
await cdp.clickTitle("Conectar");
await cdp.waitFor(`/(Conectado|anamnesis@)/.test(document.querySelector("[data-e2e='conn-chip']")?.innerText ?? '')`, 20000);
await cdp.wait(1500);
check("the toggle reads Desconectar while connected", await cdp.evaluate(`Boolean(document.querySelector('.actionstrip button[aria-label="Desconectar"]'))`));
const crumbsOn = await J(`Array.from(document.querySelectorAll('.breadcrumbs li')).map(l => l.innerText.trim())`);
check("breadcrumbs connected: connection > schema > Script sin título", crumbsOn.length === 3 && crumbsOn[0].length > 0 && crumbsOn[0] !== "Sin conexión" && crumbsOn[2] === "Script sin título", crumbsOn);
check("no transaction flag right after connecting", (await txnText()) === "");
await setEditor("select 7 from dual");
await cdp.key("F5", "F5", 116);
await cdp.waitFor(`document.querySelectorAll('.grid tbody tr').length > 0`, 15000);
await cdp.click('.activity .act[aria-label="Historial"]');
await cdp.wait(500);
const hist = await J(`Array.from(document.querySelectorAll('.history-row')).slice(0, 1).map(r => r.innerText.replace(/\\n/g, ' | '))`);
check("History view lists the executed statement with rows and time", hist.length === 1 && /select 7 from dual/.test(hist[0]) && /1 fila/.test(hist[0]), hist);
await cdp.evaluate(`document.querySelector('.history-row')?.click()`);
await cdp.wait(300);
check("clicking a history row puts the SQL back in the editor", (await editorText()).includes("select 7 from dual"));
await cdp.click('.activity .act[aria-label="Estadísticas"]');
await cdp.wait(800);
const statRows = await cdp.count(".stats-scroll tbody tr");
check("Stats view shows the session statistics", statRows > 0, statRows);
await cdp.click('.activity .act[aria-label="Objetos"]');

// A DDL tab keeps its own provenance: switching schema must not relabel it.
await cdp.click('.activity .act[aria-label="Objetos"]');
await cdp.wait(400);
await cdp.evaluate(`Array.from(document.querySelectorAll('[role=treeitem]')).find(r => /^Tablas/.test(r.innerText.trim()))?.click()`);
await cdp.waitFor(`document.querySelectorAll('[role=treeitem][data-type=TABLE][aria-level="2"]').length > 0`, 20000);
await cdp.evaluate(`(() => { const i = document.querySelector('.objects-panel input[title="Filtro"]'); Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(i, 'A11'); i.dispatchEvent(new Event('input', { bubbles: true })); })()`);
await cdp.wait(400);
await cdp.evaluate(`document.querySelector('[role=treeitem][data-type=TABLE][aria-level="2"]')?.click()`);
await cdp.wait(600);
await cdp.evaluate(`document.querySelector('.cm-content')?.focus()`);
await cdp.key("F12", "F12", 123, 2);
await cdp.waitFor(`Array.from(document.querySelectorAll('.editor-tab .tab-title')).some(t => t.innerText.trim().startsWith("A11 (TABLE)"))`, 60000, 200);
const crumbsDdl = await J(`Array.from(document.querySelectorAll('.breadcrumbs li')).map(l => l.innerText.trim())`);
await cdp.evaluate(`(() => { const i = document.querySelector('.objects-panel input[title="Esquema"]'); Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(i, 'SYS'); i.dispatchEvent(new Event('input', { bubbles: true })); })()`);
await cdp.wait(700);
const crumbsDdlAfter = await J(`Array.from(document.querySelectorAll('.breadcrumbs li')).map(l => l.innerText.trim())`);
check("a DDL tab is labelled with its own type and object", crumbsDdl.join("|") === "TABLE|A11", crumbsDdl);
check("changing the schema field never relabels the open DDL tab", crumbsDdlAfter.join("|") === crumbsDdl.join("|") && !crumbsDdlAfter.includes("SYS"), crumbsDdlAfter);
await cdp.evaluate(`(() => { const i = document.querySelector('.objects-panel input[title="Esquema"]'); Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(i, ''); i.dispatchEvent(new Event('input', { bubbles: true })); })()`);
await cdp.wait(500);

await setEditor("select 1 from dual for update");
await cdp.key("F5", "F5", 116);
await cdp.waitFor(`Boolean(document.querySelector("[data-e2e='txn-status']")?.innerText.trim())`, 15000);
check("an open transaction is shown in the status bar", (await txnText()) === "Transacción abierta");
check("Commit and Rollback are marked while it is open", (await cdp.count(".actionstrip .tb.marked")) >= 2);
await cdp.clickTitle("Rollback");
await cdp.waitFor(`!document.querySelector("[data-e2e='txn-status']")?.innerText.trim()`, 15000);
check("Rollback clears the flag", (await txnText()) === "");
// Reconnecting opens a new session: the old transaction (and its flag) is gone even if the user does not
// commit or roll back first.
await setEditor("select 3 from dual for update");
await cdp.key("F5", "F5", 116);
await cdp.waitFor(`Boolean(document.querySelector("[data-e2e='txn-status']")?.innerText.trim())`, 15000);
await palette("Session.Reconnect");
await cdp.waitFor(`!document.querySelector("[data-e2e='txn-status']")?.innerText.trim()`, 20000);
check("reconnecting clears the flag of the old session", (await txnText()) === "");

await setEditor("select 2 from dual");
await cdp.key("F5", "F5", 116);
await cdp.wait(1500);
check("a plain select leaves no flag", (await txnText()) === "");
cdp.close();
console.log(JSON.stringify(checks, null, 1));
const bad = Object.entries(checks).filter(([, v]) => !v.ok).map(([k]) => k);
console.log(bad.length ? "F5 CHECKS FAILED: " + bad.join("; ") : `F5 CHECKS OK (${Object.keys(checks).length})`);
process.exit(bad.length ? 1 : 0);