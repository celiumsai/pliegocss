import { spawnSync } from "node:child_process";
import {
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname, isAbsolute, join, relative, resolve, sep } from "node:path";
import { chromium } from "playwright";
import { build as viteBuild } from "vite";
import {
  authorityPath,
  decorateClassNodes,
  extractStaticClassGroups,
  hostFor,
  loadContract,
  profileById,
  readJson,
  renderProjectDocument,
  repositoryRoot,
  rewriteStaticClassGroups,
  sha256,
} from "./adapter-coexistence-v1.mjs";
import { targetDirectoryForRustcIdentity } from "./rust-target.mjs";

function fail(message) {
  throw new Error(`adapter coexistence certification: ${message}`);
}

function parseOptions(arguments_) {
  const options = { browser: "chromium", host: null, output: null, requireClean: false };
  for (const argument of arguments_) {
    if (argument === "--") continue;
    if (argument === "--require-clean") options.requireClean = true;
    else if (argument.startsWith("--browser=")) options.browser = argument.slice(10);
    else if (argument.startsWith("--host=")) options.host = argument.slice(7);
    else if (argument.startsWith("--output=")) options.output = resolve(repositoryRoot, argument.slice(9));
    else {
      fail(
        "usage: node scripts/adapter-coexistence-certification.mjs [--browser=chromium] [--host=ID] [--output=PATH] [--require-clean]",
      );
    }
  }
  if (options.browser !== "chromium") fail("schema 1 certifies Chromium only");
  return options;
}

