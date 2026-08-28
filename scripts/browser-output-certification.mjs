import { spawnSync } from "node:child_process";
import {
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { basename, dirname, isAbsolute, join, relative, resolve, sep } from "node:path";
import pixelmatch from "pixelmatch";
import { PNG } from "pngjs";
import { chromium, firefox, webkit } from "playwright";
import {
  authorityPath as competitorAuthorityPath,
  extractClassValues,
  loadAuthority as loadCompetitorAuthority,
  readInstalledPackage,
  repositoryRoot,
  resolveLaneCli,
  sha256,
} from "./benchmark-authority-v2.mjs";
import { targetDirectoryForRustcIdentity } from "./rust-target.mjs";

const authorityPath = resolve(
  repositoryRoot,
  "benchmarks",
  "browser-output-certification-v1",
  "authority.json",
);
const authority = JSON.parse(readFileSync(authorityPath, "utf8"));
const browserTypes = { chromium, firefox, webkit };

function fail(message) {
  throw new Error(`Browser/output certification: ${message}`);
}

function parseOptions(arguments_) {
  const options = {
    browser: null,
    host: null,
    output: null,
    requireClean: false,
  };
  for (const argument of arguments_) {
    if (argument === "--") continue;
    if (argument === "--require-clean") options.requireClean = true;
    else if (argument.startsWith("--browser=")) options.browser = argument.slice(10);
    else if (argument.startsWith("--host=")) options.host = argument.slice(7);
    else if (argument.startsWith("--output=")) {
      options.output = resolve(repositoryRoot, argument.slice(9));
    } else {
      fail(
        "usage: node scripts/browser-output-certification.mjs --browser=<chromium|firefox|webkit> [--host=<id>] [--output=<path>] [--require-clean]",
      );
    }
  }
  if (!authority.browsers.includes(options.browser)) fail("a supported --browser is required");
  const candidates = authority.hosts.filter(
    (host) =>
      host.browser === options.browser &&
      host.os === process.platform &&
      host.arch === process.arch,
  );
  const host = options.host
    ? authority.hosts.find((entry) => entry.id === options.host)
    : candidates[0];
  if (!host) fail("the current OS/architecture/browser has no authority host");
  if (
    host.browser !== options.browser ||
    host.os !== process.platform ||
    host.arch !== process.arch
  ) {
    fail(
      `host ${host.id} requires ${host.os}/${host.arch}/${host.browser}, found ${process.platform}/${process.arch}/${options.browser}`,
    );
  }
  options.host = host;
  options.output ??= resolve(
    repositoryRoot,
    "target",
    "browser-output-certification",
    `${host.id}.local.json`,
  );
  const child = relative(repositoryRoot, options.output);
  if (
    !child ||
    child === ".." ||
    child.startsWith(`..${sep}`) ||
    isAbsolute(child) ||
    !options.output.endsWith(".json")
  ) {
    fail("--output must be a .json path inside the repository");
  }
  return options;
}

function run(command, arguments_, options = {}) {
  const result = spawnSync(command, arguments_, {
    cwd: repositoryRoot,
    encoding: "utf8",
    windowsHide: true,
    timeout: 600_000,
    maxBuffer: 32 * 1024 * 1024,
    ...options,
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    fail(
      `${command} ${arguments_.join(" ")} exited ${result.status}\n${`${result.stdout ?? ""}${result.stderr ?? ""}`.slice(-8_192)}`,
    );
  }
  return result;
}

function git(arguments_) {
  return run("git", arguments_).stdout;
}

function gitState() {
  const status = git(["status", "--porcelain=v1", "-z", "--untracked-files=all"]);
  return {
    commit: git(["rev-parse", "HEAD"]).trim(),
    gitTree: git(["rev-parse", "HEAD^{tree}"]).trim(),
    dirty: status.length > 0,
    statusEntryCount: status.split("\0").filter(Boolean).length,
    statusSha256: sha256(Buffer.from(status, "utf8")),
  };
}

function instant(date = new Date()) {
  return date.toISOString().replace(/\.\d{3}Z$/u, "Z");
}

function prepareCompiler() {
  const rustcVerbose = run("rustc", ["+1.96.0", "-vV"]).stdout.trim();
  const host = /^host: (\S+)$/mu.exec(rustcVerbose)?.[1];
  if (!host) fail("cannot resolve Rust host");
  const cargoTarget = targetDirectoryForRustcIdentity(
    repositoryRoot,
    process.env,
    rustcVerbose,
  );
  const environment = {
    ...process.env,
    CARGO_INCREMENTAL: "0",
    CARGO_TARGET_DIR: cargoTarget,
    CARGO_TERM_COLOR: "never",
  };
  run(
    "cargo",
    ["+1.96.0", "build", "--release", "--locked", "--target", host, "-p", "pliego-cssc"],
    { env: environment },
  );
  const executable = join(
    cargoTarget,
    host,
    "release",
    process.platform === "win32" ? "pliego-cssc.exe" : "pliego-cssc",
  );
  return {
    executable,
    sha256: sha256(readFileSync(executable)),
    rustc: /^rustc .+$/mu.exec(rustcVerbose)?.[0] ?? "unknown",
    host,
  };
}

function tailwindInput(packageAlias) {
  return `@layer theme, utilities;\n@import "${packageAlias}/theme.css" layer(theme);\n@import "${packageAlias}/utilities.css" layer(utilities) source(none);\n@source "./fixture.html";\n@theme {\n  --color-canvas: oklch(98.5% .004 80);\n  --color-surface: #fff;\n  --color-surface-raised: oklch(96% .006 80);\n  --color-ink: oklch(19% .012 70);\n  --color-muted: oklch(50% .014 70);\n  --color-accent: oklch(58% .19 32);\n  --color-accent-strong: oklch(50% .18 30);\n  --color-line: oklch(88% .008 75);\n  --font-sans: ui-sans-serif, system-ui, sans-serif;\n  --shadow-sm: 0 1px 2px #0000000d;\n}\n`;
}

function decorateFixture(html) {
  let index = 0;
  return html.replace(/class="([^"]*)"/gu, (_, classes) => {
    const id = `node-${String(index).padStart(2, "0")}`;
    index += 1;
    return `data-cert-node="${id}" class="${classes}"`;
  });
}

