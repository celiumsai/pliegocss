import { spawnSync } from "node:child_process";
import { createRequire } from "node:module";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import {
  availableParallelism,
  cpus,
  release as osRelease,
  totalmem,
  type as osType,
  version as osVersion,
} from "node:os";
import { basename, dirname, isAbsolute, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { brotliCompressSync, constants as zlibConstants, gzipSync } from "node:zlib";
import {
  authorityPath,
  canonicalLaneMap,
  corpusMetrics,
  extractClassValues,
  loadAuthority,
  materializeCorpus,
  readInstalledPackage,
  repositoryRoot,
  resolveLaneCli,
  round,
  sha256,
  summarize,
} from "./benchmark-authority-v2.mjs";
import { targetDirectoryForRustcIdentity } from "./rust-target.mjs";

const require = createRequire(import.meta.url);
const escapeClassName = require(
  join(repositoryRoot, "node_modules", "tailwindcss-v3-lts", "lib", "util", "escapeClassName.js"),
).default;
const benchmarkRoot = join(repositoryRoot, "target", "benchmarks", "benchmark-authority-v2");
const localResultPath = join(
  repositoryRoot,
  "benchmarks",
  "results",
  "benchmark-authority-v2.local.json",
);
const evidenceRoot = join(repositoryRoot, "benchmarks", "evidence", "v2");
const harnessPath = fileURLToPath(import.meta.url);
const runnerSourcePath = join(repositoryRoot, "scripts", "benchmark-process-metrics.rs");
const packagePath = join(repositoryRoot, "package.json");
const lockPath = join(repositoryRoot, "pnpm-lock.yaml");

function fail(message) {
  throw new Error(`Benchmark Authority v2 measurement: ${message}`);
}

function parsePositiveInteger(value, label, allowZero = false) {
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed) || parsed < (allowZero ? 0 : 1)) {
    fail(`${label} must be ${allowZero ? "a non-negative" : "a positive"} integer`);
  }
  return parsed;
}

function parseOptions(arguments_, authority) {
  const options = {
    pairs: authority.methodology.measuredPairs,
    warmups: authority.methodology.warmupPairs,
    laneIds: authority.lanes.map((lane) => lane.id),
    corpusIds: authority.corpora.map((corpus) => corpus.id),
    evidence: null,
    smoke: false,
  };
  for (const argument of arguments_) {
    if (argument === "--smoke") {
      options.smoke = true;
      options.pairs = 1;
      options.warmups = 0;
    } else if (argument.startsWith("--pairs=")) {
      options.pairs = parsePositiveInteger(argument.slice("--pairs=".length), "--pairs");
    } else if (argument.startsWith("--warmups=")) {
      options.warmups = parsePositiveInteger(
        argument.slice("--warmups=".length),
        "--warmups",
        true,
      );
    } else if (argument.startsWith("--lanes=")) {
      options.laneIds = argument.slice("--lanes=".length).split(",").filter(Boolean);
    } else if (argument.startsWith("--corpora=")) {
      options.corpusIds = argument.slice("--corpora=".length).split(",").filter(Boolean);
    } else if (argument.startsWith("--evidence=")) {
      options.evidence = resolve(repositoryRoot, argument.slice("--evidence=".length));
    } else {
      fail(`unknown argument ${JSON.stringify(argument)}`);
    }
  }
  if (options.evidence && (options.smoke || options.pairs !== authority.methodology.measuredPairs || options.warmups !== authority.methodology.warmupPairs)) {
    fail("evidence requires the canonical 5 warmup and 30 measured pairs");
  }
  return options;
}

