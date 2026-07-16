import { spawn, spawnSync } from "node:child_process";
import {
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  statSync,
} from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { basename, extname, isAbsolute, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(fileURLToPath(new URL("..", import.meta.url)));
const integrationScript = join(root, "scripts", "check-pliegors-integration.mjs");
const siteRoot = join(
  root,
  "integration-tests",
  "pliegors-smoke",
  "target",
  "site",
);
const expected = Object.freeze({
  clientBootstrapHref: "/assets/pliegocss-pliegors-client-bootstrap.js",
  clientModuleHref: "/assets/pliegocss_pliegors_client.js",
  clientWasmHref: "/assets/pliegocss_pliegors_client_bg.wasm",
  islandId: "visit-counter",
  initialMinutes: 15,
  increment: 5,
  finalMinutes: 20,
  resumeRuntimeHref: "/assets/pliego-resume.js",
  stylesheetPreloadHref: "/assets/shared.css",
});
const skipBuild = process.env.PLIEGOCSS_SKIP_PLIEGORS_BUILD === "1";
const keepSite = process.env.PLIEGOCSS_KEEP_PLIEGORS_SITE === "1";
const agentReport = process.env.PLIEGOCSS_AGENT_REPORT === "1";

function fail(message, details = undefined) {
  const suffix = details === undefined ? "" : `\n${JSON.stringify(details, null, 2)}`;
  throw new Error(`${message}${suffix}`);
}

function sleep(milliseconds) {
  return new Promise((resolvePromise) => setTimeout(resolvePromise, milliseconds));
}

function runIntegrationGate() {
  if (skipBuild) {
    if (!existsSync(join(siteRoot, "visit", "index.html"))) {
      fail(
        "PLIEGOCSS_SKIP_PLIEGORS_BUILD=1 requires an existing generated PliegoRS site",
      );
    }
    return;
  }

  const result = spawnSync(process.execPath, [integrationScript], {
    cwd: root,
    encoding: "utf8",
    env: {
      ...process.env,
      PLIEGOCSS_KEEP_PLIEGORS_SITE: "1",
    },
    stdio: ["ignore", "pipe", "pipe"],
  });
  if (result.stdout) {
    process.stderr.write(result.stdout);
  }
  if (result.stderr) {
    process.stderr.write(result.stderr);
  }
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    fail("PliegoRS integration gate failed before the browser replay", {
      exitCode: result.status,
      signal: result.signal,
    });
  }
}

function executableFromPath(command) {
  const locator = process.platform === "win32" ? "where.exe" : "which";
  const result = spawnSync(locator, [command], {
    encoding: "utf8",
    windowsHide: true,
  });
  if (result.status !== 0) {
    return undefined;
  }
  return result.stdout
    .split(/\r?\n/u)
    .map((line) => line.trim())
    .find(Boolean);
}

