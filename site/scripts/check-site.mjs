// SPDX-License-Identifier: Apache-2.0

import { spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { createServer } from "node:http";
import {
  existsSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { extname, join, relative, resolve } from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { gzipSync } from "node:zlib";

const siteRoot = resolve(import.meta.dirname, "..");
const root = resolve(siteRoot, "..");
const output = join(siteRoot, "target", "site");
function findBrowser() {
  if (process.env.PLIEGOCSS_CHROME_PATH) {
    return process.env.PLIEGOCSS_CHROME_PATH;
  }
  if (process.platform === "win32") {
    return "C:/Program Files/Google/Chrome/Application/chrome.exe";
  }
  for (const name of ["google-chrome", "chromium", "chromium-browser"]) {
    const result = spawnSync("which", [name], {
      encoding: "utf8",
      windowsHide: true,
    });
    if (result.status === 0) return result.stdout.trim();
  }
  return undefined;
}

const browser = findBrowser();

function fail(message) {
  throw new Error(`site contract: ${message}`);
}

function filesBelow(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    return entry.isDirectory() ? filesBelow(path) : [path];
  });
}

function digest(directory) {
  const hash = createHash("sha256");
  for (const path of filesBelow(directory).sort()) {
    const name = relative(directory, path).replaceAll("\\", "/");
    hash.update(name).update("\0").update(readFileSync(path)).update("\0");
  }
  return hash.digest("hex");
}

function build() {
  const result = spawnSync(process.execPath, [join(siteRoot, "scripts", "build.mjs")], {
    cwd: root,
    encoding: "utf8",
    stdio: "inherit",
    windowsHide: true,
  });
  if (result.error) throw result.error;
  if (result.status !== 0) fail(`build exited with ${result.status}`);
}