function run(command, arguments_, options = {}) {
  const result = spawnSync(command, arguments_, {
    cwd: repositoryRoot,
    encoding: "utf8",
    windowsHide: true,
    timeout: 180_000,
    maxBuffer: 16 * 1024 * 1024,
    ...options,
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    fail(
      `${command} ${arguments_.join(" ")} exited ${result.status}\n${`${result.stderr ?? ""}`.slice(-4_096)}`,
    );
  }
  return result;
}

function git(arguments_, fallback = "") {
  try {
    return run("git", arguments_).stdout.trim() || fallback;
  } catch {
    return fallback;
  }
}

function gitState() {
  const status = run("git", ["status", "--porcelain=v1", "-z", "--untracked-files=all"]).stdout;
  return {
    commit: git(["rev-parse", "HEAD"], "unborn"),
    dirty: status.length > 0,
    statusEntryCount: status.split("\0").filter(Boolean).length,
    statusSha256: sha256(Buffer.from(status, "utf8")),
  };
}

function safeEvidencePath(path) {
  if (!path) return;
  const child = relative(evidenceRoot, path);
  if (
    !child ||
    child === ".." ||
    child.startsWith(`..${sep}`) ||
    isAbsolute(child) ||
    !path.endsWith(".json") ||
    existsSync(path)
  ) {
    fail("--evidence must be a new .json file below benchmarks/evidence/v2");
  }
}

function capturePowerPlan() {
  const declared = process.env.PLIEGO_BENCH_POWER_PLAN?.trim();
  if (declared) return { status: "declared", source: "PLIEGO_BENCH_POWER_PLAN", value: declared };
  const probes =
    process.platform === "win32"
      ? [["powercfg", ["/getactivescheme"]]]
      : process.platform === "darwin"
        ? [["pmset", ["-g"]]]
        : [["powerprofilesctl", ["get"]]];
  for (const [command, arguments_] of probes) {
    const result = spawnSync(command, arguments_, { encoding: "utf8", windowsHide: true });
    if (!result.error && result.status === 0 && result.stdout.trim()) {
      return { status: "recorded", source: `${command} ${arguments_.join(" ")}`, value: result.stdout.trim() };
    }
  }
  return { status: "unavailable", source: null, value: null };
}

function captureSecurityTooling(evidence) {
  const status = process.env.PLIEGO_BENCH_SECURITY_TOOLING_STATUS?.trim().toLowerCase();
  const tooling = process.env.PLIEGO_BENCH_SECURITY_TOOLING?.trim() || null;
  const allowed = new Set(["active", "disabled", "not-installed", "unknown"]);
  if (!status) {
    if (evidence) fail("evidence requires PLIEGO_BENCH_SECURITY_TOOLING_STATUS");
    return { status: "not-reported", tooling: null };
  }
  if (!allowed.has(status)) fail("invalid PLIEGO_BENCH_SECURITY_TOOLING_STATUS");
  if ((status === "active" || status === "disabled") && !tooling) {
    fail(`${status} security tooling requires PLIEGO_BENCH_SECURITY_TOOLING`);
  }
  return { status, tooling };
}

function toolchain() {
  const rustcVerbose = run("rustc", ["+1.96.0", "-vV"]).stdout.trim();
  const host = /^host: (\S+)$/mu.exec(rustcVerbose)?.[1];
  if (!host) fail("cannot resolve the Rust 1.96.0 host target");
  return {
    cargo: run("cargo", ["+1.96.0", "--version"]).stdout.trim(),
    rustc: /^rustc .+$/mu.exec(rustcVerbose)?.[0] ?? "unknown",
    rustcVerbose,
    host,
  };
}

function buildTools(toolchainValue) {
  const cargoTarget = targetDirectoryForRustcIdentity(
    repositoryRoot,
    process.env,
    toolchainValue.rustcVerbose,
  );
  const runnerRoot = join(cargoTarget, "benchmark-tools");
  mkdirSync(cargoTarget, { recursive: true });
  mkdirSync(runnerRoot, { recursive: true });
  const cargoEnvironment = {
    ...process.env,
    CARGO_INCREMENTAL: "0",
    CARGO_TARGET_DIR: cargoTarget,
    CARGO_TERM_COLOR: "never",
  };
  run(
    "cargo",
    ["+1.96.0", "build", "--release", "--locked", "--target", toolchainValue.host, "-p", "pliego-cssc"],
    { env: cargoEnvironment },
  );
  const pliego = join(
    cargoTarget,
    toolchainValue.host,
    "release",
    process.platform === "win32" ? "pliego-cssc.exe" : "pliego-cssc",
  );
  const runner = join(
    runnerRoot,
    process.platform === "win32" ? "benchmark-process-metrics.exe" : "benchmark-process-metrics",
  );
  run("rustc", ["+1.96.0", runnerSourcePath, "-O", "-o", runner]);
  return { pliego, runner, cargoTarget };
}

function measuredProcess(runner, command, arguments_, cwd) {
  const result = run(runner, [command, ...arguments_], { cwd, timeout: 60_000 });
  const metrics = JSON.parse(result.stdout.trim());
  if (
    metrics.schemaVersion !== 1 ||
    !Number.isSafeInteger(metrics.elapsedNs) ||
    metrics.elapsedNs <= 0 ||
    !Number.isSafeInteger(metrics.peakWorkingSetBytes) ||
    metrics.peakWorkingSetBytes <= 0 ||
    metrics.exitCode !== 0
  ) {
    fail(`invalid process metrics: ${result.stdout}`);
  }
  return metrics;
}

function v4Input(packageAlias) {
  return `@layer theme, utilities;\n@import "${packageAlias}/theme.css" layer(theme);\n@import "${packageAlias}/utilities.css" layer(utilities) source(none);\n@source "./fixture.html";\n@theme {\n  --color-canvas: oklch(0.985 0.004 80);\n  --color-surface: oklch(1 0 0);\n  --color-surface-raised: oklch(0.96 0.006 80);\n  --color-ink: oklch(0.19 0.012 70);\n  --color-muted: oklch(0.5 0.014 70);\n  --color-accent: oklch(0.58 0.19 32);\n  --color-accent-strong: oklch(0.5 0.18 30);\n  --color-line: oklch(0.88 0.008 75);\n}\n`;
}

function v3Config() {
  return `module.exports = {\n  content: ["./fixture.html"],\n  corePlugins: { preflight: false },\n  theme: {\n    extend: {\n      colors: {\n        canvas: "oklch(0.985 0.004 80 / <alpha-value>)",\n        surface: "oklch(1 0 0 / <alpha-value>)",\n        "surface-raised": "oklch(0.96 0.006 80 / <alpha-value>)",\n        ink: "oklch(0.19 0.012 70 / <alpha-value>)",\n        muted: "oklch(0.5 0.014 70 / <alpha-value>)",\n        accent: "oklch(0.58 0.19 32 / <alpha-value>)",\n        "accent-strong": "oklch(0.5 0.18 30 / <alpha-value>)",\n        line: "oklch(0.88 0.008 75 / <alpha-value>)"\n      }\n    }\n  }\n};\n`;
}

function prepareLaneWorkspace(workspace, lane, corpus, html, pliego) {
  const laneRoot = join(workspace, lane.id, corpus.id);
  mkdirSync(laneRoot, { recursive: true });
  const fixture = join(laneRoot, "fixture.html");
  const styles = join(laneRoot, "styles.txt");
  const tailwindInput = join(laneRoot, "input.css");
  const tailwindOutput = join(laneRoot, "tailwind.css");
  const pliegoOutput = join(laneRoot, "pliego.css");
  const manifestPath = join(laneRoot, "manifest.json");
  const manifestCss = join(laneRoot, "manifest.css");
  const classValues = extractClassValues(html);
  writeFileSync(fixture, html);
  writeFileSync(styles, `${classValues.join("\n")}\n`);
  if (lane.version.startsWith("3.")) {
    writeFileSync(tailwindInput, "@tailwind utilities;\n");
    writeFileSync(join(laneRoot, "tailwind.config.cjs"), v3Config());
  } else {
    writeFileSync(tailwindInput, v4Input(lane.packageAlias));
  }
  const cli = resolveLaneCli(lane);
  const tailwindArguments = [cli, "-i", tailwindInput, "-o", tailwindOutput, "--minify"];
  if (lane.version.startsWith("3.")) {
    tailwindArguments.push("-c", join(laneRoot, "tailwind.config.cjs"));
  }
  const pliegoArguments = [
    "compile",
    "--input",
    styles,
    "--seed",
    "--theme",
    "--output",
    pliegoOutput,
  ];
  run(process.execPath, tailwindArguments, { cwd: laneRoot, timeout: 60_000 });
  run(pliego, pliegoArguments, { cwd: laneRoot, timeout: 60_000 });
  run(
    pliego,
    [
      ...pliegoArguments.slice(0, -1),
      manifestCss,
      "--manifest",
      manifestPath,
    ],
    { cwd: laneRoot, timeout: 60_000 },
  );
  return {
    laneRoot,
    fixture,
    styles,
    tailwindOutput,
    pliegoOutput,
    manifestPath,
    manifestCss,
    classValues,
    commands: {
      pliego: { command: pliego, arguments: pliegoArguments },
      tailwind: { command: process.execPath, arguments: tailwindArguments },
    },
  };
}

function outputSize(bytes) {
  return {
    rawBytes: bytes.byteLength,
    gzipBytes: gzipSync(bytes, { level: 9 }).byteLength,
    brotliBytes: brotliCompressSync(bytes, {
      params: { [zlibConstants.BROTLI_PARAM_QUALITY]: 11 },
    }).byteLength,
    sha256: sha256(bytes),
  };
}

function generatedHtml(html, manifest) {
  const bySource = new Map();
  for (const style of manifest.styles) {
    for (const origin of style.origins) {
      const existing = bySource.get(origin.source);
      if (existing && existing !== style.className) fail(`manifest maps ${origin.source} twice`);
      bySource.set(origin.source, style.className);
    }
  }
  let replacements = 0;
  const rendered = html.replace(/class="([^"]*)"/gu, (_, source) => {
    const replacement = bySource.get(source);
    if (!replacement) fail(`manifest has no class for ${JSON.stringify(source)}`);
    replacements += 1;
    return `class="${replacement}"`;
  });
  return { bytes: Buffer.from(rendered, "utf8"), replacements };
}

