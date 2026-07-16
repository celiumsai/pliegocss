import { spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { basename, extname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
const fixtureRoot = join(root, "integration-tests", "plain-html-smoke");
const adapterPath = join(fixtureRoot, "adapter.json");

function fail(message, details = undefined) {
  const suffix = details === undefined ? "" : `\n${JSON.stringify(details, null, 2)}`;
  throw new Error(`${message}${suffix}`);
}

function sleep(milliseconds) {
  return new Promise((resolvePromise) => setTimeout(resolvePromise, milliseconds));
}

function exactKeys(value, expected, label) {
  const actual = Object.keys(value).sort();
  const wanted = [...expected].sort();
  if (JSON.stringify(actual) !== JSON.stringify(wanted)) {
    fail(`${label} does not use the closed fixture schema`, { actual, expected: wanted });
  }
}

function safeFixturePath(logical, label) {
  if (
    typeof logical !== "string" ||
    logical.length === 0 ||
    logical.includes("\\") ||
    logical.startsWith("/") ||
    logical.split("/").some((segment) => segment === "" || segment === "." || segment === "..")
  ) {
    fail(`${label} is not a safe fixture-relative path`, { logical });
  }
  const path = resolve(fixtureRoot, logical);
  if (!path.startsWith(`${fixtureRoot}\\`) && !path.startsWith(`${fixtureRoot}/`)) {
    fail(`${label} escaped the fixture root`, { logical, path });
  }
  return path;
}

function readAdapter() {
  const adapter = JSON.parse(readFileSync(adapterPath, "utf8"));
  exactKeys(
    adapter,
    ["schemaVersion", "styleInput", "template", "output", "slots"],
    "Plain HTML adapter",
  );
  if (adapter.schemaVersion !== 1 || !Array.isArray(adapter.slots) || adapter.slots.length === 0) {
    fail("Plain HTML adapter must use schema 1 and declare at least one slot");
  }
  const ids = new Set();
  const markers = new Set();
  const lines = new Set();
  for (const slot of adapter.slots) {
    exactKeys(slot, ["id", "marker", "styleLine"], "Plain HTML adapter slot");
    if (
      typeof slot.id !== "string" ||
      !/^[a-z][a-z0-9-]*$/u.test(slot.id) ||
      typeof slot.marker !== "string" ||
      slot.marker !== `{{PLIEGOCSS_CLASS:${slot.id}}}` ||
      !Number.isSafeInteger(slot.styleLine) ||
      slot.styleLine < 1 ||
      ids.has(slot.id) ||
      markers.has(slot.marker) ||
      lines.has(slot.styleLine)
    ) {
      fail("Plain HTML adapter contains an invalid or duplicate slot", { slot });
    }
    ids.add(slot.id);
    markers.add(slot.marker);
    lines.add(slot.styleLine);
  }
  return {
    ...adapter,
    outputPath: safeFixturePath(adapter.output, "Adapter output"),
    styleInputPath: safeFixturePath(adapter.styleInput, "Adapter style input"),
    templatePath: safeFixturePath(adapter.template, "Adapter template"),
  };
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function lineRecords(bytes) {
  const text = bytes.toString("utf8");
  if (Buffer.from(text, "utf8").compare(bytes) !== 0 || text.includes("\r")) {
    fail("Plain HTML style input must be canonical UTF-8 with LF line endings");
  }
  const records = [];
  let byteStart = 0;
  for (const source of text.split("\n")) {
    if (source.length > 0) {
      const byteEnd = byteStart + Buffer.byteLength(source, "utf8");
      records.push({ byteEnd, byteStart, source });
      byteStart = byteEnd + 1;
    } else {
      byteStart += 1;
    }
  }
  return records;
}

function runCompiler(adapter, outputRoot) {
  const cssPath = join(outputRoot, "app.css");
  const manifestPath = join(outputRoot, "app.manifest.json");
  const result = spawnSync(
    "cargo",
    [
      "+1.85.0",
      "run",
      "--quiet",
      "--locked",
      "-p",
      "pliego-cssc",
      "--",
      "compile",
      "--input",
      adapter.styleInputPath,
      "--seed",
      "--targets",
      "modern",
      "--output",
      cssPath,
      "--manifest",
      manifestPath,
      "--manifest-version",
      "3",
    ],
    { cwd: root, encoding: "utf8", windowsHide: true },
  );
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    fail("Plain HTML fixture compilation failed", {
      exitCode: result.status,
      stderr: result.stderr,
      stdout: result.stdout,
    });
  }
  return {
    css: readFileSync(cssPath),
    cssPath,
    manifest: JSON.parse(readFileSync(manifestPath, "utf8")),
    manifestPath,
  };
}

function buildSite(adapter, outputRoot) {
  const styleBytes = readFileSync(adapter.styleInputPath);
  const records = lineRecords(styleBytes);
  if (adapter.slots.length !== records.length) {
    fail("Plain HTML fixture requires exactly one declared slot per style line", {
      slotCount: adapter.slots.length,
      styleLineCount: records.length,
    });
  }
  const compiled = runCompiler(adapter, outputRoot);
  const { manifest } = compiled;
  if (
    manifest.schemaVersion !== 3 ||
    manifest.targets !== "modern" ||
    manifest.format !== "minified" ||
    manifest.cssBytes !== compiled.css.length ||
    manifest.cssSha256 !== sha256(compiled.css) ||
    !compiled.css.toString("utf8").endsWith("\n") ||
    !Array.isArray(manifest.styles) ||
    manifest.styles.length !== records.length
  ) {
    fail("Compiler output does not satisfy the plain HTML artifact contract", {
      cssBytes: compiled.css.length,
      manifest,
    });
  }

  let html = readFileSync(adapter.templatePath, "utf8");
  const classes = {};
  const boundStyleIds = new Set();
  for (const slot of adapter.slots) {
    const record = records[slot.styleLine - 1];
    if (!record) {
      fail("Adapter slot references a missing style line", { slot });
    }
    const matches = manifest.styles.filter(
      (candidate) =>
        candidate.origins?.length === 1 &&
        resolve(candidate.origins[0].file ?? "") === resolve(adapter.styleInputPath) &&
        candidate.origins[0].macroKind === "input" &&
        candidate.origins[0].reason === "line-oriented-input" &&
        candidate.origins[0].source === record.source &&
        candidate.origins[0].byteStart === record.byteStart &&
        candidate.origins[0].byteEnd === record.byteEnd,
    );
    const style = matches[0];
    if (
      matches.length !== 1 ||
      !/^pc_[a-z0-9]+$/u.test(style.className) ||
      boundStyleIds.has(style.styleId)
    ) {
      fail("Adapter could not bind a declared style line to one manifest class", {
        matchCount: matches.length,
        record,
        slot,
      });
    }
    if (html.split(slot.marker).length !== 2) {
      fail("Adapter marker must occur exactly once in the HTML template", { slot });
    }
    boundStyleIds.add(style.styleId);
    classes[slot.id] = style.className;
    html = html.replace(slot.marker, style.className);
  }
  if (html.includes("{{PLIEGOCSS_CLASS:")) {
    fail("Generated HTML retained an unbound PliegoCSS marker");
  }
  writeFileSync(join(outputRoot, adapter.output), html, "utf8");
  return { classes, compiled, html, records };
}

function executableFromPath(command) {
  const locator = process.platform === "win32" ? "where.exe" : "which";
  const result = spawnSync(locator, [command], {
    encoding: "utf8",
    windowsHide: true,
  });
  return result.status === 0
    ? result.stdout.split(/\r?\n/u).map((line) => line.trim()).find(Boolean)
    : undefined;
}

function findChrome() {
  const candidates = [
    process.env.PLIEGOCSS_CHROME_PATH,
    process.platform === "win32"
      ? join(
          process.env.PROGRAMFILES ?? "C:\\Program Files",
          "Google",
          "Chrome",
          "Application",
          "chrome.exe",
        )
      : undefined,
    process.platform === "win32" && process.env["PROGRAMFILES(X86)"]
      ? join(
          process.env["PROGRAMFILES(X86)"],
          "Google",
          "Chrome",
          "Application",
          "chrome.exe",
        )
      : undefined,
    process.platform === "win32" && process.env.LOCALAPPDATA
      ? join(
          process.env.LOCALAPPDATA,
          "Google",
          "Chrome",
          "Application",
          "chrome.exe",
        )
      : undefined,
    process.platform === "darwin"
      ? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
      : undefined,
    process.platform === "darwin"
      ? "/Applications/Chromium.app/Contents/MacOS/Chromium"
      : undefined,
  ].filter(Boolean);
  for (const candidate of candidates) {
    if (existsSync(candidate)) {
      return candidate;
    }
  }
  for (const command of ["google-chrome", "chrome", "chromium", "chromium-browser"]) {
    const located = executableFromPath(command);
    if (located) {
      return located;
    }
  }
  fail("Chrome or Chromium was not found; set PLIEGOCSS_CHROME_PATH");
}

function contentType(path) {
  return extname(path) === ".css"
    ? "text/css; charset=utf-8"
    : "text/html; charset=utf-8";
}

async function startServer(outputRoot, outputName) {
  const errors = [];
  const requests = new Map();
  const allowed = new Map([
    ["/", join(outputRoot, outputName)],
    ["/index.html", join(outputRoot, outputName)],
    ["/app.css", join(outputRoot, "app.css")],
  ]);
  const server = createServer((request, response) => {
    try {
      const pathname = new URL(request.url ?? "/", "http://127.0.0.1").pathname;
      requests.set(pathname, (requests.get(pathname) ?? 0) + 1);
      if (pathname === "/favicon.ico") {
        response.writeHead(204, { "Cache-Control": "no-store" });
        response.end();
        return;
      }
      const path = allowed.get(pathname);
      if (!path || !existsSync(path) || !statSync(path).isFile()) {
        response.writeHead(404, { "Content-Type": "text/plain; charset=utf-8" });
        response.end("Not found");
        return;
      }
      response.writeHead(200, {
        "Cache-Control": "no-store",
        "Content-Type": contentType(path),
      });
      response.end(readFileSync(path));
    } catch (error) {
      errors.push(String(error?.stack ?? error));
      response.writeHead(500, { "Content-Type": "text/plain; charset=utf-8" });
      response.end("Internal server error");
    }
  });
  await new Promise((resolvePromise, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolvePromise);
  });
  const address = server.address();
  if (!address || typeof address === "string") {
    fail("Plain HTML fixture server did not expose a TCP address");
  }
  return { errors, requests, server, url: `http://127.0.0.1:${address.port}/` };
}

async function waitForDevToolsPort(profilePath, chrome, startup) {
  const activePortPath = join(profilePath, "DevToolsActivePort");
  for (let attempt = 0; attempt < 160; attempt += 1) {
    if (existsSync(activePortPath)) {
      const port = Number(readFileSync(activePortPath, "utf8").split(/\r?\n/u)[0]);
      if (Number.isInteger(port) && port > 0) {
        return port;
      }
    }
    if (chrome.exitCode !== null) {
      fail("Chrome exited before exposing DevTools", { exitCode: chrome.exitCode, startup });
    }
    await sleep(125);
  }
  fail("Chrome DevTools endpoint did not become available", { startup });
}

async function fetchJsonWithRetry(url) {
  for (let attempt = 0; attempt < 80; attempt += 1) {
    try {
      const response = await fetch(url);
      if (response.ok) {
        return await response.json();
      }
    } catch {
      // The port file can appear before the endpoint accepts connections.
    }
    await sleep(125);
  }
  fail("Chrome DevTools JSON endpoint did not become available", { url });
}

async function connectCdp(webSocketUrl, events) {
  const socket = new WebSocket(webSocketUrl);
  await new Promise((resolvePromise, reject) => {
    socket.addEventListener("open", resolvePromise, { once: true });
    socket.addEventListener("error", reject, { once: true });
  });
  let nextId = 1;
  const pending = new Map();
  socket.addEventListener("message", (event) => {
    const message = JSON.parse(event.data);
    if (message.id && pending.has(message.id)) {
      const entry = pending.get(message.id);
      pending.delete(message.id);
      if (message.error) {
        entry.reject(new Error(JSON.stringify(message.error)));
      } else {
        entry.resolve(message.result);
      }
    } else if (message.method === "Runtime.exceptionThrown") {
      events.push({ kind: "exception", detail: message.params.exceptionDetails });
    } else if (message.method === "Runtime.consoleAPICalled") {
      events.push({ kind: "console", level: message.params.type });
    } else if (message.method === "Log.entryAdded") {
      events.push({
        kind: "log",
        level: message.params.entry.level,
        text: message.params.entry.text,
      });
    } else if (message.method === "Network.loadingFailed") {
      events.push({
        kind: "network",
        level: "error",
        text: message.params.errorText,
      });
    }
  });
  return {
    send(method, params = {}) {
      const id = nextId;
      nextId += 1;
      socket.send(JSON.stringify({ id, method, params }));
      return new Promise((resolvePromise, reject) => {
        pending.set(id, { reject, resolve: resolvePromise });
      });
    },
    socket,
  };
}

async function closeServer(server) {
  await new Promise((resolvePromise) => server.close(resolvePromise));
}

async function waitForChildExit(child, timeoutMilliseconds) {
  if (child.exitCode !== null) {
    return true;
  }
  return await new Promise((resolvePromise) => {
    const timer = setTimeout(() => resolvePromise(false), timeoutMilliseconds);
    child.once("exit", () => {
      clearTimeout(timer);
      resolvePromise(true);
    });
  });
}

const adapter = readAdapter();
const events = [];
const startup = [];
let chrome;
let chromePath;
let cdp;
let outputRoot;
let profilePath;
let site;
let siteBuild;

try {
  outputRoot = mkdtempSync(join(tmpdir(), "pliegocss-plain-html-"));
  siteBuild = buildSite(adapter, outputRoot);
  chromePath = findChrome();
  profilePath = mkdtempSync(join(tmpdir(), "pliegocss-plain-html-cdp-"));
  site = await startServer(outputRoot, adapter.output);
  chrome = spawn(
    chromePath,
    [
      "--headless=new",
      "--disable-background-networking",
      "--disable-component-update",
      "--disable-default-apps",
      "--disable-extensions",
      "--disable-gpu",
      "--disable-sync",
      "--no-first-run",
      "--no-default-browser-check",
      "--remote-debugging-port=0",
      `--user-data-dir=${profilePath}`,
      "about:blank",
    ],
    { stdio: ["ignore", "ignore", "pipe"], windowsHide: true },
  );
  chrome.stderr.on("data", (chunk) => startup.push(chunk.toString()));
  const port = await waitForDevToolsPort(profilePath, chrome, startup);
  const pages = await fetchJsonWithRetry(`http://127.0.0.1:${port}/json/list`);
  const page = pages.find((entry) => entry.type === "page");
  if (!page?.webSocketDebuggerUrl) {
    fail("Chrome did not expose a page target", { pages });
  }
  cdp = await connectCdp(page.webSocketDebuggerUrl, events);
  const browserVersion = await cdp.send("Browser.getVersion");
  await cdp.send("Runtime.enable");
  await cdp.send("Log.enable");
  await cdp.send("Network.enable");
  await cdp.send("Network.setCacheDisabled", { cacheDisabled: true });
  await cdp.send("Page.enable");
  await cdp.send("Page.navigate", { url: site.url });
  const result = await cdp.send("Runtime.evaluate", {
    awaitPromise: true,
    returnByValue: true,
    expression: `new Promise((resolve) => {
      const startedAt = Date.now();
      const poll = () => {
        const card = document.querySelector('#card');
        const action = document.querySelector('#action');
        if (document.readyState === 'complete' && card && action) {
          const cardStyle = getComputedStyle(card);
          const actionStyle = getComputedStyle(action);
          resolve({
            action: {
              alignItems: actionStyle.alignItems,
              className: action.className,
              display: actionStyle.display,
              justifyContent: actionStyle.justifyContent,
              paddingBottom: actionStyle.paddingBottom,
              paddingLeft: actionStyle.paddingLeft,
              paddingRight: actionStyle.paddingRight,
              paddingTop: actionStyle.paddingTop,
            },
            card: {
              borderRadius: cardStyle.borderRadius,
              boxShadow: cardStyle.boxShadow,
              className: card.className,
              display: cardStyle.display,
              gap: cardStyle.gap,
              padding: cardStyle.padding,
            },
            resourceEntries: performance.getEntriesByType('resource').map((entry) => ({
              initiatorType: entry.initiatorType,
              name: entry.name,
            })),
            scriptCount: document.scripts.length,
            stylesheetHrefs: [...document.querySelectorAll('link[rel="stylesheet"]')]
              .map((link) => link.getAttribute('href')),
            wasmResourceCount: performance.getEntriesByType('resource')
              .filter((entry) => new URL(entry.name).pathname.endsWith('.wasm')).length,
          });
          return;
        }
        if (Date.now() - startedAt > 10000) {
          resolve({ timeout: true, readyState: document.readyState });
          return;
        }
        setTimeout(poll, 25);
      };
      poll();
    })`,
  });
  if (result.exceptionDetails) {
    fail("Plain HTML browser evaluation raised an exception", result.exceptionDetails);
  }
  await sleep(200);
  const observed = result.result?.value;
  const relevantEvents = events.filter(
    (event) =>
      event.kind === "exception" ||
      event.kind === "network" ||
      ["assert", "error", "warning"].includes(event.level),
  );
  const serverRequests = Object.fromEntries(
    [...site.requests.entries()].sort(([left], [right]) => left.localeCompare(right, "en")),
  );
  const cssResources = observed?.resourceEntries?.filter(
    (entry) => entry.name === new URL("/app.css", site.url).href,
  );
  const passed =
    !observed?.timeout &&
    observed?.card?.className === siteBuild.classes.card &&
    observed?.card?.display === "grid" &&
    observed?.card?.gap === "16px" &&
    observed?.card?.padding === "16px" &&
    observed?.card?.borderRadius === "8px" &&
    observed?.card?.boxShadow !== "none" &&
    observed?.action?.className === siteBuild.classes.action &&
    observed?.action?.display === "inline-flex" &&
    observed?.action?.alignItems === "center" &&
    observed?.action?.justifyContent === "center" &&
    observed?.action?.paddingTop === "8px" &&
    observed?.action?.paddingBottom === "8px" &&
    observed?.action?.paddingLeft === "16px" &&
    observed?.action?.paddingRight === "16px" &&
    observed?.scriptCount === 0 &&
    observed?.wasmResourceCount === 0 &&
    JSON.stringify(observed?.stylesheetHrefs) === JSON.stringify(["./app.css"]) &&
    cssResources?.length === 1 &&
    cssResources[0]?.initiatorType === "link" &&
    serverRequests["/app.css"] === 1 &&
    relevantEvents.length === 0 &&
    site.errors.length === 0;
  const report = {
    schema: "pliegocss/plain-html-browser-gate/1",
    passed,
    adapter: {
      schemaVersion: adapter.schemaVersion,
      slotCount: adapter.slots.length,
      styleInputSha256: sha256(readFileSync(adapter.styleInputPath)),
    },
    artifacts: {
      cssBytes: siteBuild.compiled.css.length,
      cssSha256: sha256(siteBuild.compiled.css),
      htmlBytes: Buffer.byteLength(siteBuild.html, "utf8"),
      htmlSha256: sha256(Buffer.from(siteBuild.html, "utf8")),
      manifestSchemaVersion: siteBuild.compiled.manifest.schemaVersion,
      styleCount: siteBuild.compiled.manifest.styles.length,
    },
    browser: {
      executable: basename(chromePath),
      product: browserVersion.product,
      protocolVersion: browserVersion.protocolVersion,
      userAgent: browserVersion.userAgent,
    },
    classes: siteBuild.classes,
    observed,
    relevantEvents,
    serverErrors: site.errors,
    serverRequests,
  };
  if (!passed) {
    fail("Plain HTML browser gate failed", report);
  }
  console.log(JSON.stringify(report, null, 2));
} finally {
  if (cdp) {
    try {
      await cdp.send("Browser.close");
    } catch {
      // Chrome may already be closing after a protocol failure.
    }
    cdp.socket.close();
  }
  if (chrome && !(await waitForChildExit(chrome, 5000))) {
    chrome.kill();
    await waitForChildExit(chrome, 5000);
  }
  if (site) {
    await closeServer(site.server);
  }
  if (profilePath) {
    rmSync(profilePath, { force: true, maxRetries: 20, recursive: true, retryDelay: 100 });
  }
  if (outputRoot) {
    rmSync(outputRoot, { force: true, maxRetries: 20, recursive: true, retryDelay: 100 });
  }
}
