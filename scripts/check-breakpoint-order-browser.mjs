import { spawn, spawnSync } from "node:child_process";
import { createServer } from "node:http";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { cargoTargetRoot, isolatedCargoEnvironment } from "./rust-target.mjs";

const browser = process.argv[2];
if (!browser) throw new Error("browser executable required");
const root = resolve(import.meta.dirname, "..");
const cargoEnvironment = isolatedCargoEnvironment(root);
const cargoTarget = cargoTargetRoot(root, cargoEnvironment);
const binary = join(
  cargoTarget,
  "debug",
  process.platform === "win32" ? "pliego-cssc.exe" : "pliego-cssc",
);
const runtime = mkdtempSync(join(tmpdir(), "pliegocss-breakpoint-order-"));
const profile = mkdtempSync(join(tmpdir(), "pliegocss-breakpoint-cdp-"));
const config = join(runtime, "pliego.theme.toml");
const cssPath = join(runtime, "app.css");
const manifestPath = join(runtime, "app.manifest.json");
writeFileSync(
  config,
  'schema = 1\nextends = "seed"\n\n[breakpoints]\ncontent-wide = "72rem"\ntablet = "52rem"\n',
);
const build = spawnSync("cargo", ["build", "--quiet", "--locked", "-p", "pliego-cssc"], {
  cwd: root,
  env: cargoEnvironment,
  encoding: "utf8",
  windowsHide: true,
});
if (build.error) throw build.error;
if (build.status !== 0) throw new Error(`compiler build failed:\n${build.stdout}${build.stderr}`);
const compile = spawnSync(
  binary,
  [
    "compile",
    "--style",
    "grid grid-cols-1 tablet:grid-cols-2 content-wide:grid-cols-3",
    "--config",
    config,
    "--output",
    cssPath,
    "--manifest",
    manifestPath,
  ],
  { cwd: root, encoding: "utf8", windowsHide: true },
);
if (compile.error) throw compile.error;
if (compile.status !== 0) throw new Error(`compiler invocation failed:\n${compile.stdout}${compile.stderr}`);
const css = readFileSync(cssPath, "utf8");
const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
const className = manifest.styles?.[0]?.className;
if (!/^pc_[a-z0-9]+$/u.test(className ?? "")) throw new Error("compiled class name missing");
const narrowIndex = css.indexOf("(width>=52rem)");
const wideIndex = css.indexOf("(width>=72rem)");
if (narrowIndex < 0 || wideIndex <= narrowIndex) {
  throw new Error(`compiled media order is not 52rem before 72rem:\n${css}`);
}

const html = `<!doctype html><link rel="stylesheet" href="/app.css"><div id="grid" class="${className}"><i></i><i></i><i></i></div>`;
const server = createServer((request, response) => {
  if (request.url === "/app.css") {
    response.writeHead(200, { "content-type": "text/css" });
    response.end(css);
    return;
  }
  response.writeHead(200, { "content-type": "text/html" });
  response.end(html);
});
await new Promise((ready) => server.listen(0, "127.0.0.1", ready));
const port = server.address().port;
const debugPort = 10_200 + Math.floor(Math.random() * 300);
const child = spawn(
  browser,
  [
    "--headless=new",
    `--remote-debugging-port=${debugPort}`,
    `--user-data-dir=${profile}`,
    "--no-first-run",
    "--disable-default-apps",
    `http://127.0.0.1:${port}/`,
  ],
  { stdio: "ignore" },
);

try {
  let target;
  let version;
  for (let attempt = 0; attempt < 60; attempt++) {
    try {
      version = await (await fetch(`http://127.0.0.1:${debugPort}/json/version`)).json();
      const targets = await (await fetch(`http://127.0.0.1:${debugPort}/json/list`)).json();
      target = targets.find((entry) => entry.type === "page" && entry.url.includes(`:${port}`));
      if (target) break;
    } catch {}
    await delay(100);
  }
  if (!target) throw new Error("CDP target unavailable");
  const socket = new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((open, reject) => {
    socket.addEventListener("open", open, { once: true });
    socket.addEventListener("error", reject, { once: true });
  });
  let id = 0;
  const call = (method, params = {}) =>
    new Promise((resolveCall, reject) => {
      const current = ++id;
      const listener = (event) => {
        const message = JSON.parse(event.data);
        if (message.id !== current) return;
        socket.removeEventListener("message", listener);
        message.error ? reject(new Error(message.error.message)) : resolveCall(message.result);
      };
      socket.addEventListener("message", listener);
      socket.send(JSON.stringify({ id: current, method, params }));
    });
  await call("Page.enable");
  const observations = [];
  for (const [width, expectedColumns] of [
    [800, 1],
    [900, 2],
    [1200, 3],
  ]) {
    await call("Emulation.setDeviceMetricsOverride", {
      width,
      height: 800,
      deviceScaleFactor: 1,
      mobile: false,
    });
    await delay(100);
    const evaluation = await call("Runtime.evaluate", {
      returnByValue: true,
      expression:
        "(() => getComputedStyle(document.querySelector('#grid')).gridTemplateColumns.split(' ').filter(Boolean).length)()",
    });
    const actualColumns = evaluation.result.value;
    observations.push({ width, columns: actualColumns });
    if (actualColumns !== expectedColumns) {
      throw new Error(
        `breakpoint regression at ${width}px: expected ${expectedColumns}, found ${actualColumns}`,
      );
    }
  }
  socket.close();
  process.stdout.write(
    `${JSON.stringify(
      {
        schemaVersion: 1,
        browser: version.Browser,
        breakpoints: ["52rem", "72rem"],
        compiledOrder: "narrow-to-wide",
        observations,
      },
      null,
      2,
    )}\n`,
  );
} finally {
  if (process.platform === "win32") {
    spawnSync("taskkill", ["/pid", String(child.pid), "/t", "/f"], {
      stdio: "ignore",
      windowsHide: true,
    });
  } else {
    child.kill();
  }
  server.close();
  await delay(300);
  for (const directory of [profile, runtime]) {
    for (let attempt = 0; attempt < 20; attempt++) {
      try {
        rmSync(directory, { recursive: true, force: true, maxRetries: 2, retryDelay: 100 });
        break;
      } catch (error) {
        if (attempt === 19) {
          process.stderr.write(`warning: deferred browser temp cleanup for ${directory}: ${error.message}\n`);
          break;
        }
        await delay(100);
      }
    }
  }
}
