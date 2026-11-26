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
  const buttons = Array.from(document.querySelectorAll('.topbar button, .titlebar button, .titlebar select, .activity button'));
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

// 4) The UI font is IBM Plex Sans (the bundled woff2 loaded, not the system fallback).
const font = await cdp.evaluate(`document.fonts.check('13px "IBM Plex Sans"') && document.fonts.check('13px "JetBrains Mono"')`);
check("IBM Plex Sans and JetBrains Mono are loaded", font);

cdp.close();
const failed = Object.entries(result.checks).filter(([, v]) => !v.ok);
console.log(JSON.stringify(result, null, 2));
console.log(failed.length === 0 ? `\nF1 CHECKS OK (${Object.keys(result.checks).length})` : `\nF1 CHECKS FAILED: ${failed.map(([k]) => k).join("; ")}`);
process.exit(failed.length === 0 ? 0 : 1);