function rewritePliegoHtml(html, manifest) {
  const bySource = new Map();
  for (const style of manifest.styles ?? []) {
    for (const origin of style.origins ?? []) {
      const existing = bySource.get(origin.source);
      if (existing && existing !== style.className) {
        fail(`manifest maps ${JSON.stringify(origin.source)} to multiple classes`);
      }
      bySource.set(origin.source, style.className);
    }
  }
  let replacements = 0;
  const rewritten = html.replace(/class="([^"]*)"/gu, (_, source) => {
    const replacement = bySource.get(source);
    if (!replacement) fail(`manifest has no generated class for ${JSON.stringify(source)}`);
    replacements += 1;
    return `class="${replacement}"`;
  });
  if (replacements !== extractClassValues(html).length) fail("not every class attribute was rewritten");
  return rewritten;
}

function withoutStylesheetLink(html) {
  return html.replace(/\s*<link\s+rel="stylesheet"[^>]*>\s*/u, "\n");
}

function browserDocument(html, css, resetCss) {
  const deterministicCss = `\n*,:before,:after{animation-duration:0s!important;animation-delay:0s!important;transition-duration:0s!important;caret-color:transparent!important}\n`;
  const style = `<style data-certification-css>${resetCss}${css}${deterministicCss}</style>`;
  return withoutStylesheetLink(html).replace("</head>", `${style}</head>`);
}

