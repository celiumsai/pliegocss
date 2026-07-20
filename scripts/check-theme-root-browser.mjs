import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { setTimeout as delay } from "node:timers/promises";

const browser = process.argv[2];
if (!browser) throw new Error("browser executable required");
const css = `@theme { --color-brand-500: oklch(68% 0.17 230); --space-base: 8px; --space-card: calc(var(--space-base) * 2); --color-card: var(--color-brand-500); }\n/* PliegoCSS projected Tailwind theme */\n:root { --color-brand-500: oklch(68% 0.17 230); --space-base: 8px; --space-card: calc(var(--space-base) * 2); --color-card: var(--color-brand-500); }\n.swatch { color: var(--color-card); padding: var(--space-card); }\n`;
const html = `<!doctype html><link rel="stylesheet" href="/app.css"><div class="swatch">x</div>`;
const server=createServer((req,res)=>{if(req.url==="/app.css"){res.writeHead(200,{"content-type":"text/css"});res.end(css)}else{res.writeHead(200,{"content-type":"text/html"});res.end(html)}});
await new Promise((ok)=>server.listen(0,"127.0.0.1",ok)); const port=server.address().port;
const profile=mkdtempSync(join(tmpdir(),"pliegocss-theme-cdp-")); const debugPort=9800+Math.floor(Math.random()*100);
const child=spawn(browser,["--headless=new",`--remote-debugging-port=${debugPort}`,`--user-data-dir=${profile}`,`http://127.0.0.1:${port}/`],{stdio:"ignore"});
try{
 let target,version; for(let i=0;i<50;i++){try{version=await(await fetch(`http://127.0.0.1:${debugPort}/json/version`)).json();const list=await(await fetch(`http://127.0.0.1:${debugPort}/json/list`)).json();target=list.find(x=>x.type==="page"&&x.url.includes(`:${port}`));if(target)break}catch{}await delay(100)}
 if(!target)throw new Error("CDP target unavailable"); const ws=new WebSocket(target.webSocketDebuggerUrl); await new Promise((ok,bad)=>{ws.addEventListener("open",ok,{once:true});ws.addEventListener("error",bad,{once:true})});
 let id=0;const call=(method,params={})=>new Promise((ok,bad)=>{const n=++id;const h=e=>{const m=JSON.parse(e.data);if(m.id!==n)return;ws.removeEventListener("message",h);m.error?bad(new Error(m.error.message)):ok(m.result)};ws.addEventListener("message",h);ws.send(JSON.stringify({id:n,method,params}))});
 await delay(300);const evaluation=await call("Runtime.evaluate",{returnByValue:true,expression:`(()=>{const rootStyle=getComputedStyle(document.documentElement);const swatch=getComputedStyle(document.querySelector('.swatch'));return{root:rootStyle.getPropertyValue('--color-brand-500').trim(),spacing:rootStyle.getPropertyValue('--space-card').trim(),color:swatch.color,padding:swatch.paddingTop}})()`}); const value=evaluation.result.value;
 if(value.root!=="oklch(68% 0.17 230)"||value.spacing!=="calc(8px * 2)"||!value.color.startsWith("oklch(")||value.padding!=="16px")throw new Error(`theme projection mismatch: ${JSON.stringify(value)}`);
 ws.close();process.stdout.write(`${JSON.stringify({schemaVersion:1,browser:version.Browser,customProperty:value.root,computedColor:value.color,computedPadding:value.padding,orderedVarSpacing:value.spacing},null,2)}\n`);
}finally{child.kill();server.close();for(let i=0;i<20;i++){try{rmSync(profile,{recursive:true,force:true,maxRetries:2,retryDelay:100});break}catch(e){if(i===19)throw e;await delay(100)}}}
