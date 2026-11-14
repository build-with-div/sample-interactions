// Pinned Chromium over the DevTools protocol, for the fidelity tools.
//
// `launch(chromePath, { width, height })` starts headless Chrome for
// Testing at device scale factor 1 with an emulated focused window and
// returns { cdp, version, close }. `cdp.evaluate(expression)` awaits
// promises and returns values by value.

import { spawn } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

class Cdp {
  constructor(url) {
    this.socket = new WebSocket(url);
    this.next = 1;
    this.pending = new Map();
    this.waiters = [];
    this.socket.addEventListener("message", ({ data }) => {
      const message = JSON.parse(data);
      if (message.id) {
        const { resolve, reject } = this.pending.get(message.id);
        this.pending.delete(message.id);
        if (message.error) reject(new Error(JSON.stringify(message.error)));
        else resolve(message.result);
      } else {
        this.waiters = this.waiters.filter((waiter) => !waiter(message));
      }
    });
  }

  open() {
    return new Promise((done) => this.socket.addEventListener("open", done, { once: true }));
  }

  send(method, params = {}) {
    const id = this.next++;
    this.socket.send(JSON.stringify({ id, method, params }));
    return new Promise((resolve, reject) => this.pending.set(id, { resolve, reject }));
  }

  once(method) {
    return new Promise((done) =>
      this.waiters.push((message) => {
        if (message.method !== method) return false;
        done(message.params);
        return true;
      }),
    );
  }

  async evaluate(expression) {
    const { result, exceptionDetails } = await this.send("Runtime.evaluate", {
      expression,
      awaitPromise: true,
      returnByValue: true,
    });
    if (exceptionDetails) throw new Error(JSON.stringify(exceptionDetails));
    return result.value;
  }

  // Load `url` as a new document and wait for its load event.
  async load(url) {
    const loaded = this.once("Page.loadEventFired");
    await this.send("Page.navigate", { url });
    await loaded;
  }
}

export async function launch(chromePath, { width, height }) {
  const profile = mkdtempSync(join(tmpdir(), "div-fidelity-"));
  const chrome = spawn(
    chromePath,
    [
      "--headless=new",
      "--disable-gpu",
      "--force-device-scale-factor=1",
      "--hide-scrollbars",
      "--no-first-run",
      "--no-default-browser-check",
      "--disable-extensions",
      `--window-size=${width},${height}`,
      "--remote-debugging-port=0",
      `--user-data-dir=${profile}`,
      "about:blank",
    ],
    { stdio: "ignore" },
  );
  const exited = new Promise((done) => chrome.once("exit", done));
  const close = async () => {
    // Only the process this launch started; its helpers can hold the
    // profile a little longer, so a profile that won't go is left in the
    // temp folder rather than failing the run.
    if (chrome.exitCode === null) chrome.kill();
    await Promise.race([exited, new Promise((done) => setTimeout(done, 5000))]);
    try {
      rmSync(profile, { recursive: true, force: true, maxRetries: 10, retryDelay: 300 });
    } catch (error) {
      console.warn(`left ${profile}: ${error.code}`);
    }
  };
  try {
    const file = join(profile, "DevToolsActivePort");
    let port;
    for (let attempt = 0; attempt < 200 && !port; attempt++) {
      if (existsSync(file)) port = Number(readFileSync(file, "utf8").split("\n")[0]) || undefined;
      if (!port) await new Promise((done) => setTimeout(done, 50));
    }
    if (!port) throw new Error("Chromium did not start");
    const version = (await (await fetch(`http://127.0.0.1:${port}/json/version`)).json()).Browser;
    const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
    const cdp = new Cdp(targets.find((target) => target.type === "page").webSocketDebuggerUrl);
    await cdp.open();
    await cdp.send("Page.enable");
    // A headless page has no window focus until input arrives, so focus()
    // would set activeElement without firing `focus`. Emulate a focused
    // window, as an app window in use is (and as Div's test app is).
    await cdp.send("Emulation.setFocusEmulationEnabled", { enabled: true });
    await cdp.send("Emulation.setDeviceMetricsOverride", {
      width,
      height,
      deviceScaleFactor: 1,
      mobile: false,
    });
    return {
      cdp,
      version,
      close: async () => {
        await cdp.send("Browser.close").catch(() => {});
        await close();
      },
    };
  } catch (error) {
    await close();
    throw error;
  }
}