function prepareOutputs(compiler) {
  const competitor = loadCompetitorAuthority();
  const lane = competitor.lanes.find((entry) => entry.id === authority.competitorLane);
  if (!lane) fail(`competitor lane ${authority.competitorLane} is missing`);
  if (Date.now() > Date.parse(competitor.expiresAtUtc)) fail("competitor oracle is expired");
  const fixturePath = resolve(repositoryRoot, authority.fixture.path);
  const fixtureBytes = readFileSync(fixturePath);
  if (sha256(fixtureBytes) !== authority.fixture.sha256) fail("fixture hash drifted");
  const resetPath = resolve(repositoryRoot, authority.reset.path);
  const resetBytes = readFileSync(resetPath);
  if (sha256(resetBytes) !== authority.reset.sha256) fail("shared reset hash drifted");

  const workRoot = resolve(
    repositoryRoot,
    "target",
    "browser-output-certification",
    `work-${process.pid}-${Date.now()}`,
  );
  mkdirSync(workRoot, { recursive: true });
  const fixture = decorateFixture(fixtureBytes.toString("utf8"));
  const values = [...new Set(extractClassValues(fixture))];
  const fixtureOutput = join(workRoot, "fixture.html");
  const stylesOutput = join(workRoot, "styles.txt");
  const inputOutput = join(workRoot, "input.css");
  const tailwindOutput = join(workRoot, "tailwind.css");
  const pliegoOutput = join(workRoot, "pliego.css");
  const manifestOutput = join(workRoot, "pliego.manifest.json");
  writeFileSync(fixtureOutput, fixture);
  writeFileSync(stylesOutput, `${values.join("\n")}\n`);
  writeFileSync(inputOutput, tailwindInput(lane.packageAlias));
  run(
    process.execPath,
    [resolveLaneCli(lane), "-i", inputOutput, "-o", tailwindOutput, "--minify"],
    { cwd: workRoot },
  );
  run(
    compiler.executable,
    [
      "compile",
      "--input",
      stylesOutput,
      "--seed",
      "--theme",
      "--output",
      pliegoOutput,
      "--manifest",
      manifestOutput,
    ],
    { cwd: workRoot },
  );
  const manifest = JSON.parse(readFileSync(manifestOutput, "utf8"));
  const pliegoHtml = rewritePliegoHtml(fixture, manifest);
  const tailwindCss = readFileSync(tailwindOutput, "utf8");
  const pliegoCss = readFileSync(pliegoOutput, "utf8");
  return {
    workRoot,
    lane,
    resetCss: resetBytes.toString("utf8"),
    html: { pliego: pliegoHtml, tailwind: fixture },
    css: { pliego: pliegoCss, tailwind: tailwindCss },
    provenance: {
      competitorAuthoritySha256: sha256(readFileSync(competitorAuthorityPath)),
      competitorObservedAtUtc: competitor.observedAtUtc,
      competitorExpiresAtUtc: competitor.expiresAtUtc,
      tailwindVersion: lane.version,
      tailwindCliVersion: lane.cliVersion,
      tailwindPackageManifestSha256: sha256(
        readFileSync(readInstalledPackage(lane.packageAlias).path),
      ),
      tailwindCliManifestSha256: sha256(
        readFileSync(readInstalledPackage(lane.cliPackageAlias).path),
      ),
      fixtureSha256: authority.fixture.sha256,
      resetSha256: authority.reset.sha256,
      pliegoCssSha256: sha256(pliegoCss),
      tailwindCssSha256: sha256(tailwindCss),
    },
  };
}

