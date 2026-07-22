import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { cpSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { cargoTargetRoot } from "./rust-target.mjs";
const root=resolve(import.meta.dirname,".."),source=resolve(root,"integration-tests/representative/vite-tailwind-inventory"),stage=resolve(root,"target/tests/vite-tailwind-css-template-group"),binary=resolve(cargoTargetRoot(root),"debug",process.platform==="win32"?"pliego-cssc.exe":"pliego-cssc");
const run=(args)=>{const r=spawnSync(binary,args,{cwd:stage,encoding:"utf8"});if(r.status!==0)throw new Error(`${args.join(' ')} failed:\n${r.stderr}`);return r.stdout};const sha=b=>createHash("sha256").update(b).digest("hex");
rmSync(stage,{recursive:true,force:true});mkdirSync(stage,{recursive:true});cpSync(source,stage,{recursive:true});
const cssFile=resolve(stage,"src/app.css"),htmlFile=resolve(stage,"index.html"),cssBefore=readFileSync(cssFile),htmlBefore=readFileSync(htmlFile);
const utility=cssBefore.toString().match(/@utility\s+([A-Za-z0-9_-]+)\s*\{([^{}]*)\}/);if(!utility)throw new Error("static utility absent");
const alias="pc-content-auto";const cssAfter=Buffer.concat([cssBefore,Buffer.from(`/* PliegoCSS projected Tailwind utility */\n.${alias} {${utility[2]}}\n`)]);
const marker='class="rounded-2xl border';if(!htmlBefore.toString().includes(marker))throw new Error("static template marker absent");const htmlAfter=Buffer.from(htmlBefore.toString().replace(marker,`class="${alias} rounded-2xl border`));
for(const [name,before,after] of [["css",cssBefore,cssAfter],["html",htmlBefore,htmlAfter]]){writeFileSync(resolve(stage,`${name}.before`),before);writeFileSync(resolve(stage,`${name}.after`),after)}
const receipts=[];try{for(const [file,before,after,name] of [["src/app.css","css.before","css.after","css"],["index.html","html.before","html.after","html"]]){const receipt=`${name}.receipt.json`;run(["migration-replace-apply","--file",file,"--before",before,"--after",after,"--receipt",receipt]);receipts.push([file,receipt])}}catch(error){for(const [file,receipt] of receipts.reverse())run(["migration-replace-rollback","--file",file,"--receipt",receipt]);throw error}
if(sha(readFileSync(cssFile))!==sha(cssAfter)||sha(readFileSync(htmlFile))!==sha(htmlAfter))throw new Error("group apply identity mismatch");
for(const [file,receipt] of receipts.reverse())run(["migration-replace-rollback","--file",file,"--receipt",receipt]);
if(sha(readFileSync(cssFile))!==sha(cssBefore)||sha(readFileSync(htmlFile))!==sha(htmlBefore))throw new Error("group rollback mismatch");
rmSync(stage,{recursive:true,force:true});process.stdout.write(`${JSON.stringify({schemaVersion:1,files:["src/app.css","index.html"],alias,apply:"passed",identities:"passed",rollback:"passed"},null,2)}\n`);
