// Keyboard walk: press Tab through the app and check that every stop is visible, has a name and shows a
// focus indicator. Prints the sequence of stops. Exit 1 if any stop fails.
//   node keyboard-walk.mjs [--max 90] [--theme light|dark]
import { connect } from "./cdp.mjs";

const args = process.argv.slice(2);
const opt = (name, fallback) => { const i = args.indexOf(name); return i >= 0 ? args[i + 1] : fallback; };
const max = Number(opt("--max", 90));
const theme = opt("--theme", "light");

const cdp = await connect();
await cdp.evaluate(`localStorage.setItem('sqlt.theme', ${JSON.stringify(theme)})`);
await cdp.reload();
await cdp.evaluate(`document.activeElement?.blur()`);

const stops = [];
for (let i = 0; i < max; i++) {
  await cdp.key("Tab", "Tab", 9);
  // CodeMirror applies .cm-focused asynchronously and other styles settle on the next frame: poll briefly
  // instead of trusting one fixed delay (a real missing indicator never appears, so this hides nothing).
  let info = null;
  for (let attempt = 0; attempt < 6; attempt++) {
    await cdp.wait(attempt === 0 ? 60 : 150);
    info = await cdp.evaluate(`JSON.stringify((() => {
    const a = document.activeElement;
    if (!a || a === document.body) return null;
    const cs = getComputedStyle(a);
    const r = a.getBoundingClientRect();
    const own = (cs.outlineStyle !== 'none' && parseFloat(cs.outlineWidth) > 0) || (cs.boxShadow && cs.boxShadow !== 'none');
    const wrap = a.closest('.field, .object-filter, .side-search, .filter, .cm-editor.cm-focused');
    const wrapCs = wrap ? getComputedStyle(wrap) : null;
    const viaWrap = wrapCs ? ((wrapCs.outlineStyle !== 'none' && parseFloat(wrapCs.outlineWidth) > 0) || wrapCs.boxShadow !== 'none') : false;
    const name = a.getAttribute('aria-label') || (a.labels && a.labels[0] && a.labels[0].innerText.trim()) || a.getAttribute('title') || a.getAttribute('placeholder') || (a.innerText || '').trim().slice(0, 30) || a.tagName;
    const area = a.closest('.titlebar') ? 'titlebar' : a.closest('.actionstrip') ? 'toolbar' : a.closest('.activity') ? 'activity' : a.closest('.objects-panel') ? 'side' : a.closest('.editor-shell') ? 'editor' : a.closest('.statusbar') ? 'status' : 'panel';
    return { name: name.replace(/\\s+/g, ' '), tag: a.tagName.toLowerCase(), area, visible: r.width > 0 && r.height > 0, inViewport: r.right <= window.innerWidth + 0.5 && r.bottom <= window.innerHeight + 0.5, indicator: Boolean(own || viaWrap) };
  })())`);
    const parsed = info && JSON.parse(info);
    if (!parsed || parsed.indicator) break;
  }

  const stop = info && JSON.parse(info);
  if (!stop) break;
  if (stops.length && stops.at(-1).name === stop.name && stops.at(-1).area === stop.area && stops.length > 2 && stops[0].name === stop.name) break; // wrapped around
  stops.push(stop);
}
cdp.close();

const bad = stops.filter((s) => !s.visible || !s.inViewport || !s.indicator);
const areas = [...new Set(stops.map((s) => s.area))];
console.log(JSON.stringify({ theme, stops: stops.length, areasInOrder: areas, withoutIndicator: bad.map((s) => `${s.area}:${s.name}`) }, null, 2));
console.log(stops.map((s, i) => `${String(i + 1).padStart(2)} ${s.area.padEnd(9)} ${s.tag.padEnd(8)} ${s.name}${s.indicator ? "" : "   <-- NO FOCUS INDICATOR"}`).join("\n"));
const failed = bad.length > 0 || stops.length < 10;
console.log(failed ? `\nKEYBOARD WALK FAILED (${bad.length} stop(s) without a visible, in-viewport focus indicator; ${stops.length} stops)` : `\nKEYBOARD WALK OK (${stops.length} stops, ${theme})`);
process.exit(failed ? 1 : 0);