async function capture(page, properties) {
  return page.evaluate(({ propertyNames, lengthQuantum, geometryResolution }) => {
    const colorProperties = new Set([
      "color",
      "background-color",
      "border-top-color",
      "outline-color",
    ]);
    const canvas = document.createElement("canvas");
    canvas.width = 1;
    canvas.height = 1;
    const context = canvas.getContext("2d", { willReadFrequently: true });
    const normalizeColor = (value) => {
      context.clearRect(0, 0, 1, 1);
      context.fillStyle = "rgba(0,0,0,0)";
      context.fillStyle = value;
      context.fillRect(0, 0, 1, 1);
      return [...context.getImageData(0, 0, 1, 1).data].join(",");
    };
    const normalizeLengths = (value) =>
      value.replace(/-?(?:\d+\.?\d*|\.\d+)px/gu, (token) => {
        const number = Number.parseFloat(token);
        const rounded = Math.round(number / lengthQuantum) * lengthQuantum;
        return `${Object.is(rounded, -0) ? 0 : Number(rounded.toFixed(4))}px`;
      });
    const quantize = (value) => {
      const rounded = Math.round(value / geometryResolution) * geometryResolution;
      return Object.is(rounded, -0) ? 0 : Number(rounded.toFixed(6));
    };
    const rectangle = (value) => ({
      x: quantize(value.x),
      y: quantize(value.y),
      width: quantize(value.width),
      height: quantize(value.height),
    });
    const directTextRectangles = (element) => {
      const rectangles = [];
      for (const node of element.childNodes) {
        if (node.nodeType !== Node.TEXT_NODE || !node.textContent?.trim()) continue;
        const range = document.createRange();
        range.selectNodeContents(node);
        rectangles.push(...[...range.getClientRects()].map(rectangle));
        range.detach();
      }
      return rectangles;
    };
    const nodes = {};
    for (const element of document.querySelectorAll("[data-cert-node]")) {
      const id = element.getAttribute("data-cert-node");
      const style = getComputedStyle(element);
      const computed = {};
      for (const property of propertyNames) {
        let value = style.getPropertyValue(property).trim();
        if (colorProperties.has(property)) value = normalizeColor(value);
        if (property === "justify-content" && value === "end") value = "flex-end";
        value = normalizeLengths(value);
        computed[property] = value;
      }
      const record = {
        tag: element.tagName.toLowerCase(),
        computed,
        geometry: {
          borderBox: rectangle(element.getBoundingClientRect()),
          clientWidth: element.clientWidth,
          clientHeight: element.clientHeight,
          scrollWidth: element.scrollWidth,
          scrollHeight: element.scrollHeight,
          directTextRects: directTextRectangles(element),
        },
      };
      if (element.matches("input[placeholder], textarea[placeholder]")) {
        const placeholder = getComputedStyle(element, "::placeholder");
        record.placeholder = {
          color: normalizeColor(placeholder.getPropertyValue("color").trim()),
          opacity: placeholder.getPropertyValue("opacity").trim(),
        };
      }
      nodes[id] = record;
    }
    return {
      activeNode: document.activeElement?.getAttribute("data-cert-node") ?? null,
      documentElement: {
        backgroundColor: getComputedStyle(document.documentElement).backgroundColor,
        colorScheme: getComputedStyle(document.documentElement).colorScheme,
      },
      nodes,
    };
  }, {
    propertyNames: properties,
    lengthQuantum: authority.comparison.computedLengthQuantumCssPx,
    geometryResolution: authority.comparison.layoutGeometryResolutionCssPx,
  });
}

