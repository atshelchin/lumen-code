// Headless Chrome over the DevTools protocol, without dependencies (Node 22: fetch, WebSocket).
// Used by test/browser-e2e.js and src/tools/social-card.js.
import { spawn } from 'node:child_process';
import { mkdtempSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';

export const defaultChrome = () =>
  process.env.CHROME || (process.platform === 'darwin' ? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome' : 'google-chrome');

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

/**
 * Start Chrome and attach to its page. Chrome picks a free debugging port and writes it to
 * DevToolsActivePort in its profile, so two runs never fight over a port; a cold machine gets
 * up to `timeoutMs` to start. Returns { send(method, params), on(listener), close() }.
 */
export async function openChrome(binary = defaultChrome(), args = [], { timeoutMs = 30000 } = {}) {
  const profile = mkdtempSync(path.join(tmpdir(), 'lumen-chrome-'));
  const proc = spawn(binary, ['--headless=new', '--remote-debugging-port=0', `--user-data-dir=${profile}`, '--no-first-run', ...args, 'about:blank'], { stdio: 'ignore' });
  const deadline = Date.now() + timeoutMs;
  let port = 0;
  let page = null;
  while (!page && Date.now() < deadline) {
    await sleep(100);
    try {
      port = port || Number(readFileSync(path.join(profile, 'DevToolsActivePort'), 'utf8').split('\n')[0]);
      if (port) page = (await (await fetch(`http://127.0.0.1:${port}/json`)).json()).find((p) => p.type === 'page');
    } catch (e) {
      /* not up yet */
    }
  }
  if (!page) {
    proc.kill();
    throw new Error(`Chrome (${binary}) did not start within ${timeoutMs / 1000} s`);
  }
  const ws = new WebSocket(page.webSocketDebuggerUrl);
  await new Promise((resolve, reject) => {
    ws.onopen = resolve;
    ws.onerror = reject;
  });
  let id = 0;
  const waiting = new Map();
  const listeners = [];
  ws.onmessage = (m) => {
    const msg = JSON.parse(m.data);
    if (msg.id && waiting.has(msg.id)) {
      waiting.get(msg.id)(msg.result);
      waiting.delete(msg.id);
    }
    for (const fn of listeners) fn(msg);
  };
  return {
    send: (method, params = {}) =>
      new Promise((resolve) => {
        waiting.set(++id, resolve);
        ws.send(JSON.stringify({ id, method, params }));
      }),
    on: (fn) => listeners.push(fn),
    close: () => proc.kill(),
  };
}
