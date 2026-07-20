import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { setTimeout as delay } from "node:timers/promises";

const browser = process.argv[2];
if (!browser) throw new Error("browser executable required");
const root = resolve(import.meta.dirname, "..");
const fixture = resolve(root, "integration-tests/representative/plain-html-audit");
const html = readFileSync(resolve(fixture, "index.html"));
const css = readFileSync(resolve(fixture, "app.css"));
const server = createServer((request, response) => {
  if (request.url === "/app.css") { response.writeHead(200, {"content-type":"text/css"}); response.end(css); return; }
  response.writeHead(200, {"content-type":"text/html"}); response.end(html);
});
await new Promise((resolveListen) => server.listen(0, "127.0.0.1", resolveListen));
const port = server.address().port;
const profile = mkdtempSync(join(tmpdir(), "pliegocss-cdp-"));
const debugPort = 9229 + Math.floor(Math.random() * 500);
const child = spawn(browser, ["--headless=new", `--remote-debugging-port=${debugPort}`, `--user-data-dir=${profile}`, "--no-first-run", "--disable-default-apps", `http://127.0.0.1:${port}/`], {stdio:"ignore"});
try {
  let version;
  for (let attempt = 0; attempt < 50; attempt++) {
    try { version = await (await fetch(`http://127.0.0.1:${debugPort}/json/version`)).json(); break; } catch { await delay(100); }
  }
  if (!version) throw new Error("CDP endpoint unavailable");
  let target;
  for (let attempt = 0; attempt < 50; attempt++) {
    const targets = await (await fetch(`http://127.0.0.1:${debugPort}/json/list`)).json();
    target = targets.find((entry) => entry.type === "page" && entry.url.includes(`127.0.0.1:${port}`));
    if (target) break;
    await delay(100);
  }
  if (!target) throw new Error("fixture target unavailable");
  const ws = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((open, reject) => { ws.addEventListener("open", open, {once:true}); ws.addEventListener("error", reject, {once:true}); });
  let id = 0;
  const call = (method, params = {}) => new Promise((resolveCall, reject) => {
    const current = ++id;
    const listener = (event) => { const message = JSON.parse(event.data); if (message.id !== current) return; ws.removeEventListener("message", listener); message.error ? reject(new Error(message.error.message)) : resolveCall(message.result); };
    ws.addEventListener("message", listener);
    ws.send(JSON.stringify({id:current,method,params}));
  });
  await call("Page.enable");
  await delay(300);
  const result = await call("Runtime.evaluate", {returnByValue:true, expression:`(() => { const card=document.querySelector('.card'); const button=document.querySelector('.action'); const cs=getComputedStyle(card); const bs=getComputedStyle(button); return {title:document.title, cardDisplay:cs.display, cardRadius:cs.borderRadius, buttonDisplay:bs.display, bodyMargin:getComputedStyle(document.body).margin, stylesheetCount:document.styleSheets.length}; })()`});
  const value = result.result.value;
  if (!value) throw new Error(`evaluation failed: ${JSON.stringify(result)}`);
  if (value.title !== "Northstar release dashboard" || value.cardDisplay !== "grid" || value.cardRadius !== "16px" || value.buttonDisplay !== "block" || value.bodyMargin !== "0px" || value.stylesheetCount !== 1) throw new Error(`computed style mismatch: ${JSON.stringify(value)}`);
  ws.close();
  process.stdout.write(`${JSON.stringify({schemaVersion:1,browser:version.Browser,protocolVersion:version["Protocol-Version"],computed:value},null,2)}\n`);
} finally {
  child.kill();
  server.close();
  for (let attempt = 0; attempt < 20; attempt++) {
    try { rmSync(profile, {recursive:true,force:true,maxRetries:2,retryDelay:100}); break; }
    catch (error) { if (attempt === 19) throw error; await delay(100); }
  }
}