function candidateCoverage(css, html) {
  const tokens = [...new Set(extractClassValues(html).flatMap((value) => value.trim().split(/\s+/u).filter(Boolean)))];
  const missing = tokens.filter((token) => !css.includes(`.${escapeClassName(token)}`));
  return {
    expected: tokens.length,
    emitted: tokens.length - missing.length,
    ratio: round((tokens.length - missing.length) / tokens.length),
    missing,
  };
}

function measureComparison(prepared, tools, html, warmups, pairs) {
  const invoke = (kind, timed) => {
    const command = prepared.commands[kind];
    if (timed) {
      return measuredProcess(tools.runner, command.command, command.arguments, prepared.laneRoot);
    }
    run(command.command, command.arguments, { cwd: prepared.laneRoot, timeout: 60_000 });
    return null;
  };
  for (let index = 0; index < warmups; index += 1) {
    const order = index % 2 === 0 ? ["pliego", "tailwind"] : ["tailwind", "pliego"];
    for (const kind of order) invoke(kind, false);
  }

  const observations = [];
  const hashes = { pliego: [], tailwind: [] };
  for (let index = 0; index < pairs; index += 1) {
    const order = index % 2 === 0 ? ["pliego", "tailwind"] : ["tailwind", "pliego"];
    const record = { pair: index + 1, order };
    for (const kind of order) {
      record[kind] = invoke(kind, true);
      hashes[kind].push(sha256(readFileSync(prepared[`${kind}Output`])));
    }
    record.deltaNs = record.pliego.elapsedNs - record.tailwind.elapsedNs;
    record.tailwindToPliegoRatio = round(record.tailwind.elapsedNs / record.pliego.elapsedNs);
    record.peakWorkingSetDeltaBytes =
      record.pliego.peakWorkingSetBytes - record.tailwind.peakWorkingSetBytes;
    observations.push(record);
  }
  for (const kind of ["pliego", "tailwind"]) {
    const unique = new Set(hashes[kind]);
    if (unique.size !== 1) fail(`${kind} emitted ${unique.size} hashes across measured pairs`);
    rmSync(prepared[`${kind}Output`], { force: true });
    invoke(kind, false);
    const freshHash = sha256(readFileSync(prepared[`${kind}Output`]));
    if (!unique.has(freshHash)) fail(`${kind} fresh output differs from measured output`);
  }

  const pliegoCss = readFileSync(prepared.pliegoOutput);
  const tailwindCss = readFileSync(prepared.tailwindOutput);
  const manifestCss = readFileSync(prepared.manifestCss);
  if (!pliegoCss.equals(manifestCss)) fail("manifest generation changed PliegoCSS output");
  const manifest = JSON.parse(readFileSync(prepared.manifestPath, "utf8"));
  const rewritten = generatedHtml(html, manifest);
  const pliegoCssSize = outputSize(pliegoCss);
  const tailwindCssSize = outputSize(tailwindCss);
  const utilityHtmlSize = outputSize(Buffer.from(html, "utf8"));
  const generatedHtmlSize = outputSize(rewritten.bytes);
  const coverage = candidateCoverage(tailwindCss.toString("utf8"), html);
  if (coverage.ratio !== 1) {
    fail(`Tailwind candidate coverage is ${coverage.emitted}/${coverage.expected}: ${coverage.missing.join(", ")}`);
  }

  const pliegoLatency = observations.map((pair) => pair.pliego.elapsedNs / 1_000_000);
  const tailwindLatency = observations.map((pair) => pair.tailwind.elapsedNs / 1_000_000);
  return {
    pairs: observations,
    summaries: {
      pliegoLatency: summarize(pliegoLatency, "ms"),
      tailwindLatency: summarize(tailwindLatency, "ms"),
      pairedLatencyDelta: summarize(
        observations.map((pair) => pair.deltaNs / 1_000_000),
        "ms",
      ),
      tailwindToPliegoRatio: summarize(
        observations.map((pair) => pair.tailwindToPliegoRatio),
        "ratio",
      ),
      pliegoPeakWorkingSet: summarize(
        observations.map((pair) => pair.pliego.peakWorkingSetBytes),
        "bytes",
      ),
      tailwindPeakWorkingSet: summarize(
        observations.map((pair) => pair.tailwind.peakWorkingSetBytes),
        "bytes",
      ),
      pairedPeakWorkingSetDelta: summarize(
        observations.map((pair) => pair.peakWorkingSetDeltaBytes),
        "bytes",
      ),
    },
    determinism: {
      measuredPairs: pairs,
      freshOutputVerified: true,
      pliegoSha256: pliegoCssSize.sha256,
      tailwindSha256: tailwindCssSize.sha256,
    },
    coverage,
    output: {
      pliego: {
        css: pliegoCssSize,
        html: generatedHtmlSize,
        transferGzipBytes: pliegoCssSize.gzipBytes + generatedHtmlSize.gzipBytes,
        transferBrotliBytes: pliegoCssSize.brotliBytes + generatedHtmlSize.brotliBytes,
        classAttributesRewritten: rewritten.replacements,
      },
      tailwind: {
        css: tailwindCssSize,
        html: utilityHtmlSize,
        transferGzipBytes: tailwindCssSize.gzipBytes + utilityHtmlSize.gzipBytes,
        transferBrotliBytes: tailwindCssSize.brotliBytes + utilityHtmlSize.brotliBytes,
      },
    },
  };
}

