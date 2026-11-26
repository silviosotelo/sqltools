// F1 checks that the generic flow does not cover. Prints one JSON object and exits 1 if any check fails.
import { connect } from "./cdp.mjs";

const cdp = await connect();
const result = { checks: {} };
const check = (name, ok, detail) => { result.checks[name] = { ok: Boolean(ok), ...(detail === undefined ? {} : { detail }) }; };

// 1) Ln/Col in the status bar follows the cursor (initial document and movement).
await cdp.reload();
await cdp.evaluate(`document.querySelector('.cm-content')?.focus()`);
await cdp.key("a", "KeyA", 65, 2);
await cdp.send("Input.insertText", { text: "select 1\nfrom dual\nwhere 1 = 1" });
await cdp.wait(400);
const pos = () => cdp.evaluate(`(document.querySelector('.statusbar .seg:nth-last-child(2)')?.innerText ?? '').trim()`);
const initial = await pos();
await cdp.key("ArrowUp", "ArrowUp", 38);
await cdp.key("Home", "Home", 36);
await cdp.wait(300);
const afterMove = await pos();
check("cursor position at end of typed text", /Ln 3, Col 12/.test(initial), initial);
check("cursor position after Up + Home", /Ln 2, Col 1/.test(afterMove), afterMove);

// 2) At the minimum window size every toolbar control is fully visible (no clipped buttons).
await cdp.setViewport(1024, 640);
await cdp.wait(500);
const toolbar = JSON.parse(await cdp.evaluate(`JSON.stringify((() => {
  const buttons = Array.from(document.querySelectorAll('.actionstrip button, .titlebar button, .titlebar select, .activity button'));
  const w = window.innerWidth, h = window.innerHeight;
  const clipped = buttons.filter(b => { const r = b.getBoundingClientRect(); return r.width > 0 && (r.right > w + 0.5 || r.bottom > h + 0.5 || r.left < -0.5); })
    .map(b => (b.getAttribute('aria-label') || b.title || b.innerText || b.tagName).slice(0, 30));
  return { total: buttons.length, clipped, scrollW: document.documentElement.scrollWidth, scrollH: document.documentElement.scrollHeight, w, h };
})())`));
check("1024x640: no clipped controls", toolbar.clipped.length === 0, toolbar);
check("1024x640: no page scroll", toolbar.scrollW <= toolbar.w && toolbar.scrollH <= toolbar.h, { scrollW: toolbar.scrollW, scrollH: toolbar.scrollH });
await cdp.clearViewport();

// 3) The editor reads the same tokens as the rest of the app, in both themes.
for (const theme of ["light", "dark"]) {
  await cdp.evaluate(`localStorage.setItem('sqlt.theme', ${JSON.stringify(theme)})`);
  await cdp.reload();
  const colors = JSON.parse(await cdp.evaluate(`JSON.stringify((() => {
    const root = getComputedStyle(document.documentElement);
    const probe = (v) => { const e = document.createElement('i'); e.style.color = 'var(' + v + ')'; document.body.appendChild(e); const c = getComputedStyle(e).color; e.remove(); return c; };
    const bg = (sel) => getComputedStyle(document.querySelector(sel)).backgroundColor;
    const text = (sel) => getComputedStyle(document.querySelector(sel)).color;
    return { theme: document.documentElement.dataset.theme, editorBg: bg('.cm-editor'), surface: (() => { const e = document.createElement('i'); e.style.backgroundColor = 'var(--surface)'; document.body.appendChild(e); const c = getComputedStyle(e).backgroundColor; e.remove(); return c; })(), editorText: text('.cm-content'), text: probe('--text'), font: getComputedStyle(document.querySelector('.cm-content')).fontFamily };
  })())`));
  check(`${theme}: editor background is --surface`, colors.editorBg === colors.surface, colors);
  check(`${theme}: editor text is --text`, colors.editorText === colors.text);
  check(`${theme}: editor uses the mono font token`, /JetBrains Mono/.test(colors.font), colors.font);
}