function staticContract() {
  const required = [
    "index.html",
    "docs/index.html",
    "docs/getting-started/index.html",
    "docs/typed-styles/index.html",
    "docs/audit-and-transform/index.html",
    "docs/evidence/index.html",
    "docs/migration/index.html",
    "docs/integrations/pliegors/index.html",
    "docs/tooling/repair/index.html",
    "docs/reference/diagnostics/index.html",
    "docs/release-readiness/index.html",
    "docs/utilities/index.html",
    "playground/index.html",
    "examples/index.html",
    "benchmarks/index.html",
    "brand/index.html",
    "changelog/index.html",
    "security/index.html",
    "legal/index.html",
    "legal/terms/index.html",
    "legal/privacy/index.html",
    "legal/cookies/index.html",
    "legal/acceptable-use/index.html",
    "accessibility/index.html",
    "es/index.html",
    "es/docs/index.html",
    "es/docs/getting-started/index.html",
    "es/docs/release-readiness/index.html",
    "es/docs/utilities/index.html",
    "es/playground/index.html",
    "es/examples/index.html",
    "es/brand/index.html",
    "es/security/index.html",
    "es/legal/index.html",
    "es/legal/terms/index.html",
    "es/legal/privacy/index.html",
    "es/legal/cookies/index.html",
    "es/legal/acceptable-use/index.html",
    "es/accessibility/index.html",
    "404.html",
    "assets/site.css",
    "assets/pliegocss.css",
    "assets/laboratory.css",
    "assets/site.js",
    "assets/chunks",
    "assets/laboratory.json",
    "assets/catalog.json",
    "assets/i18n-es.json",
    "media/brand/cascade-chamber.avif",
    "media/brand/cascade-chamber.webp",
    "media/brand/cascade-chamber.png",
    "media/brand/semantic-fold.avif",
    "media/brand/evidence-archive.webp",
    "media/brand/material-study-carbon.png",
    "media/brand/material-study-paper.png",
    "media/brand/material-study-cobalt.png",
    "pliego.build.json",
    "pliego.graph.json",
    "_headers",
    "sitemap.xml",
  ];
  for (const path of required) {
    if (!existsSync(join(output, ...path.split("/")))) fail(`missing ${path}`);
  }
  const clientChunks = readdirSync(join(output, "assets", "chunks")).filter(
    (path) => path.endsWith(".js"),
  );
  if (clientChunks.length !== 1) {
    fail(`expected one lazy client chunk, found ${clientChunks.length}`);
  }
  const siteClient = readFileSync(join(output, "assets", "site.js"), "utf8");
  if (!siteClient.includes(`./chunks/${clientChunks[0]}`)) {
    fail("site client does not reference its published lazy chunk");
  }
  const assetBudgets = [
    ["assets/site.js", 64 * 1024],
    ["assets/site.css", 24 * 1024],
    ["assets/laboratory.css", 12 * 1024],
    ["assets/pliegocss.css", 2 * 1024],
  ];
  for (const [path, maximumGzipBytes] of assetBudgets) {
    const bytes = readFileSync(join(output, ...path.split("/")));
    const gzipBytes = gzipSync(bytes, { level: 9 }).length;
    if (gzipBytes > maximumGzipBytes) {
      fail(`${path} is ${gzipBytes} gzip bytes; budget is ${maximumGzipBytes}`);
    }
  }
  const home = readFileSync(join(output, "index.html"), "utf8");
  for (const marker of [
    'name="generator" content="PliegoRS 0.0.2"',
    'href="/assets/pliegocss.css"',
    'src="/assets/site.js"',
    "data-theatre",
    "data-hero-canvas",
    "PUBLIC PREVIEW / MEDELLÍN / 2026",
    "Celiums Solutions LLC",
    "Made in Medellín · Worldwide",
    'href="/legal/terms/"',
  ]) {
    if (!home.includes(marker)) fail(`home is missing ${marker}`);
  }
  const headers = readFileSync(join(output, "_headers"), "utf8");
  for (const marker of [
    "Content-Security-Policy:",
    "frame-ancestors 'none'",
    "script-src 'self' 'sha256-P+hnoNLWrWxk0w21ITjEpd7UnKmafsG79zKxPk/e3t4='",
    "Strict-Transport-Security: max-age=63072000; includeSubDomains; preload",
    "X-Content-Type-Options: nosniff",
    "/assets/*",
    "max-age=31536000, immutable",
  ]) {
    if (!headers.includes(marker)) fail(`deployment headers are missing ${marker}`);
  }
  const spanishHome = readFileSync(join(output, "es", "index.html"), "utf8");
  for (const marker of [
    '<html lang="es"',
    'hreflang="en"',
    'href="https://pliegocss.dev/"',
    'href="/es/legal/terms/"',
    "VISTA PREVIA PÚBLICA / MEDELLÍN / 2026",
    "Hecho en Medellín · Para el mundo",
  ]) {
    if (!spanishHome.includes(marker)) fail(`Spanish home is missing ${marker}`);
  }
  const englishTerms = readFileSync(
    join(output, "legal", "terms", "index.html"),
    "utf8",
  );
  const spanishTerms = readFileSync(
    join(output, "es", "legal", "terms", "index.html"),
    "utf8",
  );
  if (
    !englishTerms.includes("public-preview candidate software") ||
    !spanishTerms.includes("software candidato de vista previa pública") ||
    !spanishTerms.includes('<html lang="es"')
  ) {
    fail("legal language or public-preview status drifted");
  }
  const sitemap = readFileSync(join(output, "sitemap.xml"), "utf8");
  const sitemapUrls = sitemap.match(/<url>/gu)?.length ?? 0;
  if (
    sitemapUrls !== 90 ||
    !sitemap.includes('xmlns:xhtml="http://www.w3.org/1999/xhtml"') ||
    !sitemap.includes("https://pliegocss.dev/es/legal/privacy/")
  ) {
    fail(`bilingual sitemap drifted (${sitemapUrls} URLs)`);
  }
  const readiness = JSON.parse(
    readFileSync(join(root, "docs", "product", "release-readiness-0.1.0.json"), "utf8"),
  );
  const generatedDocs = JSON.parse(
    readFileSync(join(siteRoot, "src", "docs.generated.json"), "utf8"),
  );
  const readinessDocument = generatedDocs.documents.find(
    (document) => document.route === "/docs/release-readiness/",
  );
  const pliegorsDocument = generatedDocs.documents.find(
    (document) => document.route === "/docs/integrations/pliegors/",
  );
  const pliegorsHtml = readFileSync(
    join(output, "docs", "integrations", "pliegors", "index.html"),
    "utf8",
  );
  if (
    !pliegorsDocument ||
    !pliegorsHtml.includes(pliegorsDocument.sourcePath) ||
    !pliegorsHtml.includes(pliegorsDocument.sourceSha256) ||
    !pliegorsHtml.includes("abb8653e75da4a5cb4dd9b51200114fbb1e760c7") ||
    !pliegorsHtml.includes("0.4.0-beta.1")
  ) {
    fail("PliegoRS integration route is not bound to the current source-pinned contract");
  }
  const readinessHtml = readFileSync(
    join(output, "docs", "release-readiness", "index.html"),
    "utf8",
  );
  if (
    !readinessDocument ||
    !readinessHtml.includes(readiness.source.commit) ||
    !readinessHtml.includes(readiness.source.gitTree) ||
    !readinessHtml.includes(readinessDocument.sourcePath) ||
    !readinessHtml.includes(readinessDocument.sourceSha256) ||
    !readinessHtml.includes("Current blockers")
  ) {
    fail("release readiness route is not bound to the generated Markdown authority");
  }
  const privacyHtml = readFileSync(
    join(output, "legal", "privacy", "index.html"),
    "utf8",
  );
  const spanishPrivacyHtml = readFileSync(
    join(output, "es", "legal", "privacy", "index.html"),
    "utf8",
  );
  if (
    !privacyHtml.includes("Requests and contact") ||
    !spanishPrivacyHtml.includes("Solicitudes y contacto") ||
    !spanishPrivacyHtml.includes('<html lang="es"')
  ) {
    fail("privacy contact boundary is not bilingual");
  }
  const laboratory = JSON.parse(
    readFileSync(join(output, "assets", "laboratory.json")),
  );
  if (
    laboratory.schemaVersion !== 1 ||
    laboratory.recipes.length !== 144 ||
    laboratory.explanations.length !== 3 ||
    laboratory.conflicts.length !== 3 ||
    laboratory.recipes.some(
      (recipe) =>
        !/^pc_[a-z0-9]+$/u.test(recipe.className) ||
        !/^[a-f0-9]{32}$/u.test(recipe.styleId) ||
        !recipe.css.includes(`.${recipe.className}{`),
    )
  ) {
    fail("laboratory evidence drifted");
  }
  const laboratoryCss = readFileSync(join(output, "assets", "laboratory.css"));
  if (
    laboratory.cssBytes !== laboratoryCss.length ||
    laboratory.cssSha256 !==
      createHash("sha256").update(laboratoryCss).digest("hex")
  ) {
    fail("laboratory CSS binding drifted");
  }
  const catalog = JSON.parse(readFileSync(join(output, "assets", "catalog.json")));
  if (!Array.isArray(catalog.utilities) || catalog.utilities.length < 60) {
    fail("generated utility catalog drifted");
  }
  const ledger = JSON.parse(readFileSync(join(output, "pliego.build.json")));
  const ledgerFiles = ledger.receipt?.outputs?.files;
  if (
    ledger.reportVersion !== "2.0.0" ||
    !Array.isArray(ledgerFiles) ||
    ledger.receipt?.context?.ownership?.projectId !== "pliegocss-site"
  ) {
    fail("PliegoRS build ledger drifted");
  }
  for (const file of ledgerFiles) {
    const bytes = readFileSync(join(output, ...file.path.split("/")));
    const hash = createHash("sha256").update(bytes).digest("hex");
    if (file.bytes !== bytes.length || file.sha256 !== hash) {
      fail(`ledger integrity failed for ${file.path}`);
    }
  }
}

