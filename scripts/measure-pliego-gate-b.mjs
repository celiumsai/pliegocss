import { createHash, randomUUID } from "node:crypto";
import { spawnSync } from "node:child_process";
import { lstat, mkdir, mkdtemp, open, readFile, realpath, rm, writeFile } from "node:fs/promises";
import {
  availableParallelism,
  cpus,
  release as osRelease,
  tmpdir,
  totalmem,
  type as osType,
  version as osVersion,
} from "node:os";
import { basename, dirname, extname, isAbsolute, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";

const harnessPath = fileURLToPath(import.meta.url);
const scriptDirectory = dirname(harnessPath);
const repositoryRoot = resolve(scriptDirectory, "..");
const fixturePath = join(repositoryRoot, "benchmarks", "tailwind-v4", "fixtures.html");
const resultsPath = join(repositoryRoot, "benchmarks", "results", "pliego-gate-b.local.json");
const evidenceRoot = join(repositoryRoot, "benchmarks", "evidence");
const cargoTargetDirectory = join(repositoryRoot, "target", "benchmarks", "pliego-gate-b");
const cargoHomeDirectory = join(repositoryRoot, "target", "benchmarks", "cargo-home-gate-b");
const runLockPath = join(repositoryRoot, "target", "benchmarks", "locks", "pliego-gate-b.lock");
const evidencePath = parseEvidencePath(process.argv.slice(2));
const measuredProcessCount = 30;
const warmupProcessCount = 5;
const expectedClassAttributeCount = 44;
const requiredVariants = [
  "disabled",
  "focus",
  "focus-visible",
  "hover",
  "lg",
  "md",
  "placeholder",
  "sm",
];
const emittedVariantMarkers = {
  disabled: ":disabled{",
  focus: ":focus{",
  "focus-visible": ":focus-visible{",
  hover: ":hover{",
  lg: "@media (width>=64rem)",
  md: "@media (width>=48rem)",
  placeholder: "::placeholder{",
  sm: "@media (width>=40rem)",
};
const frozenTailwind = {
  version: "4.3.2",
  generated_at: "2026-07-12T22:30:41.861Z",
  profiles: {
    "full-complete": {
      median_ms: 210.202,
      min_ms: 187.341,
      p95_ms: 318.531,
      css_raw_bytes: 12_377,
      css_gzip_bytes: 3_333,
      html_gzip_bytes: 1_506,
      transfer_gzip_bytes: 4_839,
    },
    "no-preflight-complete": {
      median_ms: 204.826,
      min_ms: 185.374,
      p95_ms: 418.003,
      css_raw_bytes: 8_618,
      css_gzip_bytes: 2_327,
      html_gzip_bytes: 1_506,
      transfer_gzip_bytes: 3_833,
    },
  },
};
const cargoInfluencePatterns = [
  /^CARGO_BUILD_/iu,
  /^CARGO_ENCODED_RUSTFLAGS$/iu,
  /^CARGO_HOME$/iu,
  /^CARGO_INCREMENTAL$/iu,
  /^CARGO_PROFILE_/iu,
  /^CARGO_TARGET_/iu,
  /^CARGO_TERM_/iu,
  /^RUSTC$/iu,
  /^RUSTC_BOOTSTRAP$/iu,
  /^RUSTC_WORKSPACE_WRAPPER$/iu,
  /^RUSTC_WRAPPER$/iu,
  /^RUSTDOCFLAGS$/iu,
  /^RUSTFLAGS$/iu,
  /^RUSTUP_TOOLCHAIN$/iu,
];

function sanitizedCargoEnvironmentKeys() {
  return Object.keys(process.env)
    .filter((key) => cargoInfluencePatterns.some((pattern) => pattern.test(key)))
    .sort();
}

function controlledCargoEnvironment() {
  const environment = { ...process.env };
  for (const key of sanitizedCargoEnvironmentKeys()) {
    delete environment[key];
  }
  return {
    ...environment,
    CARGO_HOME: cargoHomeDirectory,
    CARGO_INCREMENTAL: "0",
    CARGO_TARGET_DIR: cargoTargetDirectory,
    CARGO_TERM_COLOR: "never",
  };
}

function parseEvidencePath(arguments_) {
  if (arguments_.length === 0) {
    return null;
  }
  if (arguments_.length !== 2 || arguments_[0] !== "--evidence" || !arguments_[1]) {
    throw new Error(
      "usage: node scripts/measure-pliego-gate-b.mjs [--evidence benchmarks/evidence/<snapshot>.json]",
    );
  }
  if (arguments_[1].includes("\0")) {
    throw new Error("evidence path contains a NUL byte");
  }
  const candidate = resolve(repositoryRoot, arguments_[1]);
  const child = relative(evidenceRoot, candidate);
  if (
    !child ||
    child === ".." ||
    child.startsWith(`..${sep}`) ||
    isAbsolute(child) ||
    extname(candidate).toLowerCase() !== ".json"
  ) {
    throw new Error("--evidence must name a .json file below benchmarks/evidence/");
  }
  return candidate;
}

function run(command, commandArguments, options = {}) {
  const result = spawnSync(command, commandArguments, {
    cwd: repositoryRoot,
    encoding: "utf8",
    windowsHide: true,
    ...options,
  });
  if (result.error) {
    throw result.error;
  }
  if (result.status !== 0) {
    throw new Error(
      `${command} ${commandArguments.join(" ")} failed with exit code ${result.status}\n${result.stderr}`,
    );
  }
  return result;
}

function probeCommand(command, commandArguments) {
  const result = spawnSync(command, commandArguments, {
    cwd: repositoryRoot,
    encoding: "utf8",
    windowsHide: true,
  });
  const source = `${command} ${commandArguments.join(" ")}`;
  if (result.error) {
    return {
      status: result.error.code === "ENOENT" ? "unavailable" : "error",
      source,
      value: null,
      detail: result.error.message,
    };
  }
  if (result.status !== 0) {
    return {
      status: "error",
      source,
      value: null,
      exit_code: result.status,
      detail: `${result.stderr ?? ""}`.trim().slice(0, 2_048) || null,
    };
  }
  const value = `${result.stdout ?? ""}`.trim().replaceAll("\r\n", "\n");
  return {
    status: value ? "recorded" : "empty",
    source,
    value: value || null,
  };
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function round(value) {
  return Number(value.toFixed(3));
}

function percentile(sorted, percentage) {
  const index = Math.ceil((percentage / 100) * sorted.length) - 1;
  return sorted[Math.max(0, Math.min(index, sorted.length - 1))];
}

function conventionalMedian(sorted) {
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 0
    ? (sorted[middle - 1] + sorted[middle]) / 2
    : sorted[middle];
}

function summarize(samples) {
  const sorted = [...samples].sort((left, right) => left - right);
  const mean = sorted.reduce((sum, sample) => sum + sample, 0) / sorted.length;
  const median = conventionalMedian(sorted);
  const absoluteDeviations = sorted
    .map((sample) => Math.abs(sample - median))
    .sort((left, right) => left - right);
  const variance =
    sorted.reduce((sum, sample) => sum + (sample - mean) ** 2, 0) / sorted.length;
  return {
    runs: sorted.length,
    min_ms: round(sorted[0]),
    median_ms: round(median),
    mean_ms: round(mean),
    p95_ms: round(percentile(sorted, 95)),
    max_ms: round(sorted.at(-1)),
    median_absolute_deviation_ms: round(conventionalMedian(absoluteDeviations)),
    standard_deviation_ms: round(Math.sqrt(variance)),
    samples_ms: samples.map(round),
  };
}

function verifySummaryContract() {
  const even = summarize([4, 1, 3, 2]);
  const odd = summarize([3, 1, 2]);
  if (
    even.median_ms !== 2.5 ||
    even.median_absolute_deviation_ms !== 1 ||
    odd.median_ms !== 2 ||
    odd.median_absolute_deviation_ms !== 1
  ) {
    throw new Error("benchmark summary contract is not conventional median/MAD");
  }
}

verifySummaryContract();

function toolVersion(command, commandArguments) {
  return run(command, commandArguments).stdout.trim();
}

function captureToolchain() {
  const cargo = toolVersion("cargo", ["+1.96.0", "--version"]);
  const rustc = toolVersion("rustc", ["+1.96.0", "--version"]);
  const rustcVerbose = toolVersion("rustc", ["+1.96.0", "-vV"]);
  if (!/^cargo 1\.96\.0\b/u.test(cargo) || !/^rustc 1\.96\.0\b/u.test(rustc)) {
    throw new Error(`benchmark requires exact Cargo/rustc 1.96.0; found ${cargo} / ${rustc}`);
  }
  const hostTarget = /^host: (\S+)$/mu.exec(rustcVerbose)?.[1];
  if (!hostTarget) {
    throw new Error(`cannot determine the Rust 1.96.0 host target from:\n${rustcVerbose}`);
  }
  return { cargo, rustc, host_target: hostTarget };
}

function captureGitState() {
  const commit = toolVersion("git", ["rev-parse", "HEAD"]);
  if (!/^[0-9a-f]{40}$/u.test(commit)) {
    throw new Error(`unexpected Git commit identity: ${commit}`);
  }
  const status = run("git", ["status", "--porcelain=v1", "-z", "--untracked-files=all"]).stdout;
  const entries = status.split("\0").filter(Boolean);
  return {
    commit,
    dirty: entries.length > 0,
    status_entry_count: entries.length,
    status_sha256: sha256(Buffer.from(status, "utf8")),
  };
}

function trackedInputProvenance(commit, absolutePath, bytes) {
  const path = relative(repositoryRoot, absolutePath).replaceAll("\\", "/");
  if (!path || path.startsWith("../") || isAbsolute(path)) {
    throw new Error(`tracked benchmark input escapes the repository: ${absolutePath}`);
  }
  if (evidencePath) {
    const result = spawnSync("git", ["show", "--no-textconv", `${commit}:${path}`], {
      cwd: repositoryRoot,
      encoding: null,
      maxBuffer: 16 * 1024 * 1024,
      windowsHide: true,
    });
    if (result.error) {
      throw new Error(`cannot read tracked benchmark input ${commit}:${path}: ${result.error.message}`);
    }
    if (result.status !== 0) {
      throw new Error(
        `cannot read tracked benchmark input ${commit}:${path}: ${result.stderr.toString("utf8").trim()}`,
      );
    }
    if (!result.stdout.equals(bytes)) {
      throw new Error(
        `benchmark input ${path} differs byte-for-byte from ${commit}; normalize or restore it before recording evidence`,
      );
    }
  }
  return { path, sha256: sha256(bytes) };
}

function securityToolingMetadata() {
  const reportedStatus = process.env.PLIEGO_BENCH_SECURITY_TOOLING_STATUS?.trim().toLowerCase();
  const tooling = process.env.PLIEGO_BENCH_SECURITY_TOOLING?.trim() || null;
  if (!reportedStatus) {
    if (tooling) {
      throw new Error(
        "PLIEGO_BENCH_SECURITY_TOOLING requires PLIEGO_BENCH_SECURITY_TOOLING_STATUS",
      );
    }
    if (evidencePath) {
      throw new Error("--evidence requires PLIEGO_BENCH_SECURITY_TOOLING_STATUS");
    }
    return {
      status: "not-reported",
      tooling: null,
      source: "PLIEGO_BENCH_SECURITY_TOOLING_STATUS/PLIEGO_BENCH_SECURITY_TOOLING",
    };
  }
  const allowed = new Set(["active", "disabled", "not-installed", "unknown"]);
  if (!allowed.has(reportedStatus)) {
    throw new Error(
      "PLIEGO_BENCH_SECURITY_TOOLING_STATUS must be active, disabled, not-installed, or unknown",
    );
  }
  if ((reportedStatus === "active" || reportedStatus === "disabled") && !tooling) {
    throw new Error(`${reportedStatus} security tooling requires PLIEGO_BENCH_SECURITY_TOOLING`);
  }
  return {
    status: reportedStatus,
    tooling,
    source: "environment",
  };
}

async function powerPlanMetadata() {
  const declared = process.env.PLIEGO_BENCH_POWER_PLAN?.trim();
  if (declared) {
    return { status: "declared", source: "PLIEGO_BENCH_POWER_PLAN", value: declared };
  }
  if (process.platform === "win32") {
    return probeCommand("powercfg", ["/getactivescheme"]);
  }
  if (process.platform === "darwin") {
    return probeCommand("pmset", ["-g"]);
  }
  if (process.platform === "linux") {
    const profile = probeCommand("powerprofilesctl", ["get"]);
    if (profile.status === "recorded") {
      return profile;
    }
    const governorPath = "/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor";
    try {
      const governor = (await readFile(governorPath, "utf8")).trim();
      return {
        status: governor ? "recorded" : "empty",
        source: governorPath,
        value: governor || null,
        fallback_from: profile,
      };
    } catch (error) {
      return {
        status: error.code === "ENOENT" ? "unavailable" : "error",
        source: "powerprofilesctl/sysfs",
        value: null,
        detail: error.message,
        powerprofilesctl: profile,
      };
    }
  }
  return {
    status: "unsupported",
    source: null,
    value: null,
    detail: `no power-plan probe for ${process.platform}`,
  };
}

async function captureEnvironment(toolchain) {
  const cpuRecords = cpus();
  return {
    os: {
      platform: process.platform,
      type: osType(),
      release: osRelease(),
      version: osVersion(),
      architecture: process.arch,
    },
    cpu: {
      models: [...new Set(cpuRecords.map((cpu) => cpu.model))],
      logical_core_count: cpuRecords.length,
      available_parallelism: availableParallelism(),
    },
    memory: { total_bytes: totalmem() },
    node: process.version,
    zlib: process.versions.zlib,
    toolchain,
    build: {
      host_target: toolchain.host_target,
      cargo_target_dir: relative(repositoryRoot, cargoTargetDirectory).replaceAll("\\", "/"),
      cargo_incremental: false,
      cargo_home: relative(repositoryRoot, cargoHomeDirectory).replaceAll("\\", "/"),
      cargo_home_policy: "isolated benchmark directory without user Cargo configuration",
      sanitized_environment_keys: sanitizedCargoEnvironmentKeys(),
    },
    power_plan: await powerPlanMetadata(),
    security_tooling: securityToolingMetadata(),
  };
}

async function prepareCargoHome() {
  await mkdir(cargoHomeDirectory, { recursive: true });
  const info = await lstat(cargoHomeDirectory);
  const targetRoot = await realpath(join(repositoryRoot, "target"));
  if (
    !info.isDirectory() ||
    info.isSymbolicLink() ||
    !pathIsWithin(targetRoot, await realpath(cargoHomeDirectory))
  ) {
    throw new Error("benchmark CARGO_HOME must be a real directory below target/");
  }
  for (const name of ["config", "config.toml"]) {
    if (await lstatOrNull(join(cargoHomeDirectory, name))) {
      throw new Error(`benchmark CARGO_HOME must not contain user configuration: ${name}`);
    }
  }
}

async function acquireRunLock() {
  await mkdir(dirname(runLockPath), { recursive: true });
  const token = randomUUID();
  let handle;
  try {
    handle = await open(runLockPath, "wx", 0o600);
    await handle.writeFile(
      `${JSON.stringify({ pid: process.pid, started_at: new Date().toISOString(), token })}\n`,
      "utf8",
    );
  } catch (error) {
    if (handle) {
      await handle.close();
      await rm(runLockPath, { force: true });
    }
    if (error.code === "EEXIST") {
      throw new Error(`another Gate B benchmark may be running: ${runLockPath}`);
    }
    throw error;
  }
  return { handle, token };
}

async function releaseRunLock(lock) {
  const current = JSON.parse(await readFile(runLockPath, "utf8"));
  await lock.handle.close();
  if (current.token !== lock.token) {
    throw new Error(`Gate B run lock ownership changed: ${runLockPath}`);
  }
  await rm(runLockPath);
}

function pathIsWithin(root, candidate) {
  const child = relative(root, candidate);
  return child === "" || (child !== ".." && !child.startsWith(`..${sep}`) && !isAbsolute(child));
}

async function lstatOrNull(path) {
  try {
    return await lstat(path);
  } catch (error) {
    if (error.code === "ENOENT") {
      return null;
    }
    throw error;
  }
}

async function prepareEvidenceDestination(destination) {
  if (!destination) {
    return;
  }
  await mkdir(evidenceRoot, { recursive: true });
  const rootInfo = await lstat(evidenceRoot);
  if (!rootInfo.isDirectory() || rootInfo.isSymbolicLink()) {
    throw new Error("benchmarks/evidence must be a real directory, not a link");
  }
  const physicalRoot = await realpath(evidenceRoot);
  const parts = relative(evidenceRoot, destination).split(sep);
  let current = evidenceRoot;
  for (const part of parts.slice(0, -1)) {
    current = join(current, part);
    let info = await lstatOrNull(current);
    if (!info) {
      await mkdir(current);
      info = await lstat(current);
    }
    if (!info.isDirectory() || info.isSymbolicLink()) {
      throw new Error(`evidence parent is not a real directory: ${current}`);
    }
    const physical = await realpath(current);
    if (!pathIsWithin(physicalRoot, physical)) {
      throw new Error(`evidence parent escapes benchmarks/evidence: ${current}`);
    }
  }
  if (await lstatOrNull(destination)) {
    throw new Error(`evidence snapshots are immutable and already exist: ${destination}`);
  }
}

async function safelyRemoveWorkspace(path, prefix) {
  const temporaryRoot = resolve(tmpdir());
  const candidate = resolve(path);
  const child = relative(temporaryRoot, candidate);
  if (
    !child ||
    child === ".." ||
    child.startsWith(`..${sep}`) ||
    isAbsolute(child) ||
    !basename(candidate).startsWith(prefix)
  ) {
    throw new Error(`refusing to remove unsafe benchmark workspace: ${candidate}`);
  }
  await rm(candidate, { recursive: true, force: true, maxRetries: 3, retryDelay: 100 });
}

function sizeReport(bytes, cssGzipBytes) {
  const gzipBytes = gzipSync(bytes, { level: 9 }).byteLength;
  return {
    raw_bytes: bytes.byteLength,
    gzip_bytes: gzipBytes,
    transfer_html_css_gzip_bytes: gzipBytes + cssGzipBytes,
    sha256: sha256(bytes),
  };
}

function percentChange(reference, candidate) {
  return round(((candidate - reference) / reference) * 100);
}

function replaceClassValues(html, classesBySource) {
  let replacements = 0;
  const rendered = html.replace(/class="([^"]*)"/gu, (_, source) => {
    const className = classesBySource.get(source);
    if (typeof className !== "string") {
      throw new Error(`manifest has no className for class attribute: ${source}`);
    }
    replacements += 1;
    return `class="${className}"`;
  });
  if (replacements !== expectedClassAttributeCount) {
    throw new Error(
      `expected ${expectedClassAttributeCount} class replacements, performed ${replacements}`,
    );
  }
  const withoutClassValues = (value) => value.replace(/class="[^"]*"/gu, 'class=""');
  if (withoutClassValues(html) !== withoutClassValues(rendered)) {
    throw new Error("generated HTML changed bytes outside class attribute values");
  }
  return { bytes: Buffer.from(rendered, "utf8"), replacements };
}