function findChrome() {
  const explicit = process.env.PLIEGOCSS_CHROME_PATH;
  const candidates = [
    explicit,
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
  fail(
    "Chrome or Chromium was not found; set PLIEGOCSS_CHROME_PATH to an executable",
  );
}

function contentType(path) {
  switch (extname(path)) {
    case ".css":
      return "text/css; charset=utf-8";
    case ".html":
      return "text/html; charset=utf-8";
    case ".js":
      return "text/javascript; charset=utf-8";
    case ".json":
      return "application/json; charset=utf-8";
    case ".wasm":
      return "application/wasm";
    default:
      return "application/octet-stream";
  }
}

async function startSiteServer() {
  const errors = [];
  const requests = new Map();
  const server = createServer((request, response) => {
    try {
      const requestUrl = new URL(request.url ?? "/", "http://127.0.0.1");
      requests.set(requestUrl.pathname, (requests.get(requestUrl.pathname) ?? 0) + 1);
      if (requestUrl.pathname === "/favicon.ico") {
        response.writeHead(204, { "Cache-Control": "no-store" });
        response.end();
        return;
      }

      const decoded = decodeURIComponent(requestUrl.pathname);
      const pathWithIndex = decoded.endsWith("/") ? `${decoded}index.html` : decoded;
      const candidate = resolve(siteRoot, `.${pathWithIndex}`);
      const localPath = relative(siteRoot, candidate);
      if (
        localPath === "" ||
        localPath.startsWith("..") ||
        isAbsolute(localPath) ||
        !existsSync(candidate) ||
        !statSync(candidate).isFile()
      ) {
        response.writeHead(404, {
          "Cache-Control": "no-store",
          "Content-Type": "text/plain; charset=utf-8",
        });
        response.end("Not found");
        return;
      }

      response.writeHead(200, {
        "Cache-Control": "no-store",
        "Content-Type": contentType(candidate),
      });
      response.end(readFileSync(candidate));
    } catch (error) {
      errors.push(String(error?.stack ?? error));
      response.writeHead(500, {
        "Cache-Control": "no-store",
        "Content-Type": "text/plain; charset=utf-8",
      });
      response.end("Internal server error");
    }
  });

  await new Promise((resolvePromise, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolvePromise);
  });
  const address = server.address();
  if (!address || typeof address === "string") {
    fail("The local PliegoRS site server did not expose a TCP address");
  }
  return {
    errors,
    requests,
    server,
    url: `http://127.0.0.1:${address.port}/visit/`,
  };
}

async function waitForDevToolsPort(profilePath, chrome, startup) {
  const activePortPath = join(profilePath, "DevToolsActivePort");
  for (let attempt = 0; attempt < 160; attempt += 1) {
    if (existsSync(activePortPath)) {
      const [portLine] = readFileSync(activePortPath, "utf8").split(/\r?\n/u);
      const port = Number(portLine);
      if (Number.isInteger(port) && port > 0) {
        return port;
      }
    }
    if (chrome.exitCode !== null) {
      fail("Chrome exited before exposing its DevTools endpoint", {
        exitCode: chrome.exitCode,
        startup,
      });
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
      // Chrome can expose DevToolsActivePort before the JSON endpoint accepts connections.
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
      return;
    }

    if (message.method === "Runtime.exceptionThrown") {
      events.push({ kind: "exception", detail: message.params.exceptionDetails });
    } else if (message.method === "Runtime.consoleAPICalled") {
      events.push({
        kind: "console",
        level: message.params.type,
        text: (message.params.args ?? [])
          .map((argument) => argument.value ?? argument.description ?? "")
          .join(" "),
      });
    } else if (message.method === "Log.entryAdded") {
      events.push({
        kind: "log",
        level: message.params.entry.level,
        source: message.params.entry.source,
        text: message.params.entry.text,
      });
    } else if (message.method === "Network.loadingFailed") {
      events.push({
        kind: "network",
        level: "error",
        text: message.params.errorText,
        url: message.params.url,
      });
    }
  });

  function send(method, params = {}) {
    const id = nextId;
    nextId += 1;
    socket.send(JSON.stringify({ id, method, params }));
    return new Promise((resolvePromise, reject) => {
      pending.set(id, { reject, resolve: resolvePromise });
    });
  }

  return { send, socket };
}

function valueFromEvaluation(result, label) {
  if (result.exceptionDetails) {
    fail(`${label} raised a browser exception`, result.exceptionDetails);
  }
  return result.result?.value;
}

async function closeServer(server) {
  await new Promise((resolvePromise) => server.close(resolvePromise));
}

async function waitForChildExit(child, timeoutMilliseconds) {
  if (child.exitCode !== null) {
    return true;
  }
  return await new Promise((resolvePromise) => {
    const onExit = () => {
      clearTimeout(timer);
      resolvePromise(true);
    };
    const timer = setTimeout(() => {
      child.off("exit", onExit);
      resolvePromise(false);
    }, timeoutMilliseconds);
    child.once("exit", onExit);
  });
}

runIntegrationGate();

const chromePath = findChrome();
const profilePath = mkdtempSync(join(tmpdir(), "pliegocss-cdp-"));
const site = await startSiteServer();
const startup = [];
const events = [];
let chrome;
let cdp;