function contentType(path) {
  return (
    new Map([
      [".css", "text/css; charset=utf-8"],
      [".html", "text/html; charset=utf-8"],
      [".js", "text/javascript; charset=utf-8"],
      [".json", "application/json; charset=utf-8"],
      [".avif", "image/avif"],
      [".png", "image/png"],
      [".svg", "image/svg+xml"],
      [".webp", "image/webp"],
      [".woff2", "font/woff2"],
      [".xml", "application/xml; charset=utf-8"],
    ]).get(extname(path)) ?? "application/octet-stream"
  );
}

async function browserContract() {
  if (!browser || !existsSync(browser)) {
    process.stdout.write("site browser contract: not-configured (Chrome unavailable)\n");
    return { status: "not-configured" };
  }
  const server = createServer((request, response) => {
    try {
      const requestUrl = new URL(request.url ?? "/", "http://127.0.0.1");
      const decoded = decodeURIComponent(requestUrl.pathname);
      if (decoded.includes("\0") || decoded.split("/").includes("..")) {
        response.writeHead(400).end();
        return;
      }
      let path = join(output, decoded.replace(/^\/+/u, ""));
      if (decoded.endsWith("/")) path = join(path, "index.html");
      if (!existsSync(path) || statSync(path).isDirectory()) {
        path = join(output, "404.html");
        response.statusCode = 404;
      }
      response.setHeader("content-type", contentType(path));
      response.end(readFileSync(path));
    } catch {
      response.writeHead(500).end();
    }
  });
  await new Promise((resolveListen) => server.listen(0, "127.0.0.1", resolveListen));
  const port = server.address().port;
  const profile = mkdtempSync(join(tmpdir(), "pliegocss-site-cdp-"));
  const debugPortFile = join(profile, "DevToolsActivePort");
  const chromeStderr = [];
  let childError;
  const child = spawn(
    browser,
    [
      "--headless=new",
      "--remote-debugging-port=0",
      `--user-data-dir=${profile}`,
      "--no-first-run",
      "--disable-default-apps",
      "--disable-dev-shm-usage",
      "about:blank",
    ],
    { stdio: ["ignore", "ignore", "pipe"], windowsHide: true },
  );
  child.stderr.on("data", (chunk) => {
    chromeStderr.push(chunk);
    if (chromeStderr.length > 32) chromeStderr.shift();
  });
  child.once("error", (error) => {
    childError = error;
  });
  try {
    let debugPort;
    for (let attempt = 0; attempt < 300; attempt += 1) {
      if (existsSync(debugPortFile)) {
        const candidate = Number.parseInt(
          readFileSync(debugPortFile, "utf8").split(/\r?\n/u)[0],
          10,
        );
        if (Number.isInteger(candidate) && candidate > 0 && candidate <= 65_535) {
          debugPort = candidate;
          break;
        }
      }
      if (childError || child.exitCode !== null) break;
      await delay(100);
    }
    if (!debugPort) {
      const stderr = Buffer.concat(chromeStderr).toString("utf8").trim().slice(-2_000);
      fail(
        `Chrome CDP bootstrap unavailable (browser=${browser}, exit=${
          child.exitCode ?? "running"
        }, error=${childError?.message ?? "none"}, stderr=${JSON.stringify(stderr)})`,
      );
    }
    let version;
    for (let attempt = 0; attempt < 100; attempt += 1) {
      try {
        version = await (
          await fetch(`http://127.0.0.1:${debugPort}/json/version`)
        ).json();
        break;
      } catch {
        await delay(100);
      }
    }
    if (!version) {
      fail(
        `Chrome CDP endpoint unavailable on allocated port ${debugPort} (exit=${
          child.exitCode ?? "running"
        })`,
      );
    }
    let target;
    for (let attempt = 0; attempt < 60; attempt += 1) {
      const targets = await (
        await fetch(`http://127.0.0.1:${debugPort}/json/list`)
      ).json();
      target = targets.find(
        (entry) => entry.type === "page",
      );
      if (target) break;
      await delay(100);
    }
    if (!target) fail("site page target unavailable");
    const ws = new WebSocket(target.webSocketDebuggerUrl);
    await new Promise((open, reject) => {
      ws.addEventListener("open", open, { once: true });
      ws.addEventListener("error", reject, { once: true });
    });
    let id = 0;
    const call = (method, params = {}) =>
      new Promise((resolveCall, reject) => {
        const current = ++id;
        const listener = (event) => {
          const message = JSON.parse(event.data);
          if (message.id !== current) return;
          ws.removeEventListener("message", listener);
          message.error
            ? reject(new Error(message.error.message))
            : resolveCall(message.result);
        };
        ws.addEventListener("message", listener);
        ws.send(JSON.stringify({ id: current, method, params }));
      });
    await call("Page.enable");
    const navigate = async (path) => {
      await call("Page.navigate", { url: `http://127.0.0.1:${port}${path}` });
      for (let attempt = 0; attempt < 80; attempt += 1) {
        const ready = await call("Runtime.evaluate", {
          returnByValue: true,
          expression: "document.readyState",
        });
        if (ready.result.value === "complete") break;
        await delay(50);
      }
      await delay(1200);
    };
    await navigate("/");
    const evaluation = await call("Runtime.evaluate", {
      returnByValue: true,
      awaitPromise: true,
      expression: `(async () => {
        const gap = document.querySelector('[data-lab-control="gap"] [data-lab-option="open"]');
        gap.click();
        await new Promise(resolve => setTimeout(resolve, 100));
        const preview = document.querySelector('[data-lab-preview]');
        const css = document.querySelector('[data-lab-css]').textContent;
        const hero = document.querySelector('.hero');
        const action = document.querySelector('.action-link');
        const canvas = document.querySelector('[data-hero-canvas]');
        const outputIntent = document.querySelector('[data-output-tab="intent"]');
        outputIntent.focus();
        outputIntent.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
        await new Promise(resolve => setTimeout(resolve, 50));
        const conflictPadding = document.querySelector('[data-conflict="padding"]');
        conflictPadding.focus();
        conflictPadding.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowRight', bubbles: true }));
        await new Promise(resolve => setTimeout(resolve, 50));
        return {
          title: document.title,
          recipeSelected: gap.getAttribute('aria-pressed'),
          cssBound: css.includes('bound corpus') && /^preview-card pc_[a-z0-9]+$/.test(preview.className),
          previewGap: getComputedStyle(preview).gap,
          heroDisplay: getComputedStyle(hero).display,
          actionDisplay: getComputedStyle(action).display,
          canvasReady: canvas.width > 0 && canvas.height > 0,
          hasThreeFallback: document.documentElement.classList.contains('no-webgl'),
          scriptCount: document.querySelectorAll('script[type="module"]').length,
          stylesheets: document.styleSheets.length,
          outputTab: document.querySelector('[data-output-tab][aria-selected="true"]').dataset.outputTab,
          outputPanel: document.querySelector('[data-lab-panel]:not([hidden])').dataset.labPanel,
          conflictTab: document.querySelector('[data-conflict][aria-selected="true"]').dataset.conflict,
          conflictStatus: document.querySelector('[data-conflict-status]').textContent
        };
      })()`,
    });
    const value = evaluation.result.value;
    if (
      value.title !== "PliegoCSS — Compile confidence into CSS" ||
      value.recipeSelected !== "true" ||
      !value.cssBound ||
      value.previewGap !== "24px" ||
      value.heroDisplay !== "grid" ||
      value.actionDisplay !== "flex" ||
      (!value.canvasReady && !value.hasThreeFallback) ||
      value.scriptCount !== 1 ||
      value.stylesheets !== 3 ||
      value.outputTab !== "css" ||
      value.outputPanel !== "css" ||
      value.conflictTab !== "condition-safe" ||
      value.conflictStatus !== "ACCEPTED"
    ) {
      fail(`computed browser contract mismatch: ${JSON.stringify(value)}`);
    }
    await navigate("/examples/");
    const examples = await call("Runtime.evaluate", {
      returnByValue: true,
      awaitPromise: true,
      expression: `(async () => {
        document.querySelector('[data-example-select="operations"]').click();
        document.querySelector('[data-ops-pulse]').click();
        await new Promise(resolve => setTimeout(resolve, 50));
        return {
          active: document.querySelector('[data-example-panel]:not([hidden])').dataset.examplePanel,
          state: document.querySelector('[data-example-state]').textContent,
          hasGeneratedClass: /^pc_[a-z0-9]+$/.test(document.querySelector('.example-inspector dd').textContent),
          cssBound: document.querySelector('[data-example-css]').textContent.includes('.pc_')
        };
      })()`,
    });
    if (
      examples.result.value.active !== "operations" ||
      examples.result.value.state !== "operations / pulse" ||
      !examples.result.value.hasGeneratedClass ||
      !examples.result.value.cssBound
    ) {
      fail(`examples browser contract mismatch: ${JSON.stringify(examples.result.value)}`);
    }
    await call("Emulation.setDeviceMetricsOverride", {
      width: 390,
      height: 844,
      deviceScaleFactor: 1,
      mobile: true,
    });
    await navigate("/examples/");
    const mobileExamples = await call("Runtime.evaluate", {
      returnByValue: true,
      awaitPromise: true,
      expression: `(async () => {
        document.querySelector('[data-example-select="operations"]').click();
        document.querySelector('[data-ops-view="policies"]').click();
        await new Promise(resolve => setTimeout(resolve, 50));
      const root = document.querySelector('[data-example-lab]');
      return {
          overflow: document.documentElement.scrollWidth > document.documentElement.clientWidth,
          active: document.querySelector('[data-example-panel]:not([hidden])').dataset.examplePanel,
          view: document.querySelector('.example-operations').dataset.opsView,
          title: document.querySelector('[data-ops-title]').textContent,
          metric: document.querySelector('.ops-metrics strong').textContent,
          event: document.querySelector('.ops-feed strong').textContent,
          announced: document.querySelector('[data-ops-announcer]').textContent,
          viewport: document.documentElement.clientWidth,
        rootVisible: getComputedStyle(root).display !== 'none'
      };
      })()`,
    });
    if (
      mobileExamples.result.value.overflow ||
      mobileExamples.result.value.active !== "operations" ||
      mobileExamples.result.value.view !== "policies" ||
      mobileExamples.result.value.title !== "Policy control" ||
      mobileExamples.result.value.metric !== "0" ||
      mobileExamples.result.value.event !== "licenses" ||
      mobileExamples.result.value.announced !== "Policies selected." ||
      mobileExamples.result.value.viewport !== 390 ||
      !mobileExamples.result.value.rootVisible
    ) {
      fail(
        `mobile examples browser contract mismatch: ${JSON.stringify(
          mobileExamples.result.value,
        )}`,
      );
    }
    await navigate("/");
    const mobileHome = await call("Runtime.evaluate", {
      returnByValue: true,
      expression: `(() => {
        const workbench = document.querySelector('.theatre-workbench');
        const actions = document.querySelector('.hero-actions');
        const terminal = document.querySelector('.hero-terminal');
        return {
          overflow: document.documentElement.scrollWidth > document.documentElement.clientWidth,
          headline: document.querySelector('.hero h1')?.innerText,
          actionsOpacity: Number.parseFloat(getComputedStyle(actions).opacity),
          terminalOpacity: Number.parseFloat(getComputedStyle(terminal).opacity),
          workbenchOpacity: Number.parseFloat(getComputedStyle(workbench).opacity),
          workbenchTransform: getComputedStyle(workbench).transform,
          workbenchHeight: workbench.getBoundingClientRect().height
        };
      })()`,
    });
    if (
      mobileHome.result.value.overflow ||
      mobileHome.result.value.headline !== "CSS you can\nprove." ||
      mobileHome.result.value.actionsOpacity < 0.99 ||
      mobileHome.result.value.terminalOpacity < 0.99 ||
      mobileHome.result.value.workbenchOpacity < 0.99 ||
      mobileHome.result.value.workbenchTransform !== "none" ||
      mobileHome.result.value.workbenchHeight < 1_000
    ) {
      fail(
        `mobile home browser contract mismatch: ${JSON.stringify(
          mobileHome.result.value,
        )}`,
      );
    }
    await call("Emulation.setEmulatedMedia", {
      media: "",
      features: [{ name: "prefers-reduced-motion", value: "reduce" }],
    });
    await navigate("/");
    const reducedMotionContract = await call("Runtime.evaluate", {
      returnByValue: true,
      expression: `(() => {
        const canvas = document.querySelector('[data-hero-canvas]');
        return {
          classApplied: document.documentElement.classList.contains('reduced-motion'),
          canvasDisplay: getComputedStyle(canvas).display,
          overflow: document.documentElement.scrollWidth > document.documentElement.clientWidth,
          theatreVisible: getComputedStyle(document.querySelector('[data-theatre]')).display !== 'none'
        };
      })()`,
    });
    if (
      !reducedMotionContract.result.value.classApplied ||
      reducedMotionContract.result.value.canvasDisplay !== "none" ||
      reducedMotionContract.result.value.overflow ||
      !reducedMotionContract.result.value.theatreVisible
    ) {
      fail(
        `reduced-motion browser contract mismatch: ${JSON.stringify(
          reducedMotionContract.result.value,
        )}`,
      );
    }
    await call("Emulation.clearDeviceMetricsOverride");
    await call("Emulation.setEmulatedMedia", { media: "", features: [] });
    await navigate("/es/");
    const spanish = await call("Runtime.evaluate", {
      returnByValue: true,
      awaitPromise: true,
      expression: `(async () => {
        const search = document.querySelector('[data-command-open]');
        search.click();
        await new Promise(resolve => setTimeout(resolve, 100));
        const input = document.querySelector('[data-command-search]');
        input.value = 'privacidad';
        input.dispatchEvent(new Event('input', { bubbles: true }));
        await new Promise(resolve => setTimeout(resolve, 100));
        const first = document.querySelector('.command-result');
        return {
          lang: document.documentElement.lang,
          title: document.title,
          preview: document.querySelector('.preview-rail strong')?.textContent,
          footer: document.querySelector('.footer-bottom span:nth-child(2)')?.textContent,
          termsHref: document.querySelector('.footer-directory a[href*="legal/terms"]')?.getAttribute('href'),
          brandHref: [...document.querySelectorAll('.footer-directory a')].find(link => link.textContent === 'Marca')?.getAttribute('href'),
          paletteHref: first?.getAttribute('href'),
          paletteText: first?.textContent,
          overflow: document.documentElement.scrollWidth > document.documentElement.clientWidth
        };
      })()`,
    });
    const spanishValue = spanish.result.value;
    if (
      spanishValue.lang !== "es" ||
      spanishValue.title !== "PliegoCSS — Compila confianza en CSS" ||
      spanishValue.preview !== "VISTA PREVIA PÚBLICA / MEDELLÍN / 2026" ||
      spanishValue.footer !== "Hecho en Medellín · Para el mundo" ||
      spanishValue.termsHref !== "/es/legal/terms/" ||
      spanishValue.brandHref !== "/es/brand/" ||
      !spanishValue.paletteHref?.startsWith("/es/legal/") ||
      !spanishValue.paletteText?.includes("legal") ||
      spanishValue.overflow
    ) {
      fail(`Spanish browser contract mismatch: ${JSON.stringify(spanishValue)}`);
    }
    await navigate("/es/legal/terms/");
    const spanishLegal = await call("Runtime.evaluate", {
      returnByValue: true,
      expression: `(() => ({
        lang: document.documentElement.lang,
        heading: document.querySelector('h1')?.textContent,
        publicPreview: document.querySelector('.legal-document__body p')?.textContent.includes('vista previa pública'),
        englishHref: document.querySelector('.language-switcher a[lang="en"]')?.getAttribute('href'),
        overflow: document.documentElement.scrollWidth > document.documentElement.clientWidth
      }))()`,
    });
    if (
      spanishLegal.result.value.lang !== "es" ||
      spanishLegal.result.value.heading !== "Términos" ||
      !spanishLegal.result.value.publicPreview ||
      spanishLegal.result.value.englishHref !== "/legal/terms/" ||
      spanishLegal.result.value.overflow
    ) {
      fail(
        `Spanish legal browser contract mismatch: ${JSON.stringify(
          spanishLegal.result.value,
        )}`,
      );
    }
    await navigate("/docs/");
    const docs = await call("Runtime.evaluate", {
      returnByValue: true,
      awaitPromise: true,
      expression: `(() => {
        const input = document.querySelector('[data-doc-search]');
        input.value = 'repair';
        input.dispatchEvent(new Event('input'));
        return {
          items: document.querySelectorAll('[data-doc-search-item]').length,
          groups: document.querySelectorAll('[data-doc-search-group]').length,
          visible: [...document.querySelectorAll('[data-doc-search-item]')].filter(item => !item.hidden).length,
          visibleGroups: [...document.querySelectorAll('[data-doc-search-group]')].filter(item => !item.hidden).length
        };
      })()`,
    });
    if (
      docs.result.value.items < 30 ||
      docs.result.value.groups !== 8 ||
      docs.result.value.visible !== 1 ||
      docs.result.value.visibleGroups !== 1
    ) {
      fail(`docs browser contract mismatch: ${JSON.stringify(docs.result.value)}`);
    }
    ws.close();
    return {
      status: "passed",
      browser: version.Browser,
      computed: value,
      examples: examples.result.value,
      mobileExamples: mobileExamples.result.value,
      mobileHome: mobileHome.result.value,
      reducedMotion: reducedMotionContract.result.value,
      spanish: spanishValue,
      spanishLegal: spanishLegal.result.value,
      docs: docs.result.value,
    };
  } finally {
    child.kill();
    server.close();
    for (let attempt = 0; attempt < 20; attempt += 1) {
      try {
        rmSync(profile, { recursive: true, force: true, maxRetries: 2, retryDelay: 100 });
        break;
      } catch (error) {
        if (attempt === 19) throw error;
        await delay(100);
      }
    }
  }
}

build();
staticContract();
const firstDigest = digest(output);
build();
const secondDigest = digest(output);
if (firstDigest !== secondDigest) fail("identical builds produced different site bytes");
const browserResult = await browserContract();
process.stdout.write(
  `${JSON.stringify(
    {
      schemaVersion: 1,
      result: browserResult.status === "passed" ? "passed" : "blocked",
      deterministicSha256: secondDigest,
      files: filesBelow(output).length,
      browser: browserResult,
    },
    null,
    2,
  )}\n`,
);
if (browserResult.status !== "passed" && process.env.CI) process.exitCode = 1;
