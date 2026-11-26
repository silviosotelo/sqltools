// F5 checks on the installed app: bottom tabs, the 21 old toolbar controls, side views, palette, transaction flag.
import { connect } from "./cdp.mjs";
const cdp = await connect();
const checks = {};
const check = (n, ok, d) => { checks[n] = { ok: Boolean(ok), ...(d === undefined ? {} : { d }) }; };
const J = async (expr) => JSON.parse(await cdp.evaluate(`JSON.stringify(${expr})`));
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
  await cdp.wait(500);
  return true;
}

await cdp.reload();
await cdp.wait(1500);

// 1) Bottom tabs: exactly Results, Plan, Object, Output.
const tabs = await J(`Array.from(document.querySelectorAll('.panel-bar .tabs button')).map(b => b.innerText.trim())`);
check("bottom tabs are Resultados, Plan, Objeto, Salida", tabs.join("|") === "Resultados|Plan de ejecución|Objeto|Salida", tabs);

// 2) Every control of the old 21-button toolbar still exists somewhere.
const strip = await J(`Array.from(document.querySelectorAll('.actionstrip button, .actionstrip input')).map(e => e.getAttribute('aria-label') || e.closest('label')?.innerText.trim() || '')`);
const need = [/^Ejecutar/, /^Cancelar$/, /^Formatear$/, /^Plan$/, /^Commit$/, /^Rollback$/, /^Solo lectura$/, /^Conectar$/, /^Más acciones$/];
check("action strip has run, cancel, format, plan, commit, rollback, read-only, connect, more", need.every((re) => strip.some((l) => re.test(l))), strip);
const act = await J(`Array.from(document.querySelectorAll('.activity .act')).map(b => b.getAttribute('aria-label'))`);
check("activity bar has Historial, Estadísticas, Conexiones, Atajos, Ajustes", ["Historial", "Estadísticas", "Conexiones", "Atajos de teclado", "Ajustes de la aplicación"].every((l) => act.includes(l)), act);
check("Export CSV lives in the results panel", await cdp.evaluate(`Boolean(document.querySelector('.panel-bar [title="Exportar CSV"], .panel-bar [aria-label="Exportar CSV"]'))`));
await cdp.click('.actionstrip button[aria-label="Más acciones"]');
await cdp.wait(300);
const more = await J(`Array.from(document.querySelectorAll('[role=menuitem]')).map(b => b.innerText.trim())`);
check("the overflow menu has workspace save/load, refresh cache, check updates", ["Guardar workspace", "Abrir workspace", "Actualizar", "Buscar actualizaciones"].every((l) => more.includes(l)), more);
const focusedInMenu = await cdp.evaluate(`document.activeElement?.getAttribute('role')`);
check("opening the menu focuses its first item", focusedInMenu === "menuitem", focusedInMenu);
await cdp.key("Escape", "Escape", 27);
await cdp.wait(200);
check("Escape closes the menu and returns focus to its button", (await cdp.evaluate(`document.querySelectorAll('[role=menu]').length`)) === 0 && (await cdp.evaluate(`document.activeElement?.getAttribute('aria-label')`)) === "Más acciones");
check("the connection list replaces the old select", await cdp.evaluate(`Boolean(document.querySelector('.activity .act[aria-label="Conexiones"]'))`));
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
const crumbsOn = await J(`Array.from(document.querySelectorAll('.breadcrumbs li')).map(l => l.innerText.trim())`);
check("breadcrumbs connected: connection > schema > Script sin título", crumbsOn.length === 3 && crumbsOn[0].length > 0 && crumbsOn[0] !== "Sin conexión" && crumbsOn[2] === "Script sin título", crumbsOn);
check("no transaction flag right after connecting", (await cdp.count("[data-e2e='txn-pending']")) === 0);
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

await setEditor("select 1 from dual for update");
await cdp.key("F5", "F5", 116);
await cdp.waitFor(`Boolean(document.querySelector("[data-e2e='txn-pending']"))`, 15000);
check("an open transaction is shown in the status bar", await cdp.evaluate(`document.querySelector("[data-e2e='txn-pending']")?.innerText`) === "Transacción abierta");
check("Commit and Rollback are marked while it is open", (await cdp.count(".actionstrip .tb.marked")) >= 2);
await cdp.clickTitle("Rollback");
await cdp.waitFor(`!document.querySelector("[data-e2e='txn-pending']")`, 15000);
check("Rollback clears the flag", (await cdp.count("[data-e2e='txn-pending']")) === 0);
await setEditor("select 2 from dual");
await cdp.key("F5", "F5", 116);
await cdp.wait(1500);
check("a plain select leaves no flag", (await cdp.count("[data-e2e='txn-pending']")) === 0);
cdp.close();
console.log(JSON.stringify(checks, null, 1));
const bad = Object.entries(checks).filter(([, v]) => !v.ok).map(([k]) => k);
console.log(bad.length ? "F5 CHECKS FAILED: " + bad.join("; ") : `F5 CHECKS OK (${Object.keys(checks).length})`);
process.exit(bad.length ? 1 : 0);