const authority = loadAuthority();
const options = parseOptions(process.argv.slice(2), authority);
const laneMap = canonicalLaneMap(authority);
const lanes = options.laneIds.map((id) => laneMap.get(id) ?? fail(`unknown lane ${id}`));
const corpusMap = new Map(authority.corpora.map((corpus) => [corpus.id, corpus]));
const corpora = options.corpusIds.map((id) => corpusMap.get(id) ?? fail(`unknown corpus ${id}`));
safeEvidencePath(options.evidence);
const stateBefore = gitState();
if (options.evidence && stateBefore.dirty) fail("evidence requires a clean Git worktree");
const toolchainValue = toolchain();
const tools = buildTools(toolchainValue);
const runDirectory = join(benchmarkRoot, `run-${process.pid}-${Date.now()}`);
mkdirSync(runDirectory, { recursive: true });

try {
  const results = {};
  for (const lane of lanes) {
    results[lane.id] = {
      role: lane.role,
      version: lane.version,
      cliVersion: lane.cliVersion,
      packageManifestSha256: sha256(readFileSync(readInstalledPackage(lane.packageAlias).path)),
      cliManifestSha256: sha256(readFileSync(readInstalledPackage(lane.cliPackageAlias).path)),
      corpora: {},
    };
    for (const corpus of corpora) {
      process.stdout.write(`[measuring] ${lane.id}/${corpus.id}\n`);
      const html = materializeCorpus(corpus);
      const metrics = corpusMetrics(html);
      if (JSON.stringify(metrics) !== JSON.stringify(corpus.expected)) {
        fail(`${corpus.id} materialization drifted from oracle`);
      }
      const prepared = prepareLaneWorkspace(runDirectory, lane, corpus, html, tools.pliego);
      results[lane.id].corpora[corpus.id] = {
        fixture: metrics,
        ...measureComparison(prepared, tools, html, options.warmups, options.pairs),
      };
    }
  }
  const stateAfter = gitState();
  if (JSON.stringify(stateBefore) !== JSON.stringify(stateAfter)) fail("source tree changed during measurement");
  const result = {
    schemaVersion: 2,
    benchmark: "PliegoCSS Benchmark Authority v2 paired competitor oracle",
    generatedAtUtc: new Date().toISOString(),
    mode: options.evidence ? "immutable-evidence" : options.smoke ? "smoke" : "machine-local",
    authority: {
      path: relative(repositoryRoot, authorityPath).replaceAll("\\", "/"),
      sha256: sha256(readFileSync(authorityPath)),
      observedAtUtc: authority.observedAtUtc,
      expiresAtUtc: authority.expiresAtUtc,
      primaryLane: authority.primaryLane,
      resetContract: authority.resetContract,
    },
    provenance: {
      git: stateBefore,
      harness: { path: relative(repositoryRoot, harnessPath).replaceAll("\\", "/"), sha256: sha256(readFileSync(harnessPath)) },
      processMetricsRunner: { path: relative(repositoryRoot, runnerSourcePath).replaceAll("\\", "/"), sha256: sha256(readFileSync(runnerSourcePath)) },
      packageJsonSha256: sha256(readFileSync(packagePath)),
      pnpmLockSha256: sha256(readFileSync(lockPath)),
      executableSha256: sha256(readFileSync(tools.pliego)),
    },
    methodology: {
      design: authority.methodology.design,
      warmupPairs: options.warmups,
      measuredPairs: options.pairs,
      pairOrder: authority.methodology.pairOrder,
      freshProcess: true,
      completeSamples: true,
      freshOutputVerification: true,
      memoryMetric: authority.methodology.memoryMetric,
      compression: authority.methodology.compression,
    },
    environment: {
      os: { platform: process.platform, type: osType(), release: osRelease(), version: osVersion(), architecture: process.arch },
      cpu: { models: [...new Set(cpus().map((cpu) => cpu.model))], logicalCoreCount: cpus().length, availableParallelism: availableParallelism() },
      totalMemoryBytes: totalmem(),
      node: process.version,
      zlib: process.versions.zlib,
      toolchain: toolchainValue,
      cargoTarget: relative(repositoryRoot, tools.cargoTarget).replaceAll("\\", "/"),
      powerPlan: capturePowerPlan(),
      securityTooling: captureSecurityTooling(options.evidence),
    },
    lanes: results,
    claimBoundary: authority.methodology.scorePolicy,
  };
  const serialized = `${JSON.stringify(result, null, 2)}\n`;
  mkdirSync(dirname(localResultPath), { recursive: true });
  writeFileSync(localResultPath, serialized);
  if (options.evidence) {
    mkdirSync(dirname(options.evidence), { recursive: true });
    writeFileSync(options.evidence, serialized, { flag: "wx" });
  }
  process.stdout.write(
    `${JSON.stringify({ schemaVersion: 2, status: "passed", mode: result.mode, lanes: Object.keys(results), corpora: options.corpusIds, pairs: options.pairs, result: relative(repositoryRoot, options.evidence ?? localResultPath).replaceAll("\\", "/") }, null, 2)}\n`,
  );
} finally {
  const child = relative(benchmarkRoot, runDirectory);
  if (!child || child === ".." || child.startsWith(`..${sep}`) || isAbsolute(child) || !basename(runDirectory).startsWith("run-")) {
    fail(`refusing to remove unsafe benchmark run directory ${runDirectory}`);
  }
  rmSync(runDirectory, { recursive: true, force: true, maxRetries: 3, retryDelay: 100 });
}