// 3b) Focus must land somewhere meaningful when the side view changes (review finding).
await cdp.reload();
await cdp.wait(1200);
check("startup view is Objects when saved connections exist", (await cdp.evaluate(`Array.from(document.querySelectorAll('.activity .act[aria-pressed="true"]')).map(b => b.getAttribute('aria-label')).join(',')`)) === "Objetos");
const focusInfo = () => cdp.evaluate(`JSON.stringify((() => { const a = document.activeElement; return { tag: a?.tagName, title: a?.title ?? '', label: a?.getAttribute('aria-label') ?? '', inActivity: Boolean(a?.closest('.activity')), pressed: a?.getAttribute('aria-pressed') ?? null }; })())`);
const pressedView = () => cdp.evaluate(`Array.from(document.querySelectorAll('.activity .act[aria-pressed="true"]')).map(b => b.getAttribute('aria-label')).join(',')`);
// a) With Connections open, View.ObjectTreeSetFocus (Alt+Shift+1) switches to Objects and focuses its filter.
await cdp.click('.activity .act[aria-label="Conexiones"]');
await cdp.wait(400);
check("opening Connections marks only that view as pressed", (await pressedView()) === "Conexiones", await pressedView());
await cdp.evaluate(`document.querySelector('.cm-content')?.focus()`);
await cdp.key("!", "Digit1", 49, 1 | 8);
await cdp.wait(600);
const afterA = JSON.parse(await focusInfo());
check("Alt+Shift+1 from Connections focuses the Objects filter", afterA.title === "Filtro" && (await pressedView()) === "Objetos", afterA);
// b) Focus inside the Connections form, then an accelerator that closes that view: focus must not fall to <body>.
await cdp.click('.activity .act[aria-label="Conexiones"]');
await cdp.wait(400);
await cdp.evaluate(`document.querySelector('.objects-panel input')?.focus()`);
await cdp.key("1", "Digit1", 49, 1);   // Alt+1 = View.ObjectTree
await cdp.wait(900);
const afterB = JSON.parse(await focusInfo());
check("focus is not lost when the focused Connections control is unmounted", afterB.tag !== "BODY" && afterB.inActivity, afterB);
// c) Connections is a plain section now: no collapsible summary that could diverge from the active view.
await cdp.click('.activity .act[aria-label="Conexiones"]');
await cdp.wait(400);
const structure = JSON.parse(await cdp.evaluate(`JSON.stringify({ heading: Boolean(document.querySelector('#connections-title')), summaries: Array.from(document.querySelectorAll('.objects-panel summary')).map(x => x.innerText.trim()) })`));
check("Connections has a heading and no collapsible summary", structure.heading && !structure.summaries.includes("Conexiones"), structure);
// d) Only the two view toggles expose aria-pressed; History and Stats are plain actions.
const pressedAttrs = JSON.parse(await cdp.evaluate(`JSON.stringify(Array.from(document.querySelectorAll('.activity .act')).map(b => [b.getAttribute('aria-label'), b.hasAttribute('aria-pressed')]))`));
check("the four views are toggles; shortcuts and settings are plain actions", pressedAttrs.filter(([l]) => /Objetos|Conexiones|Historial|Estad/.test(l)).every(([, has]) => has) && pressedAttrs.filter(([l]) => /Atajos|Ajustes/.test(l)).every(([, has]) => !has), pressedAttrs);
// e) The status bar announces only its message, not the cursor position.
const live = JSON.parse(await cdp.evaluate(`JSON.stringify({ roleStatus: document.querySelectorAll('.statusbar [role="status"]').length, statusContainsCursor: /Ln \\d/.test(document.querySelector('.statusbar [role="status"]')?.innerText ?? '') })`));
check("only the status message is a live region", live.roleStatus === 1 && !live.statusContainsCursor, live);
await cdp.click('.activity .act[aria-label="Objetos"]');

// 3c) The editor shows a focus ring when it receives focus.
await cdp.evaluate(`document.querySelector('.cm-content')?.focus()`);
await cdp.wait(300);
const ring = JSON.parse(await cdp.evaluate(`JSON.stringify((() => { const e = document.querySelector('.cm-editor'); const cs = getComputedStyle(e); return { focused: e.classList.contains('cm-focused'), outlineStyle: cs.outlineStyle, outlineWidth: cs.outlineWidth }; })())`));
check("the editor shows a focus ring", ring.focused && ring.outlineStyle !== "none" && parseFloat(ring.outlineWidth) >= 2, ring);

// 4) The UI font is IBM Plex Sans (the bundled woff2 loaded, not the system fallback).
const font = await cdp.evaluate(`document.fonts.check('13px "IBM Plex Sans"') && document.fonts.check('13px "JetBrains Mono"')`);
check("IBM Plex Sans and JetBrains Mono are loaded", font);

cdp.close();
const failed = Object.entries(result.checks).filter(([, v]) => !v.ok);
console.log(JSON.stringify(result, null, 2));
console.log(failed.length === 0 ? `\nF1 CHECKS OK (${Object.keys(result.checks).length})` : `\nF1 CHECKS FAILED: ${failed.map(([k]) => k).join("; ")}`);
process.exit(failed.length === 0 ? 0 : 1);
