// Minimal Chrome DevTools Protocol client for the Tauri webview (WebView2).
// Launch the app with WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222.
// Uses only Node 22 built-ins (fetch, WebSocket).
import fs from "node:fs";

export async function connect(port = 9222) {
  const list = await fetch(`http://127.0.0.1:${port}/json/list`).then((r) => r.json());
  const page = list.find((t) => t.type === "page");
  if (!page) throw new Error("No page target; is the app running with the remote debugging port?");
  const ws = new WebSocket(page.webSocketDebuggerUrl);
  await new Promise((resolve, reject) => {
    ws.onopen = resolve;
    ws.onerror = () => reject(new Error("WebSocket error"));
  });
  let id = 0;
  const pending = new Map();
  ws.onmessage = (event) => {
    const message = JSON.parse(event.data);
    if (message.id && pending.has(message.id)) {
      pending.get(message.id)(message);
      pending.delete(message.id);
    }
  };
  const send = (method, params = {}) =>
    new Promise((resolve) => {
      const current = ++id;
      pending.set(current, resolve);
      ws.send(JSON.stringify({ id: current, method, params }));
    });

  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

  async function evaluate(expression) {
    const out = await send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true });
    if (out.result?.exceptionDetails) {
      throw new Error("evaluate failed: " + JSON.stringify(out.result.exceptionDetails).slice(0, 300));
    }
    return out.result?.result?.value;
  }

  async function waitFor(fn, ms = 10000, step = 150) {
    const deadline = Date.now() + ms;
    for (;;) {
      const value = await evaluate(typeof fn === "string" ? fn : `(${fn})()`);
      if (value) return value;
      if (Date.now() > deadline) return null;
      await wait(step);
    }
  }

  // modifiers: 1 = Alt, 2 = Ctrl, 4 = Meta, 8 = Shift
  async function key(k, code, vk, modifiers = 0) {
    for (const type of ["keyDown", "keyUp"]) {
      await send("Input.dispatchKeyEvent", { type, modifiers, key: k, code, windowsVirtualKeyCode: vk });
    }
  }

  const q = JSON.stringify;
  const clickTitle = (title) =>
    evaluate(`(() => { const b = Array.from(document.querySelectorAll('button')).find(x => (x.title || '') === ${q(title)}); if (!b) return false; b.click(); return true; })()`);
  const clickText = (text) =>
    evaluate(`(() => { const b = Array.from(document.querySelectorAll('button')).find(x => (x.innerText || '').trim() === ${q(text)}); if (!b) return false; b.click(); return true; })()`);
  const click = (selector) =>
    evaluate(`(() => { const e = document.querySelector(${q(selector)}); if (!e) return false; e.click(); return true; })()`);
  const text = (selector) =>
    evaluate(`(document.querySelector(${q(selector)})?.innerText ?? '').trim()`);
  const count = (selector) => evaluate(`document.querySelectorAll(${q(selector)}).length`);

  async function screenshot(file) {
    const out = await send("Page.captureScreenshot", { format: "png" });
    if (!out.result?.data) throw new Error("screenshot failed");
    fs.writeFileSync(file, Buffer.from(out.result.data, "base64"));
    return file;
  }

  async function setViewport(width, height) {
    await send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: 1, mobile: false });
  }
  const clearViewport = () => send("Emulation.clearDeviceMetricsOverride");

  async function reload() {
    await send("Page.reload");
    await wait(1500);
    await waitFor(`Boolean(document.querySelector('.app'))`, 15000);
    await wait(500);
  }

  return { send, evaluate, waitFor, wait, key, clickTitle, clickText, click, text, count, screenshot, setViewport, clearViewport, reload, close: () => ws.close() };
}
