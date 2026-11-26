// F4: environment badge end to end on the installed app. Edits the saved connection, checks the badge in
// the chip, the list, the objects panel and the status bar, proves the stored password survived the edit
// (connect still works), then restores the original (no environment). Exit 1 on any failed check.
import { connect } from "./cdp.mjs";
const cdp = await connect();
const checks = {};
const check = (n, ok, d) => { checks[n] = { ok: Boolean(ok), ...(d === undefined ? {} : { d }) }; };
const setField = (label, value) => cdp.evaluate(`(() => { const f = Array.from(document.querySelectorAll('.objects-panel .field-row')).find(x => x.querySelector('span')?.innerText.trim() === ${JSON.stringify(label)}); const i = f?.querySelector('input'); if (!i) return false; Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(i, ${JSON.stringify(value)}); i.dispatchEvent(new Event('input', { bubbles: true })); return true; })()`);
const badges = (scope) => cdp.evaluate(`JSON.stringify(Array.from(document.querySelectorAll(${JSON.stringify(scope + " [data-e2e='env-badge']")})).map(b => b.innerText.trim() + ':' + b.dataset.color))`);
const openConnections = async () => { await cdp.click('.activity .act[aria-label="Conexiones"]'); await cdp.wait(300); };
const connectNow = async () => {
  await cdp.clickTitle("Conectar");
  return Boolean(await cdp.waitFor(`/(Conectado|anamnesis@)/.test(document.querySelector("[data-e2e='conn-chip']")?.innerText ?? '')`, 20000));
};

await cdp.reload();
await cdp.wait(1500);
await openConnections();
const listed = JSON.parse(await cdp.evaluate(`JSON.stringify(Array.from(document.querySelectorAll('.conn-item .conn-name')).map(e => e.innerText))`));
check("the connection list shows the saved connections", listed.length >= 1, listed);
const hadEnv = JSON.parse(await badges(".conn-list")).length > 0;
check("an old connection without environment shows no badge", !hadEnv, await badges(".conn-list"));
await cdp.evaluate(`document.querySelector('.conn-item')?.click()`);
await cdp.wait(300);

// Legacy connection (no environment) still connects.
check("legacy connection connects and the chip has no badge", (await connectNow()) && JSON.parse(await badges(".titlebar")).length === 0);
await cdp.clickTitle("Desconectar").catch(() => {});

// Edit: label DESA + cyan, leave the password field EMPTY (the stored password must survive).
await openConnections();
await setField("Etiqueta de entorno", "  DESA  ");
await cdp.evaluate(`document.querySelector('.swatch[data-color="cyan"]')?.click()`);
await cdp.wait(200);
const radios = JSON.parse(await cdp.evaluate(`JSON.stringify(Array.from(document.querySelectorAll('[role=radiogroup] [role=radio]')).map(r => [r.getAttribute('aria-label'), r.getAttribute('aria-checked')]))`));
check("the swatches are a radio group with the cyan one checked", radios.length === 6 && radios.filter(([, c]) => c === "true").map(([l]) => l).join() === "Cian", radios);
// Roving focus: ArrowRight moves the selection (and focus) from cyan to green, then back.
await cdp.evaluate(`document.querySelector('.swatch[aria-checked="true"]')?.focus()`);
await cdp.key("ArrowRight", "ArrowRight", 39);
await cdp.wait(300);
const afterArrow = JSON.parse(await cdp.evaluate(`JSON.stringify({ checked: document.querySelector('.swatch[aria-checked="true"]')?.getAttribute('aria-label'), focused: document.activeElement?.getAttribute('aria-label'), tabbable: document.querySelectorAll('.swatch[tabindex="0"]').length })`));
check("ArrowRight moves the checked swatch and focus (one tab stop)", afterArrow.checked === "Verde" && afterArrow.focused === "Verde" && afterArrow.tabbable === 1, afterArrow);
await cdp.key("ArrowLeft", "ArrowLeft", 37);
await cdp.wait(300);
await cdp.evaluate(`Array.from(document.querySelectorAll('.objects-panel .row button')).find(b => b.innerText.trim() === 'Guardar')?.click()`);
await cdp.wait(1200);
await openConnections();
const inList = JSON.parse(await badges(".conn-list"));
check("badge (trimmed label, cyan) appears in the connection list", inList.length === 1 && inList[0] === "DESA:cyan", inList);

check("connects after the edit (password kept)", await connectNow());
await cdp.wait(1500);
check("badge in the title-bar chip", JSON.parse(await badges(".titlebar")).join() === "DESA:cyan", await badges(".titlebar"));
check("badge in the status bar", JSON.parse(await badges(".statusbar")).join() === "DESA:cyan", await badges(".statusbar"));
await cdp.click('.activity .act[aria-label="Objetos"]');
await cdp.wait(300);
check("badge in the objects panel", JSON.parse(await badges(".objects-panel")).join() === "DESA:cyan", await badges(".objects-panel"));

// Validation: 13 characters is refused with a message.
await openConnections();
await setField("Etiqueta de entorno", "1234567890123");
await cdp.evaluate(`Array.from(document.querySelectorAll('.objects-panel .row button')).find(b => b.innerText.trim() === 'Guardar')?.click()`);
await cdp.wait(600);
const msg = await cdp.evaluate(`document.querySelector("[data-e2e='status-message']")?.innerText ?? ''`);
check("a 13-character label is rejected with a message", /1 y 12/.test(msg), msg);

// Restore: empty label removes the environment.
await setField("Etiqueta de entorno", "");
await cdp.evaluate(`Array.from(document.querySelectorAll('.objects-panel .row button')).find(b => b.innerText.trim() === 'Guardar')?.click()`);
await cdp.wait(1200);
await openConnections();
check("restored: no badge left", JSON.parse(await badges(".conn-list")).length === 0, await badges(".conn-list"));
cdp.close();
console.log(JSON.stringify(checks, null, 1));
const bad = Object.entries(checks).filter(([, v]) => !v.ok).map(([k]) => k);
console.log(bad.length ? "ENV CHECKS FAILED: " + bad.join("; ") : `ENV CHECKS OK (${Object.keys(checks).length})`);
process.exit(bad.length ? 1 : 0);