try {
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
      "--window-size=1280,800",
      "about:blank",
    ],
    {
      stdio: ["ignore", "ignore", "pipe"],
      windowsHide: true,
    },
  );
  chrome.stderr.on("data", (chunk) => startup.push(chunk.toString()));

  const devToolsPort = await waitForDevToolsPort(profilePath, chrome, startup);
  const pages = await fetchJsonWithRetry(
    `http://127.0.0.1:${devToolsPort}/json/list`,
  );
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

  const initialResult = await cdp.send("Runtime.evaluate", {
    awaitPromise: true,
    returnByValue: true,
    expression: `new Promise((resolve) => {
      const startedAt = Date.now();
      const poll = () => {
        const islands = document.querySelectorAll('pliego-island[data-pliego-id="${expected.islandId}"]');
        const island = islands[0];
        const buttons = island?.querySelectorAll('button[data-pliego-action="increment"][data-pliego-key="minutes"]');
        const values = island?.querySelectorAll('[data-pliego-bind-text="minutes"]');
        if (document.readyState === "complete" && document.documentElement.dataset.pliegoWasm === "ready" && islands.length === 1 && buttons?.length === 1 && values?.length === 1) {
          const button = buttons[0];
          const value = values[0];
          const state = JSON.parse(island.dataset.pliegoState || "{}");
          const gate = {
            button,
            buttonClass: button.className,
            document,
            events: [],
            island,
            value,
            valueClass: value.className,
          };
          island.addEventListener("pliego:state", (event) => {
            gate.events.push({ key: event.detail?.key, value: event.detail?.value });
          });
          globalThis.__pliegocssBrowserGate = gate;
          const preloadHrefs = [...document.querySelectorAll('link[rel="preload"][as="style"]')]
            .map((link) => link.getAttribute("href"));
          const stylesheetHrefs = [...document.querySelectorAll('link[rel="stylesheet"]')]
            .map((link) => link.getAttribute("href"));
          const preloadUrl = new URL("${expected.stylesheetPreloadHref}", location.href).href;
          const clientResourceUrls = [
            "${expected.clientBootstrapHref}",
            "${expected.clientModuleHref}",
            "${expected.clientWasmHref}",
            "${expected.resumeRuntimeHref}",
          ].map((path) => new URL(path, location.href).href);
          resolve({
            buttonClass: gate.buttonClass,
            buttonText: button.textContent,
            initialMinutes: state.minutes,
            islandCount: islands.length,
            islandId: island.dataset.pliegoId,
            preloadHrefs,
            preloadResourceEntries: performance.getEntriesByName(preloadUrl).map((entry) => ({
              initiatorType: entry.initiatorType,
              name: entry.name,
            })),
            clientResourceEntries: clientResourceUrls.flatMap((url) =>
              performance.getEntriesByName(url).map((entry) => ({
                initiatorType: entry.initiatorType,
                name: entry.name,
              })),
            ),
            moduleScriptSources: [...document.querySelectorAll('script[type="module"][src]')]
              .map((script) => script.getAttribute("src")),
            stylesheetHrefs,
            text: value.textContent,
            valueClass: gate.valueClass,
            wasmReady: document.documentElement.dataset.pliegoWasm,
          });
          return;
        }
        if (Date.now() - startedAt > 15000) {
          resolve({
            timeout: true,
            readyState: document.readyState,
            islandCount: islands.length,
            buttonCount: buttons?.length ?? 0,
            valueCount: values?.length ?? 0,
            wasmReady: document.documentElement.dataset.pliegoWasm,
          });
          return;
        }
        setTimeout(poll, 50);
      };
      poll();
    })`,
  });
  const initial = valueFromEvaluation(initialResult, "Initial PliegoRS state capture");
  if (
    initial?.timeout ||
    initial?.islandCount !== 1 ||
    initial?.islandId !== expected.islandId ||
    initial?.initialMinutes !== expected.initialMinutes ||
    initial?.text !== String(expected.initialMinutes) ||
    initial?.buttonText !== `+${expected.increment}` ||
    initial?.wasmReady !== "ready" ||
    JSON.stringify(initial?.moduleScriptSources) !==
      JSON.stringify([expected.resumeRuntimeHref, expected.clientBootstrapHref]) ||
    initial?.clientResourceEntries?.length !== 4 ||
    ![
      expected.clientBootstrapHref,
      expected.clientModuleHref,
      expected.clientWasmHref,
      expected.resumeRuntimeHref,
    ].every((path) =>
      initial.clientResourceEntries.some(
        (entry) => entry.name === new URL(path, site.url).href,
      ),
    ) ||
    JSON.stringify(initial?.preloadHrefs) !==
      JSON.stringify([expected.stylesheetPreloadHref]) ||
    !Array.isArray(initial?.stylesheetHrefs) ||
    !initial.stylesheetHrefs.includes(expected.stylesheetPreloadHref) ||
    initial?.preloadResourceEntries?.length !== 1 ||
    initial.preloadResourceEntries[0]?.initiatorType !== "link" ||
    initial.preloadResourceEntries[0]?.name !==
      new URL(expected.stylesheetPreloadHref, site.url).href ||
    typeof initial?.buttonClass !== "string" ||
    initial.buttonClass.length === 0 ||
    !initial.buttonClass
      .split(/\s+/u)
      .every((className) => /^pc_[a-z0-9]+$/u.test(className)) ||
    initial?.buttonClass !== initial?.valueClass
  ) {
    fail("Initial PliegoRS browser state did not match the contract", {
      expected,
      initial,
    });
  }

  const finalResult = await cdp.send("Runtime.evaluate", {
    awaitPromise: true,
    returnByValue: true,
    expression: `new Promise((resolve) => {
      const gate = globalThis.__pliegocssBrowserGate;
      gate.button.click();
      const startedAt = Date.now();
      const poll = () => {
        const island = document.querySelector('pliego-island[data-pliego-id="${expected.islandId}"]');
        const button = island?.querySelector('button[data-pliego-action="increment"][data-pliego-key="minutes"]');
        const value = island?.querySelector('[data-pliego-bind-text="minutes"]');
        const state = JSON.parse(island?.dataset.pliegoState || "{}");
        if (state.minutes === ${expected.finalMinutes} && value?.textContent === "${expected.finalMinutes}") {
          resolve({
            buttonClass: button?.className,
            documentIdentity: gate.document === document,
            eventCount: gate.events.length,
            events: gate.events,
            finalMinutes: state.minutes,
            islandCount: document.querySelectorAll('pliego-island[data-pliego-id="${expected.islandId}"]').length,
            islandId: island?.dataset.pliegoId,
            nodeIdentity: {
              button: gate.button === button,
              island: gate.island === island,
              value: gate.value === value,
            },
            text: value?.textContent,
            valueClass: value?.className,
          });
          return;
        }
        if (Date.now() - startedAt > 5000) {
          resolve({
            timeout: true,
            finalMinutes: state.minutes,
            text: value?.textContent,
            events: gate.events,
          });
          return;
        }
        setTimeout(poll, 25);
      };
      poll();
    })`,
  });
  const final = valueFromEvaluation(finalResult, "PliegoRS resumability replay");
  await sleep(250);

  const relevantEvents = events.filter((event) => {
    if (event.kind === "exception" || event.kind === "network") {
      return true;
    }
    return ["assert", "error", "warning"].includes(event.level);
  });
  const eventContract = final?.events?.[0];
  const serverRequests = Object.fromEntries(
    [...site.requests.entries()].sort(([left], [right]) => left.localeCompare(right, "en")),
  );
  const passed =
    !final?.timeout &&
    final?.documentIdentity === true &&
    final?.nodeIdentity?.island === true &&
    final?.nodeIdentity?.button === true &&
    final?.nodeIdentity?.value === true &&
    final?.islandCount === 1 &&
    final?.islandId === expected.islandId &&
    final?.finalMinutes === expected.finalMinutes &&
    final?.text === String(expected.finalMinutes) &&
    final?.buttonClass === initial.buttonClass &&
    final?.valueClass === initial.valueClass &&
    final?.eventCount === 1 &&
    eventContract?.key === "minutes" &&
    eventContract?.value === expected.finalMinutes &&
    serverRequests[expected.stylesheetPreloadHref] === 1 &&
    serverRequests[expected.clientBootstrapHref] === 1 &&
    serverRequests[expected.clientModuleHref] === 1 &&
    serverRequests[expected.clientWasmHref] === 1 &&
    serverRequests[expected.resumeRuntimeHref] === 1 &&
    relevantEvents.length === 0 &&
    site.errors.length === 0;
  const report = {
    schema: "pliegocss/pliegors-browser-gate/2",
    passed,
    browser: {
      executable: basename(chromePath),
      jsVersion: browserVersion.jsVersion,
      product: browserVersion.product,
      protocolVersion: browserVersion.protocolVersion,
      revision: browserVersion.revision,
      userAgent: browserVersion.userAgent,
    },
    expected,
    initial,
    final,
    relevantEvents,
    serverErrors: site.errors,
    serverRequests,
  };
  const compactReport = {
    schemaVersion: "1.0.0",
    passed,
    browser: {
      family: "chromium",
      executable: report.browser.executable,
      product: report.browser.product,
      revision: report.browser.revision,
      protocolVersion: report.browser.protocolVersion,
      jsVersion: report.browser.jsVersion,
      userAgent: report.browser.userAgent,
    },
    observation: {
      nodeIdentity: {
        document: final?.documentIdentity,
        island: final?.nodeIdentity?.island,
        button: final?.nodeIdentity?.button,
        value: final?.nodeIdentity?.value,
      },
      islandCount: final?.islandCount,
      islandId: final?.islandId,
      initialMinutes: initial?.initialMinutes,
      finalMinutes: final?.finalMinutes,
      increment: expected.increment,
      initialText: initial?.text,
      finalText: final?.text,
      initialClass: initial?.buttonClass,
      finalClass: final?.buttonClass,
      eventCount: final?.eventCount,
      eventKey: eventContract?.key,
      eventValue: eventContract?.value,
      moduleScriptCount: initial?.moduleScriptSources?.length,
      preloadEntryCount: initial?.preloadResourceEntries?.length,
      preloadRequestCount: serverRequests[expected.stylesheetPreloadHref] ?? 0,
      clientRequestCount: [
        expected.clientBootstrapHref,
        expected.clientModuleHref,
        expected.clientWasmHref,
        expected.resumeRuntimeHref,
      ].reduce((count, path) => count + (serverRequests[path] ?? 0), 0),
      stylesheetPresent: initial?.stylesheetHrefs?.includes(
        expected.stylesheetPreloadHref,
      ),
      wasmReady: initial?.wasmReady === "ready",
      relevantEventCount: relevantEvents.length,
      serverErrorCount: site.errors.length,
    },
  };
  if (agentReport) {
    console.log(JSON.stringify(compactReport, null, 2));
  }
  if (!passed) {
    if (agentReport) {
      process.exitCode = 1;
    } else {
      fail("PliegoRS browser gate failed", report);
    }
  }
  if (!agentReport) {
    console.log(JSON.stringify(report, null, 2));
  }
} finally {
  if (cdp) {
    try {
      await cdp.send("Browser.close");
    } catch {
      // The browser may already be closing after a protocol or page failure.
    }
    cdp.socket.close();
  }
  if (chrome && !(await waitForChildExit(chrome, 5000))) {
    chrome.kill();
    await waitForChildExit(chrome, 5000);
  }
  await closeServer(site.server);
  rmSync(profilePath, {
    force: true,
    maxRetries: 20,
    recursive: true,
    retryDelay: 100,
  });
  if (!keepSite) {
    rmSync(siteRoot, { force: true, recursive: true });
  }
}