const runLock = await acquireRunLock();
try {
const harnessBytesBefore = await readFile(harnessPath);
const gitStateBefore = captureGitState();
if (evidencePath && gitStateBefore.dirty) {
  throw new Error("--evidence requires a clean Git worktree");
}
const harnessProvenance = trackedInputProvenance(
  gitStateBefore.commit,
  harnessPath,
  harnessBytesBefore,
);
const toolchain = captureToolchain();
const environment = await captureEnvironment(toolchain);
const cargoEnvironment = controlledCargoEnvironment();
const executablePath = join(
  cargoTargetDirectory,
  toolchain.host_target,
  "release",
  process.platform === "win32" ? "pliego-cssc.exe" : "pliego-cssc",
);
await prepareEvidenceDestination(evidencePath);
await prepareCargoHome();

console.log("Building pliego-cssc in release mode (excluded from timings)...");
run(
  "cargo",
  [
    "+1.96.0",
    "build",
    "--release",
    "--locked",
    "--target",
    toolchain.host_target,
    "-p",
    "pliego-cssc",
  ],
  { env: cargoEnvironment },
);

const workspace = await mkdtemp(join(tmpdir(), "pliego-gate-b-"));
try {
  const fixtureBefore = await readFile(fixturePath);
  const fixtureProvenance = trackedInputProvenance(
    gitStateBefore.commit,
    fixturePath,
    fixtureBefore,
  );
  const fixtureHtml = fixtureBefore.toString("utf8");
  const classValues = [...fixtureHtml.matchAll(/class="([^"]*)"/gu)].map(
    (match) => match[1],
  );
  if (classValues.length !== expectedClassAttributeCount) {
    throw new Error(
      `complete fixture must contain ${expectedClassAttributeCount} class attributes, found ${classValues.length}`,
    );
  }

  const uniqueStyleInputs = new Set(classValues);
  const utilityTokens = classValues.flatMap((value) => value.split(/\s+/u));
  const variants = [...new Set(
    utilityTokens.flatMap((token) => {
      const segments = token.split(":");
      return segments.length > 1 ? segments.slice(0, -1) : [];
    }),
  )].sort();
  const missingVariants = requiredVariants.filter((variant) => !variants.includes(variant));
  if (missingVariants.length > 0) {
    throw new Error(`complete fixture is missing required variants: ${missingVariants.join(", ")}`);
  }

  const inputPath = join(workspace, "complete-styles.txt");
  const inputBytes = Buffer.from(`${classValues.join("\n")}\n`, "utf8");
  await writeFile(inputPath, inputBytes);

  const warmupOutputPath = join(workspace, "warmup.css");
  for (let index = 0; index < warmupProcessCount; index += 1) {
    run(executablePath, [
      "compile",
      "--input",
      inputPath,
      "--seed",
      "--theme",
      "--output",
      warmupOutputPath,
    ]);
  }

  const elapsedMilliseconds = [];
  const outputs = [];
  const hashes = [];
  for (let index = 0; index < measuredProcessCount; index += 1) {
    const outputPath = join(workspace, `gate-b-${index}.css`);
    const startedAt = process.hrtime.bigint();
    run(executablePath, [
      "compile",
      "--input",
      inputPath,
      "--seed",
      "--theme",
      "--output",
      outputPath,
    ]);
    const stoppedAt = process.hrtime.bigint();
    const css = await readFile(outputPath);
    elapsedMilliseconds.push(Number(stoppedAt - startedAt) / 1_000_000);
    outputs.push(css);
    hashes.push(sha256(css));
  }

  const uniqueHashes = [...new Set(hashes)];
  if (uniqueHashes.length !== 1) {
    throw new Error(`non-deterministic output: observed ${uniqueHashes.length} SHA-256 hashes`);
  }
  const fixtureAfter = await readFile(fixturePath);
  if (!fixtureBefore.equals(fixtureAfter)) {
    throw new Error("tracked complete fixture changed during measurement");
  }

  const css = outputs[0];
  const cssText = css.toString("utf8");
  const missingEmittedVariants = Object.entries(emittedVariantMarkers)
    .filter(([, marker]) => !cssText.includes(marker))
    .map(([variant]) => variant);
  if (missingEmittedVariants.length > 0) {
    throw new Error(
      `compiled CSS is missing markers for variants: ${missingEmittedVariants.join(", ")}`,
    );
  }
  const compressedCss = gzipSync(css, { level: 9 });
  const manifestPath = join(workspace, "manifest.json");
  const manifestOutputPath = join(workspace, "manifest.css");
  run(executablePath, [
    "compile",
    "--input",
    inputPath,
    "--seed",
    "--theme",
    "--output",
    manifestOutputPath,
    "--manifest",
    manifestPath,
  ]);
  const manifestBytes = await readFile(manifestPath);
  const manifestCss = await readFile(manifestOutputPath);
  if (!css.equals(manifestCss)) {
    throw new Error("manifest generation changed the compiled CSS artifact");
  }
  const manifest = JSON.parse(manifestBytes.toString("utf8"));
  if (
    manifest.schemaVersion !== 3 ||
    manifest.styleIdFormatVersion !== 2 ||
    manifest.classNameFormatVersion !== 1 ||
    manifest.themeIdFormatVersion !== 2
  ) {
    throw new Error(
      `expected manifest schema/identity formats 3/2/1/2, received ${manifest.schemaVersion}/${manifest.styleIdFormatVersion}/${manifest.classNameFormatVersion}/${manifest.themeIdFormatVersion}`,
    );
  }
  const classesBySource = new Map();
  for (const style of manifest.styles) {
    for (const origin of style.origins) {
      const existing = classesBySource.get(origin.source);
      if (existing && existing !== style.className) {
        throw new Error(`manifest maps ${origin.source} to multiple classes`);
      }
      classesBySource.set(origin.source, style.className);
    }
  }
  if (classesBySource.size !== uniqueStyleInputs.size) {
    throw new Error(
      `expected ${uniqueStyleInputs.size} manifest styles, received ${classesBySource.size}`,
    );
  }
  if (new Set(classesBySource.values()).size !== classesBySource.size) {
    throw new Error("manifest contains a generated class collision");
  }

  const generatedHtml = replaceClassValues(fixtureHtml, classesBySource);
  const utilityHtmlReport = sizeReport(fixtureBefore, compressedCss.byteLength);
  const generatedClassHtmlReport = sizeReport(generatedHtml.bytes, compressedCss.byteLength);
  const timing = summarize(elapsedMilliseconds);
  const output = {
    css_raw_bytes: css.byteLength,
    css_gzip_bytes: compressedCss.byteLength,
    css_sha256: uniqueHashes[0],
    executable_sha256: sha256(await readFile(executablePath)),
    manifest_sha256: sha256(manifestBytes),
    transfer_gzip_bytes: generatedClassHtmlReport.transfer_html_css_gzip_bytes,
    html_unchanged_outside_class_values: true,
    html: {
      utility_strings: utilityHtmlReport,
      generated_classes: generatedClassHtmlReport,
    },
  };
  const comparisons = Object.fromEntries(
    Object.entries(frozenTailwind.profiles).map(([profile, baseline]) => [
      profile,
      {
        median_time_change_percent: percentChange(baseline.median_ms, timing.median_ms),
        median_speed_ratio: round(baseline.median_ms / timing.median_ms),
        css_raw_change_percent: percentChange(baseline.css_raw_bytes, output.css_raw_bytes),
        css_gzip_change_percent: percentChange(baseline.css_gzip_bytes, output.css_gzip_bytes),
        transfer_gzip_change_percent: percentChange(
          baseline.transfer_gzip_bytes,
          output.transfer_gzip_bytes,
        ),
      },
    ]),
  );
  const harnessBytesAfter = await readFile(harnessPath);
  if (!harnessBytesBefore.equals(harnessBytesAfter)) {
    throw new Error("benchmark harness changed during measurement");
  }
  const gitStateAfter = captureGitState();
  if (JSON.stringify(gitStateBefore) !== JSON.stringify(gitStateAfter)) {
    throw new Error("Git commit or worktree state changed during measurement");
  }
  const result = {
    schema_version: 3,
    benchmark: "PliegoCSS Gate B complete-fixture fresh-process compile",
    profile: "complete",
    generated_at: new Date().toISOString(),
    provenance: {
      git: gitStateBefore,
      harness: harnessProvenance,
      tracked_inputs: [fixtureProvenance],
    },
    fixture: {
      html: "benchmarks/tailwind-v4/fixtures.html",
      groups: ["button", "card", "navbar", "form", "dashboard"],
      class_attribute_count: classValues.length,
      class_attributes_resolved: generatedHtml.replacements,
      all_class_attributes_resolved: generatedHtml.replacements === classValues.length,
      unique_style_inputs: uniqueStyleInputs.size,
      utility_occurrences: utilityTokens.length,
      unique_utility_tokens: new Set(utilityTokens).size,
      variants,
      required_variants: requiredVariants,
      emitted_variant_markers_verified: Object.keys(emittedVariantMarkers).sort(),
      theme_included: true,
      html_sha256: sha256(fixtureBefore),
      derived_input_sha256: sha256(inputBytes),
    },
    environment,
    command:
      "pliego-cssc compile --input <derived-from-complete-class-attributes> --seed --theme --output <temporary-file>",
    statistics: {
      median: "average of the two middle values for even samples",
      p95: "nearest-rank",
      median_absolute_deviation: "conventional median of absolute deviations from the median",
    },
    timing: {
      warmup_processes: warmupProcessCount,
      ...timing,
    },
    determinism: {
      verified_runs: measuredProcessCount,
      deterministic: true,
      sha256: uniqueHashes[0],
    },
    output,
    frozen_tailwind_baseline: frozenTailwind,
    comparisons,
    caveats: [
      "Fresh-process timings retain normal OS caches and exclude the release build.",
      "Manifest generation and HTML rewriting are verified outside the timed region.",
      "PliegoCSS does not emit reset/preflight; no-preflight-complete is the primary payload comparison.",
      "The generated-class HTML and Tailwind utility HTML preserve the same DOM but have different class-value bytes.",
    ],
  };

  const serializedResult = `${JSON.stringify(result, null, 2)}\n`;
  await mkdir(dirname(resultsPath), { recursive: true });
  await writeFile(resultsPath, serializedResult, "utf8");
  if (evidencePath) {
    await writeFile(evidencePath, serializedResult, { encoding: "utf8", flag: "wx" });
  }

  console.log(
    `Gate B complete: ${generatedHtml.replacements}/${classValues.length} class attributes resolved (${uniqueStyleInputs.size} unique styles), ${warmupProcessCount} warmups + ${measuredProcessCount} measured fresh processes`,
  );
  console.log(`Variants: ${variants.join(", ")}`);
  console.log(
    `Timing: min ${timing.min_ms} ms | median ${timing.median_ms} ms | p95 ${timing.p95_ms} ms | max ${timing.max_ms} ms`,
  );
  console.log(`CSS: ${output.css_raw_bytes} B raw | ${output.css_gzip_bytes} B gzip`);
  console.log(
    `Generated HTML: ${generatedClassHtmlReport.raw_bytes} B raw | ${generatedClassHtmlReport.gzip_bytes} B gzip | ${generatedClassHtmlReport.transfer_html_css_gzip_bytes} B transfer`,
  );
  console.log(
    `Determinism: ${measuredProcessCount}/${measuredProcessCount} identical (${uniqueHashes[0]})`,
  );
  console.log(`Results: ${resultsPath}`);
  if (evidencePath) {
    console.log(`Evidence: ${evidencePath}`);
  }
} finally {
  await safelyRemoveWorkspace(workspace, "pliego-gate-b-");
}
} finally {
  await releaseRunLock(runLock);
}