function compareScreenshots(pliegoBytes, tailwindBytes, diffPath) {
  const pliego = PNG.sync.read(pliegoBytes);
  const tailwind = PNG.sync.read(tailwindBytes);
  if (pliego.width !== tailwind.width || pliego.height !== tailwind.height) {
    return {
      passed: false,
      byteEqual: false,
      width: pliego.width,
      height: pliego.height,
      tailwindWidth: tailwind.width,
      tailwindHeight: tailwind.height,
      mismatchPixels: null,
      mismatchRatio: null,
      diff: null,
    };
  }
  const diff = new PNG({ width: pliego.width, height: pliego.height });
  const mismatchPixels = pixelmatch(
    pliego.data,
    tailwind.data,
    diff.data,
    pliego.width,
    pliego.height,
    {
      threshold: authority.comparison.screenshotPixelThreshold,
      includeAA: false,
      diffMask: true,
    },
  );
  const mismatchRatio = mismatchPixels / (pliego.width * pliego.height);
  const diffBytes = PNG.sync.write(diff);
  writeFileSync(diffPath, diffBytes);
  return {
    passed: mismatchRatio <= authority.comparison.maximumScreenshotMismatchRatio,
    byteEqual: pliegoBytes.equals(tailwindBytes),
    width: pliego.width,
    height: pliego.height,
    tailwindWidth: tailwind.width,
    tailwindHeight: tailwind.height,
    mismatchPixels,
    mismatchRatio: Number(mismatchRatio.toFixed(9)),
    diff: {
      path: relative(dirname(options.output), diffPath).replaceAll("\\", "/"),
      sha256: sha256(diffBytes),
      bytes: diffBytes.byteLength,
    },
  };
}

function computedDiff(pliego, tailwind) {
  const differences = [];
  const geometry = { maximumDeltaCssPx: 0 };
  const ids = [...new Set([...Object.keys(pliego.nodes), ...Object.keys(tailwind.nodes)])].sort();
  if (pliego.activeNode !== tailwind.activeNode) {
    differences.push({ path: "activeNode", pliego: pliego.activeNode, tailwind: tailwind.activeNode });
  }
  for (const id of ids) {
    const left = pliego.nodes[id];
    const right = tailwind.nodes[id];
    if (!left || !right) {
      differences.push({ path: id, pliego: left ?? null, tailwind: right ?? null });
      continue;
    }
    for (const property of authority.computedProperties) {
      if (left.computed[property] !== right.computed[property]) {
        differences.push({
          path: `${id}.${property}`,
          pliego: left.computed[property],
          tailwind: right.computed[property],
        });
      }
    }
    geometryDiff(left.geometry, right.geometry, `${id}.geometry`, differences, geometry);
    if (JSON.stringify(left.placeholder ?? null) !== JSON.stringify(right.placeholder ?? null)) {
      differences.push({
        path: `${id}.placeholder`,
        pliego: left.placeholder ?? null,
        tailwind: right.placeholder ?? null,
      });
    }
  }
  return {
    differences,
    maximumLayoutGeometryDeltaCssPx: Number(geometry.maximumDeltaCssPx.toFixed(6)),
  };
}

function geometryDiff(pliego, tailwind, path, differences, geometry) {
  if (typeof pliego === "number" && typeof tailwind === "number") {
    const delta = Math.abs(pliego - tailwind);
    geometry.maximumDeltaCssPx = Math.max(geometry.maximumDeltaCssPx, delta);
    if (delta > authority.comparison.maximumLayoutGeometryDeltaCssPx) {
      differences.push({ path, pliego, tailwind, delta: Number(delta.toFixed(6)) });
    }
    return;
  }
  if (Array.isArray(pliego) && Array.isArray(tailwind)) {
    if (pliego.length !== tailwind.length) {
      differences.push({ path: `${path}.length`, pliego: pliego.length, tailwind: tailwind.length });
      return;
    }
    for (let index = 0; index < pliego.length; index += 1) {
      geometryDiff(pliego[index], tailwind[index], `${path}[${index}]`, differences, geometry);
    }
    return;
  }
  if (
    pliego &&
    tailwind &&
    typeof pliego === "object" &&
    typeof tailwind === "object"
  ) {
    const keys = [...new Set([...Object.keys(pliego), ...Object.keys(tailwind)])].sort();
    for (const key of keys) {
      geometryDiff(pliego[key], tailwind[key], `${path}.${key}`, differences, geometry);
    }
    return;
  }
  if (pliego !== tailwind) differences.push({ path, pliego: pliego ?? null, tailwind: tailwind ?? null });
}