function run(command, arguments_, options = {}) {
  const result = spawnSync(command, arguments_, {
    cwd: repositoryRoot,
    encoding: "utf8",
    windowsHide: true,
    timeout: 600_000,
    maxBuffer: 64 * 1024 * 1024,
    ...options,
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    fail(
      `${command} ${arguments_.join(" ")} exited ${result.status}\n${`${result.stdout ?? ""}${result.stderr ?? ""}`.slice(-16_384)}`,
    );
  }
  return result;
}

function git(arguments_) {
  return run("git", arguments_).stdout;
}

function gitState(requireClean) {
  const status = git(["status", "--porcelain=v1", "-z", "--untracked-files=all"]);
  if (requireClean && status.length !== 0) fail("--require-clean found a dirty source tree");
  return {
    commit: git(["rev-parse", "HEAD"]).trim(),
    gitTree: git(["rev-parse", "HEAD^{tree}"]).trim(),
    dirty: status.length !== 0,
    statusEntryCount: status.split("\0").filter(Boolean).length,
    statusSha256: sha256(Buffer.from(status, "utf8")),
  };
}

function prepareCompiler() {
  const rustcVerbose = run("rustc", ["+1.96.0", "-vV"]).stdout.replaceAll("\r\n", "\n").trim();
  const host = /^host: (\S+)$/mu.exec(rustcVerbose)?.[1];
  if (!host) fail("cannot resolve the Rust host");
  const cargoTarget = targetDirectoryForRustcIdentity(repositoryRoot, process.env, rustcVerbose);
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

function themeCss() {
  return `@theme {\n  --color-canvas: oklch(98.5% .004 80);\n  --color-surface: #fff;\n  --color-surface-raised: oklch(96% .006 80);\n  --color-ink: oklch(19% .012 70);\n  --color-muted: oklch(50% .014 70);\n  --color-accent: oklch(58% .19 32);\n  --color-accent-strong: oklch(50% .18 30);\n  --color-line: oklch(88% .008 75);\n  --font-sans: ui-sans-serif, system-ui, sans-serif;\n  --shadow-sm: 0 1px 2px #0000000d;\n}\n`;
}

function v3Config() {
  return `module.exports = {\n  content: ["./index.html"],\n  corePlugins: { preflight: false },\n  theme: { extend: {\n    colors: { canvas: "oklch(98.5% .004 80)", surface: "#fff", "surface-raised": "oklch(96% .006 80)", ink: "oklch(19% .012 70)", muted: "oklch(50% .014 70)", accent: "oklch(58% .19 32)", "accent-strong": "oklch(50% .18 30)", line: "oklch(88% .008 75)" },\n    fontFamily: { sans: ["ui-sans-serif", "system-ui", "sans-serif"] },\n    boxShadow: { sm: "0 1px 2px #0000000d" }\n  } }\n};\n`;
}

function compileTailwind(profile, stage) {
  const input = join(stage, "tailwind.input.css");
  const output = join(stage, "tailwind.css");
  const environment = { ...process.env, BROWSERSLIST_IGNORE_OLD_DATA: "1" };
  if (profile.id === "tailwind-v3-lts") {
    writeFileSync(input, "@tailwind utilities;\n");
    writeFileSync(join(stage, "tailwind.config.cjs"), v3Config());
    run(
      process.execPath,
      [
        resolve(repositoryRoot, "node_modules", profile.packageAlias, "lib", "cli.js"),
        "-i",
        "tailwind.input.css",
        "-o",
        "tailwind.css",
        "--minify",
        "-c",
        "tailwind.config.cjs",
      ],
      { cwd: stage, env: environment },
    );
  } else if (profile.id === "tailwind-v4-current") {
    writeFileSync(
      input,
      `@layer theme, utilities;\n@import "${profile.packageAlias}/theme.css" layer(theme);\n@import "${profile.packageAlias}/utilities.css" layer(utilities) source(none);\n@source "./index.html";\n${themeCss()}`,
    );
    const cli = resolve(
      repositoryRoot,
      "node_modules",
      profile.cliPackageAlias,
      "dist",
      "index.mjs",
    );
    run(process.execPath, [cli, "-i", "tailwind.input.css", "-o", "tailwind.css", "--minify"], {
      cwd: stage,
      env: environment,
    });
  } else {
    fail(`unsupported Tailwind profile ${profile.id}`);
  }
  const bytes = readFileSync(output);
  if (bytes.length === 0) fail(`${profile.id} emitted empty CSS`);
  return {
    input: readFileSync(input),
    output: bytes,
    inputSha256: sha256(readFileSync(input)),
    outputSha256: sha256(bytes),
    outputBytes: bytes.length,
  };
}

function auditTailwind(compiler, stage) {
  const control = join(stage, "tailwind-audit-control");
  mkdirSync(control, { recursive: true });
  const arguments_ = [
    "audit",
    "--input",
    "tailwind.css",
    "--targets",
    "modern",
    "--format",
    "json",
    "--control-dir",
    "tailwind-audit-control",
  ];
  const first = run(compiler.executable, arguments_, { cwd: stage });
  const check = run(compiler.executable, [...arguments_, "--check"], { cwd: stage });
  if (first.stdout !== check.stdout) fail("Tailwind output audit changed in --check mode");
  const report = JSON.parse(first.stdout);
  const findings = report.findings ?? [];
  if (findings.some((finding) => finding.severity === "error")) {
    fail("Tailwind output audit contains a blocking error");
  }
  const files = [
    "pliego.css.findings.json",
    "pliego.css.manifest.json",
    "pliego.css.receipt.json",
  ];
  return {
    reportSha256: sha256(Buffer.from(first.stdout)),
    findings: findings.length,
    severities: Object.fromEntries(
      ["error", "warning", "info"].map((severity) => [
        severity,
        findings.filter((finding) => finding.severity === severity).length,
      ]),
    ),
    codes: [...new Set(findings.map((finding) => finding.code))].sort(),
    control: Object.fromEntries(
      files.map((file) => {
        const bytes = readFileSync(join(control, file));
        return [file, { bytes: bytes.length, sha256: sha256(bytes) }];
      }),
    ),
  };
}

function inventoryTailwindSource(compiler, stage, profile) {
  const arguments_ = ["migration-inventory", "tailwind", "tailwind.input.css"];
  const first = run(compiler.executable, arguments_, { cwd: stage });
  const second = run(compiler.executable, arguments_, { cwd: stage });
  if (first.stderr !== "" || second.stderr !== "" || first.stdout !== second.stdout) {
    fail(`${profile.id} source inventory is not deterministic and clean`);
  }
  const inventory = JSON.parse(first.stdout);
  const kinds = inventory.constructs?.map((construct) => construct.kind) ?? [];
  if (
    inventory.schemaVersion !== 1 ||
    inventory.sourceKind !== "tailwind" ||
    inventory.preflightReliance !== "not-observed" ||
    inventory.summary?.dynamic !== 0 ||
    inventory.summary?.unsupported !== 0 ||
    (profile.id === "tailwind-v3-lts" && !kinds.includes("tailwind-layer")) ||
    (profile.id === "tailwind-v4-current" && !kinds.includes("tailwind-import"))
  ) {
    fail(`${profile.id} source inventory did not match its bounded v3/v4 contract`);
  }
  return {
    sha256: sha256(Buffer.from(first.stdout)),
    bytes: Buffer.byteLength(first.stdout),
    constructs: inventory.summary.constructs,
    dynamic: inventory.summary.dynamic,
    unsupported: inventory.summary.unsupported,
    preflightReliance: inventory.preflightReliance,
    kinds: [...new Set(kinds)].sort(),
  };
}

function compilePliego(compiler, stage, document) {
  const { uniqueGroups } = extractStaticClassGroups(document, "staged project");
  writeFileSync(join(stage, "styles.txt"), `${uniqueGroups.join("\n")}\n`);
  run(
    compiler.executable,
    [
      "compile",
      "--input",
      "styles.txt",
      "--seed",
      "--theme",
      "--targets",
      "modern",
      "--output",
      "pliego.after.css",
      "--manifest",
      "pliego.manifest.json",
    ],
    { cwd: stage },
  );
  const css = readFileSync(join(stage, "pliego.after.css"));
  const manifestBytes = readFileSync(join(stage, "pliego.manifest.json"));
  const manifest = JSON.parse(manifestBytes.toString("utf8"));
  if (manifest.styles?.length !== uniqueGroups.length) {
    fail(`Pliego manifest expected ${uniqueGroups.length} styles, found ${manifest.styles?.length}`);
  }
  return {
    css,
    cssSha256: sha256(css),
    manifest,
    manifestSha256: sha256(manifestBytes),
    styleGroups: uniqueGroups.length,
  };
}

function prepareMigration(compiler, stage, document, pliego) {
  const rewrite = rewriteStaticClassGroups(document, pliego.manifest, "staged project");
  writeFileSync(join(stage, "document.after.html"), rewrite.document);
  writeFileSync(join(stage, "pliego.css"), Buffer.alloc(0));
  const group = {
    schemaVersion: 1,
    entries: [
      { file: "index.html", after: "document.after.html" },
      { file: "pliego.css", after: "pliego.after.css" },
    ],
  };
  const groupBytes = Buffer.from(`${JSON.stringify(group, null, 2)}\n`);
  writeFileSync(join(stage, "migration.group.json"), groupBytes);
  const before = {
    document: readFileSync(join(stage, "index.html")),
    pliego: readFileSync(join(stage, "pliego.css")),
    tailwind: readFileSync(join(stage, "tailwind.css")),
  };
  run(
    compiler.executable,
    [
      "migration-group-apply",
      "--manifest",
      "migration.group.json",
      "--receipt",
      "migration.receipt.json",
    ],
    { cwd: stage },
  );
  const receiptBytes = readFileSync(join(stage, "migration.receipt.json"));
  const receipt = JSON.parse(receiptBytes.toString("utf8"));
  const after = {
    document: readFileSync(join(stage, "index.html")),
    pliego: readFileSync(join(stage, "pliego.css")),
    tailwind: readFileSync(join(stage, "tailwind.css")),
  };
  if (
    sha256(after.document) !== sha256(Buffer.from(rewrite.document)) ||
    sha256(after.pliego) !== pliego.cssSha256 ||
    sha256(after.tailwind) !== sha256(before.tailwind)
  ) {
    fail("applied migration bytes do not match the approved group");
  }
  return {
    rewrite,
    groupSha256: sha256(groupBytes),
    receipt,
    receiptSha256: sha256(receiptBytes),
    before,
    after,
  };
}

function rollbackMigration(compiler, stage, migration) {
  run(
    compiler.executable,
    ["migration-group-rollback", "--receipt", "migration.receipt.json"],
    { cwd: stage },
  );
  const restored = {
    document: readFileSync(join(stage, "index.html")),
    pliego: readFileSync(join(stage, "pliego.css")),
    tailwind: readFileSync(join(stage, "tailwind.css")),
  };
  for (const key of Object.keys(restored)) {
    if (sha256(restored[key]) !== sha256(migration.before[key])) {
      fail(`rollback did not restore exact ${key} bytes`);
    }
  }
  try {
    statSync(join(stage, "migration.receipt.json"));
    fail("rollback left the migration receipt behind");
  } catch (error) {
    if (error.code !== "ENOENT") throw error;
  }
  return {
    exact: true,
    documentSha256: sha256(restored.document),
    pliegoSha256: sha256(restored.pliego),
    tailwindSha256: sha256(restored.tailwind),
  };
}

async function viteBuildSnapshot(stage, outDir) {
  await viteBuild({
    root: stage,
    configFile: false,
    logLevel: "silent",
    base: "./",
    build: {
      outDir,
      emptyOutDir: true,
      minify: false,
      cssMinify: false,
    },
  });
  const root = resolve(stage, outDir);
  const htmlBytes = readFileSync(join(root, "index.html"));
  const html = htmlBytes.toString("utf8");
  const hrefs = [...html.matchAll(/<link[^>]+rel="stylesheet"[^>]+href="([^"]+\.css)"[^>]*>/gu)].map(
    (match) => match[1],
  );
  if (hrefs.length === 0) fail("Vite output has no stylesheet asset");
  const css = Buffer.concat(
    hrefs.map((href) => readFileSync(resolve(root, href.replace(/^\.\//u, "")))),
  );
  return {
    html,
    htmlSha256: sha256(htmlBytes),
    css,
    cssSha256: sha256(css),
    assets: listFiles(root).map((path) => {
      const bytes = readFileSync(path);
      return {
        path: relative(root, path).replaceAll("\\", "/"),
        bytes: bytes.length,
        sha256: sha256(bytes),
      };
    }),
  };
}

function listFiles(directory) {
  return readdirSync(directory, { withFileTypes: true })
    .sort((left, right) => left.name.localeCompare(right.name, "en"))
    .flatMap((entry) => {
      const path = join(directory, entry.name);
      return entry.isDirectory() ? listFiles(path) : [path];
    });
}

function injectCss(document, css) {
  const withoutLinks = document.replace(/<link[^>]+rel="stylesheet"[^>]*>\s*/gu, "");
  const deterministic =
    "*,:before,:after{animation-duration:0s!important;animation-delay:0s!important;transition-duration:0s!important;caret-color:transparent!important}";
  return withoutLinks.replace(
    "</head>",
    `<style data-g6-certification>${css.toString("utf8")}\n${deterministic}</style></head>`,
  );
}

function normalizeComputed(value) {
  return value
    .replaceAll(/\s+/gu, " ")
    .replaceAll("flex-end", "end")
    .trim();
}

async function browserSnapshot(browser, document, css, viewport, properties) {
  const page = await browser.newPage({ viewport });
  const consoleErrors = [];
  const pageErrors = [];
  page.on("console", (message) => {
    if (message.type() === "error") consoleErrors.push(message.text());
  });
  page.on("pageerror", (error) => pageErrors.push(error.message));
  try {
    await page.setContent(injectCss(document, css), { waitUntil: "load" });
    const dom = await page.evaluate(() => {
      function project(node) {
        if (node.nodeType === Node.TEXT_NODE) {
          const text = node.textContent.replace(/\s+/gu, " ").trim();
          return text ? { text } : null;
        }
        if (node.nodeType !== Node.ELEMENT_NODE) return null;
        const attributes = [...node.attributes]
          .filter((attribute) => attribute.name !== "class" && attribute.name !== "data-g6-node")
          .map((attribute) => [attribute.name, attribute.value])
          .sort(([left], [right]) => left.localeCompare(right, "en"));
        return {
          tag: node.tagName.toLowerCase(),
          attributes,
          children: [...node.childNodes].map(project).filter(Boolean),
        };
      }
      return project(document.body);
    });
    const aria = await page.locator("body").ariaSnapshot();
    const nodes = await page.locator("[data-g6-node]").evaluateAll(
      (elements, frozenProperties) =>
        elements.map((element) => {
          const style = getComputedStyle(element);
          const rectangle = element.getBoundingClientRect();
          return {
            id: element.getAttribute("data-g6-node"),
            computed: Object.fromEntries(
              frozenProperties.map((property) => [property, style.getPropertyValue(property)]),
            ),
            geometry: {
              x: rectangle.x,
              y: rectangle.y,
              width: rectangle.width,
              height: rectangle.height,
            },
          };
        }),
      properties,
    );
    if (consoleErrors.length || pageErrors.length) {
      fail(`browser emitted errors: ${JSON.stringify({ consoleErrors, pageErrors })}`);
    }
    return { dom, aria, nodes };
  } finally {
    await page.close();
  }
}

function compareSnapshots(project, baseline, candidate, maximumGeometryDelta) {
  if (JSON.stringify(baseline.dom) !== JSON.stringify(candidate.dom)) {
    fail(`${project.id} DOM semantics changed`);
  }
  if (baseline.aria !== candidate.aria) fail(`${project.id} ARIA snapshot changed`);
  if (baseline.nodes.length !== candidate.nodes.length || baseline.nodes.length === 0) {
    fail(`${project.id} class-bearing node inventory changed`);
  }
  let maximumObservedGeometryDelta = 0;
  for (let index = 0; index < baseline.nodes.length; index += 1) {
    const left = baseline.nodes[index];
    const right = candidate.nodes[index];
    if (left.id !== right.id) fail(`${project.id} node identity changed at ${index}`);
    for (const property of Object.keys(left.computed)) {
      const expected = normalizeComputed(left.computed[property]);
      const actual = normalizeComputed(right.computed[property]);
      if (expected !== actual) {
        fail(
          `${project.id} computed ${property} changed on ${left.id}: ${JSON.stringify(expected)} != ${JSON.stringify(actual)}`,
        );
      }
    }
    for (const coordinate of ["x", "y", "width", "height"]) {
      const delta = Math.abs(left.geometry[coordinate] - right.geometry[coordinate]);
      maximumObservedGeometryDelta = Math.max(maximumObservedGeometryDelta, delta);
      if (delta > maximumGeometryDelta) {
        fail(`${project.id} geometry ${coordinate} changed by ${delta} CSS px on ${left.id}`);
      }
    }
  }
  return {
    domEquivalent: true,
    ariaEquivalent: true,
    computedStyleEquivalent: true,
    geometryEquivalent: true,
    nodes: baseline.nodes.length,
    maximumObservedGeometryDeltaCssPx: maximumObservedGeometryDelta,
    ariaSha256: sha256(Buffer.from(baseline.aria)),
    domSha256: sha256(Buffer.from(JSON.stringify(baseline.dom))),
  };
}

function instant(date = new Date()) {
  return date.toISOString().replace(/\.\d{3}Z$/u, "Z");
}

const options = parseOptions(process.argv.slice(2));
const contract = loadContract();
const host = hostFor(contract.authority, options.browser, options.host);
options.output ??= resolve(
  repositoryRoot,
  "target",
  "adapter-coexistence-certification",
  `${host.id}.local.json`,
);
const outputChild = relative(repositoryRoot, options.output);
if (
  !outputChild ||
  outputChild === ".." ||
  outputChild.startsWith(`..${sep}`) ||
  isAbsolute(outputChild) ||
  !options.output.endsWith(".json")
) {
  fail("--output must be a .json path inside the repository");
}

const source = gitState(options.requireClean);
const compiler = prepareCompiler();
const resetBytes = readFileSync(resolve(repositoryRoot, contract.authority.sharedReset.path));
const workRoot = resolve(
  repositoryRoot,
  "target",
  "adapter-coexistence-certification",
  `work-${process.pid}-${Date.now()}`,
);
mkdirSync(workRoot, { recursive: true });

const browser = await chromium.launch({ headless: true });
const startedAt = instant();
const projects = [];
try {
  for (const project of contract.corpus.projects) {
    process.stdout.write(`[adapter-coexistence] ${project.id}\n`);
    const stage = join(workRoot, project.id);
    mkdirSync(stage, { recursive: true });
    const document = decorateClassNodes(renderProjectDocument(project), project.id);
    writeFileSync(join(stage, "index.html"), document);
    const profile = profileById(contract.authority, project.tailwindProfile);
    const tailwind = compileTailwind(profile, stage);
    const sourceInventory = inventoryTailwindSource(compiler, stage, profile);
    const audit = auditTailwind(compiler, stage);
    const pliego = compilePliego(compiler, stage, document);
    const baselineSource = {
      document,
      css: Buffer.concat([resetBytes, tailwind.output]),
    };
    let baselineBuild = null;
    if (project.adapter === "vite") {
      writeFileSync(join(stage, "pliego.css"), Buffer.alloc(0));
      baselineBuild = await viteBuildSnapshot(stage, "dist-baseline");
    }
    const migration = prepareMigration(compiler, stage, document, pliego);
    let candidateBuild = null;
    if (project.adapter === "vite") candidateBuild = await viteBuildSnapshot(stage, "dist-candidate");
    const baselineDocument = baselineBuild?.html ?? baselineSource.document;
    const baselineCss = baselineBuild
      ? Buffer.concat([resetBytes, baselineBuild.css])
      : baselineSource.css;
    const candidateDocument = candidateBuild?.html ?? migration.after.document.toString("utf8");
    const candidateCss = candidateBuild
      ? Buffer.concat([resetBytes, candidateBuild.css])
      : Buffer.concat([resetBytes, migration.after.tailwind, migration.after.pliego]);
    const baselineSnapshot = await browserSnapshot(
      browser,
      baselineDocument,
      baselineCss,
      project.viewport,
      contract.authority.comparison.computedProperties,
    );
    const candidateSnapshot = await browserSnapshot(
      browser,
      candidateDocument,
      candidateCss,
      project.viewport,
      contract.authority.comparison.computedProperties,
    );
    const equivalence = compareSnapshots(
      project,
      baselineSnapshot,
      candidateSnapshot,
      contract.authority.comparison.maximumLayoutGeometryDeltaCssPx,
    );
    const rollback = rollbackMigration(compiler, stage, migration);
    projects.push({
      id: project.id,
      adapter: project.adapter,
      tailwindProfile: project.tailwindProfile,
      viewport: project.viewport,
      source: {
        documentSha256: sha256(Buffer.from(document)),
        classGroups: extractStaticClassGroups(document, project.id).groups.length,
      },
      tailwind: {
        version: profile.version,
        inputSha256: tailwind.inputSha256,
        outputSha256: tailwind.outputSha256,
        outputBytes: tailwind.outputBytes,
        sourceInventory,
        audit,
      },
      pliego: {
        outputSha256: pliego.cssSha256,
        outputBytes: pliego.css.length,
        manifestSha256: pliego.manifestSha256,
        styleGroups: pliego.styleGroups,
      },
      migration: {
        mode: contract.authority.migration.mode,
        replacements: migration.rewrite.replacements,
        groupSha256: migration.groupSha256,
        receiptSha256: migration.receiptSha256,
        receiptSchemaVersion: migration.receipt.schemaVersion,
        beforeDocumentSha256: sha256(migration.before.document),
        afterDocumentSha256: sha256(migration.after.document),
        tailwindRetained: sha256(migration.before.tailwind) === sha256(migration.after.tailwind),
        rollback,
      },
      vite:
        project.adapter === "vite"
          ? {
              version: contract.authority.adapters.find((entry) => entry.id === "vite").version,
              baseline: {
                htmlSha256: baselineBuild.htmlSha256,
                cssSha256: baselineBuild.cssSha256,
                assets: baselineBuild.assets,
              },
              candidate: {
                htmlSha256: candidateBuild.htmlSha256,
                cssSha256: candidateBuild.cssSha256,
                assets: candidateBuild.assets,
              },
            }
          : null,
      equivalence,
      passed: true,
    });
  }
} finally {
  await browser.close();
}

const completedAt = instant();
const adapterCounts = Object.fromEntries(
  contract.authority.adapters.map((adapter) => [
    adapter.id,
    projects.filter((project) => project.adapter === adapter.id).length,
  ]),
);
const profileCounts = Object.fromEntries(
  contract.authority.tailwindProfiles.map((profile) => [
    profile.id,
    projects.filter((project) => project.tailwindProfile === profile.id).length,
  ]),
);
const evidence = {
  schemaVersion: 1,
  kind: "pliegocss-adapter-coexistence-host-evidence",
  result: projects.length >= contract.authority.corpus.minimumProjects ? "pass" : "fail",
  startedAt,
  completedAt,
  source,
  authority: {
    path: relative(repositoryRoot, authorityPath).replaceAll("\\", "/"),
    sha256: contract.authoritySha256,
    corpusPath: relative(repositoryRoot, contract.corpusPath).replaceAll("\\", "/"),
    corpusSha256: contract.corpusSha256,
  },
  host: {
    ...host,
    browserVersion: browser.version(),
    playwright: readJson(resolve(repositoryRoot, "node_modules", "playwright", "package.json")).version,
  },
  compiler,
  summary: {
    projects: projects.length,
    passed: projects.filter((project) => project.passed).length,
    adapters: adapterCounts,
    tailwindProfiles: profileCounts,
    domEquivalent: projects.filter((project) => project.equivalence.domEquivalent).length,
    ariaEquivalent: projects.filter((project) => project.equivalence.ariaEquivalent).length,
    computedStyleEquivalent: projects.filter(
      (project) => project.equivalence.computedStyleEquivalent,
    ).length,
    geometryEquivalent: projects.filter((project) => project.equivalence.geometryEquivalent).length,
    exactRollbacks: projects.filter((project) => project.migration.rollback.exact).length,
  },
  projects,
  claimBoundary: contract.authority.claimBoundary,
};
if (evidence.result !== "pass") fail("project floor was not met");
mkdirSync(dirname(options.output), { recursive: true });
writeFileSync(options.output, `${JSON.stringify(evidence, null, 2)}\n`);
rmSync(workRoot, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 });
process.stdout.write(
  `${JSON.stringify(
    {
      passed: true,
      host: host.id,
      projects: projects.length,
      adapters: adapterCounts,
      tailwindProfiles: profileCounts,
      output: relative(repositoryRoot, options.output).replaceAll("\\", "/"),
    },
    null,
    2,
  )}\n`,
);