async function renderEngine(browserInstance, engine, prepared, mode, scenario, screenshotPath) {
  const context = await browserInstance.newContext({
    viewport: { width: scenario.width, height: scenario.height },
    deviceScaleFactor: 1,
    locale: "en-US",
    colorScheme: "light",
    reducedMotion: "reduce",
    forcedColors: "none",
  });
  const page = await context.newPage();
  try {
    const resetCss = mode.id === "shared-reset" ? prepared.resetCss : "";
    await page.setContent(
      browserDocument(prepared.html[engine], prepared.css[engine], resetCss),
      { waitUntil: "load" },
    );
    await page.evaluate(() => document.fonts.ready);
    if (scenario.action === "hover") await page.locator(scenario.selector).hover();
    else if (scenario.action === "focus") await page.locator(scenario.selector).focus();
    else if (scenario.action !== "none") fail(`unknown scenario action ${scenario.action}`);
    const computed = await capture(page, authority.computedProperties);
    const screenshot = await page.screenshot({
      path: screenshotPath,
      fullPage: true,
      animations: "disabled",
      caret: "hide",
      scale: "css",
    });
    return { computed, screenshot };
  } finally {
    await context.close();
  }
}

const options = parseOptions(process.argv.slice(2));
const state = gitState();
if (options.requireClean && state.dirty) fail("--require-clean rejects a dirty Git worktree");
const generatedAt = new Date();
const expiresAt = new Date(
  generatedAt.getTime() + authority.maximumEvidenceAgeHours * 60 * 60 * 1_000,
);
const compiler = prepareCompiler();
const prepared = prepareOutputs(compiler);
const outputDirectory = dirname(options.output);
mkdirSync(outputDirectory, { recursive: true });
const screenshotRoot = resolve(
  outputDirectory,
  `${basename(options.output, ".json")}.screenshots`,
);
const screenshotChild = relative(outputDirectory, screenshotRoot);
if (!screenshotChild || screenshotChild.startsWith("..") || isAbsolute(screenshotChild)) {
  fail("unsafe screenshot output path");
}
rmSync(screenshotRoot, { recursive: true, force: true });
mkdirSync(screenshotRoot, { recursive: true });

const playwrightPackage = JSON.parse(
  readFileSync(resolve(repositoryRoot, "node_modules", "playwright", "package.json"), "utf8"),
);
const browserInstance = await browserTypes[options.browser].launch({ headless: true });
const observations = [];
let computedMismatchCount = 0;
let screenshotMismatchCount = 0;
let maximumLayoutGeometryDeltaCssPx = 0;
try {
  for (const mode of authority.resetModes) {
    for (const scenario of authority.scenarios) {
      process.stdout.write(`[browser-output] ${options.host.id}/${mode.id}/${scenario.id}\n`);
      const scenarioRoot = resolve(screenshotRoot, mode.id, scenario.id);
      mkdirSync(scenarioRoot, { recursive: true });
      const paths = {
        pliego: resolve(scenarioRoot, "pliego.png"),
        tailwind: resolve(scenarioRoot, "tailwind.png"),
        diff: resolve(scenarioRoot, "diff.png"),
      };
      const pliego = await renderEngine(
        browserInstance,
        "pliego",
        prepared,
        mode,
        scenario,
        paths.pliego,
      );
      const tailwind = await renderEngine(
        browserInstance,
        "tailwind",
        prepared,
        mode,
        scenario,
        paths.tailwind,
      );
      const computedComparison = computedDiff(pliego.computed, tailwind.computed);
      const { differences } = computedComparison;
      const screenshotComparison = compareScreenshots(
        pliego.screenshot,
        tailwind.screenshot,
        paths.diff,
      );
      computedMismatchCount += differences.length;
      maximumLayoutGeometryDeltaCssPx = Math.max(
        maximumLayoutGeometryDeltaCssPx,
        computedComparison.maximumLayoutGeometryDeltaCssPx,
      );
      if (!screenshotComparison.passed) screenshotMismatchCount += 1;
      observations.push({
        resetMode: mode.id,
        scenario: scenario.id,
        viewport: { width: scenario.width, height: scenario.height },
        action: scenario.action,
        computed: {
          equal: differences.length === 0,
          differenceCount: differences.length,
          differences: differences.slice(0, 100),
          maximumLayoutGeometryDeltaCssPx:
            computedComparison.maximumLayoutGeometryDeltaCssPx,
          pliegoSha256: sha256(Buffer.from(JSON.stringify(pliego.computed))),
          tailwindSha256: sha256(Buffer.from(JSON.stringify(tailwind.computed))),
          nodeCount: Object.keys(pliego.computed.nodes).length,
        },
        screenshots: {
          ...screenshotComparison,
          threshold: authority.comparison.screenshotPixelThreshold,
          maximumMismatchRatio: authority.comparison.maximumScreenshotMismatchRatio,
          pliego: {
            path: relative(outputDirectory, paths.pliego).replaceAll("\\", "/"),
            sha256: sha256(pliego.screenshot),
            bytes: pliego.screenshot.byteLength,
          },
          tailwind: {
            path: relative(outputDirectory, paths.tailwind).replaceAll("\\", "/"),
            sha256: sha256(tailwind.screenshot),
            bytes: tailwind.screenshot.byteLength,
          },
        },
      });
    }
  }
} finally {
  await browserInstance.close();
  const workChild = relative(
    resolve(repositoryRoot, "target", "browser-output-certification"),
    prepared.workRoot,
  );
  if (
    !workChild ||
    workChild === ".." ||
    workChild.startsWith(`..${sep}`) ||
    isAbsolute(workChild) ||
    !basename(prepared.workRoot).startsWith("work-")
  ) {
    fail("refusing to clean an unsafe certification work directory");
  }
  rmSync(prepared.workRoot, { recursive: true, force: true, maxRetries: 3, retryDelay: 100 });
}

const result = computedMismatchCount === 0 && screenshotMismatchCount === 0 ? "passed" : "failed";
const document = {
  schemaVersion: 1,
  kind: "pliegocss-browser-output-host-evidence",
  result,
  generatedAtUtc: instant(generatedAt),
  expiresAtUtc: instant(expiresAt),
  source: state,
  host: {
    id: options.host.id,
    runner: options.host.runner,
    os: process.platform,
    arch: process.arch,
    node: process.version,
    playwright: playwrightPackage.version,
    browser: options.browser,
    browserVersion: browserInstance.version(),
  },
  authority: {
    path: relative(repositoryRoot, authorityPath).replaceAll("\\", "/"),
    sha256: sha256(readFileSync(authorityPath)),
  },
  compiler: {
    sha256: compiler.sha256,
    rustc: compiler.rustc,
    host: compiler.host,
  },
  inputs: prepared.provenance,
  resetModes: authority.resetModes,
  computedProperties: authority.computedProperties,
  observations,
  coverage: authority.requiredCoverage,
  summary: {
    resetModeCount: authority.resetModes.length,
    scenarioCount: observations.length,
    computedMismatchCount,
    screenshotMismatchCount,
    maximumLayoutGeometryDeltaCssPx: Number(maximumLayoutGeometryDeltaCssPx.toFixed(6)),
  },
  claimBoundary: authority.comparison.claimBoundary,
};
writeFileSync(options.output, `${JSON.stringify(document, null, 2)}\n`);
process.stdout.write(
  `${JSON.stringify(
    {
      schemaVersion: 1,
      result,
      host: options.host.id,
      browserVersion: document.host.browserVersion,
      scenarios: observations.length,
      computedMismatchCount,
      screenshotMismatchCount,
      maximumLayoutGeometryDeltaCssPx: document.summary.maximumLayoutGeometryDeltaCssPx,
      output: relative(repositoryRoot, options.output).replaceAll("\\", "/"),
    },
    null,
    2,
  )}\n`,
);
if (result !== "passed") process.exitCode = 1;
