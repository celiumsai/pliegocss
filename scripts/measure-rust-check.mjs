import { spawnSync } from "node:child_process";
import { createHash, randomUUID } from "node:crypto";
import {
  closeSync,
  lstatSync,
  mkdirSync,
  openSync,
  readFileSync,
  realpathSync,
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
import { dirname, extname, isAbsolute, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const harnessPath = fileURLToPath(import.meta.url);
const root = resolve(dirname(harnessPath), "..");
const runtime = join(root, "benchmarks", "rust-check", "runtime");
const lockTemplatesDirectory = join(root, "benchmarks", "rust-check", "locks");
const resultsPath = join(root, "benchmarks", "results", "rust-check.local.json");
const evidenceRoot = join(root, "benchmarks", "evidence");
const runLockPath = join(root, "target", "benchmarks", "rust-check", "run.lock");
const cargoHomeDirectory = join(root, "target", "benchmarks", "rust-check", "cargo-home");
const toolchainVersion = "1.85.0";
const cargoToolchain = `+${toolchainVersion}`;
const commandTimeoutMs = 120_000;
const coldPairCount = 6;
const noopPairCount = 30;
const changedPairCount = 30;
const baseGapPx = 4;
const evidencePath = parseEvidencePath(process.argv.slice(2));
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

function parseEvidencePath(arguments_) {
  if (arguments_.length === 0) {
    return null;
  }
  if (arguments_.length !== 2 || arguments_[0] !== "--evidence" || !arguments_[1]) {
    throw new Error(
      "usage: node scripts/measure-rust-check.mjs [--evidence benchmarks/evidence/<snapshot>.json]",
    );
  }
  if (arguments_[1].includes("\0")) {
    throw new Error("evidence path contains a NUL byte");
  }
  const candidate = resolve(root, arguments_[1]);
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

function lstatOrNull(path) {
  try {
    return lstatSync(path);
  } catch (error) {
    if (error.code === "ENOENT") {
      return null;
    }
    throw error;
  }
}

function pathIsWithin(parent, candidate) {
  const child = relative(parent, candidate);
  return child === "" || (child !== ".." && !child.startsWith(`..${sep}`) && !isAbsolute(child));
}

function prepareEvidenceDestination(destination) {
  if (!destination) {
    return;
  }
  mkdirSync(evidenceRoot, { recursive: true });
  const rootInfo = lstatSync(evidenceRoot);
  if (!rootInfo.isDirectory() || rootInfo.isSymbolicLink()) {
    throw new Error("benchmarks/evidence must be a real directory, not a link");
  }
  const physicalRoot = realpathSync(evidenceRoot);
  const parts = relative(evidenceRoot, destination).split(sep);
  let current = evidenceRoot;
  for (const part of parts.slice(0, -1)) {
    current = join(current, part);
    let info = lstatOrNull(current);
    if (!info) {
      mkdirSync(current);
      info = lstatSync(current);
    }
    if (!info.isDirectory() || info.isSymbolicLink()) {
      throw new Error(`evidence parent is not a real directory: ${current}`);
    }
    if (!pathIsWithin(physicalRoot, realpathSync(current))) {
      throw new Error(`evidence parent escapes benchmarks/evidence: ${current}`);
    }
  }
  if (lstatOrNull(destination)) {
    throw new Error(`evidence snapshots are immutable and already exist: ${destination}`);
  }
}

function acquireRunLock() {
  mkdirSync(dirname(runLockPath), { recursive: true });
  const token = randomUUID();
  let descriptor;
  try {
    descriptor = openSync(runLockPath, "wx", 0o600);
    writeFileSync(
      descriptor,
      `${JSON.stringify({ pid: process.pid, started_at: new Date().toISOString(), token })}\n`,
      "utf8",
    );
  } catch (error) {
    if (descriptor !== undefined) {
      closeSync(descriptor);
      rmSync(runLockPath, { force: true });
    }
    if (error.code === "EEXIST") {
      throw new Error(
        `another rust-check benchmark may be running; refusing to replace lock ${runLockPath}`,
      );
    }
    throw error;
  }
  return { descriptor, token };
}

function releaseRunLock(lock) {
  const expectedToken = lock.token;
  const current = JSON.parse(readFileSync(runLockPath, "utf8"));
  closeSync(lock.descriptor);
  if (current.token !== expectedToken) {
    throw new Error(`rust-check run lock ownership changed; refusing to remove ${runLockPath}`);
  }
  rmSync(runLockPath);
}

function run(command, commandArguments, options = {}) {
  const result = spawnSync(command, commandArguments, {
    cwd: root,
    encoding: "utf8",
    timeout: commandTimeoutMs,
    windowsHide: true,
    ...options,
  });
  const invocation = `${command} ${commandArguments.join(" ")}`;
  if (result.error) {
    if (result.error.code === "ETIMEDOUT") {
      throw new Error(`${invocation} exceeded the ${commandTimeoutMs} ms timeout`);
    }
    throw new Error(`${invocation} failed to start: ${result.error.message}`);
  }
  if (result.status !== 0) {
    throw new Error(
      `${invocation} failed with exit code ${result.status}\n${`${result.stderr ?? ""}`.trim()}`,
    );
  }
  return result;
}

function probeCommand(command, commandArguments) {
  const source = `${command} ${commandArguments.join(" ")}`;
  const result = spawnSync(command, commandArguments, {
    cwd: root,
    encoding: "utf8",
    timeout: commandTimeoutMs,
    windowsHide: true,
  });
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
  return { status: value ? "recorded" : "empty", source, value: value || null };
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

function summarize(samples) {
  const sorted = [...samples].sort((left, right) => left - right);
  const mean = sorted.reduce((sum, sample) => sum + sample, 0) / sorted.length;
  const middle = Math.floor(sorted.length / 2);
  const median =
    sorted.length % 2 === 0 ? (sorted[middle - 1] + sorted[middle]) / 2 : sorted[middle];
  const absoluteDeviations = sorted
    .map((sample) => Math.abs(sample - median))
    .sort((left, right) => left - right);
  const deviationMiddle = Math.floor(absoluteDeviations.length / 2);
  const medianAbsoluteDeviation =
    absoluteDeviations.length % 2 === 0
      ? (absoluteDeviations[deviationMiddle - 1] + absoluteDeviations[deviationMiddle]) / 2
      : absoluteDeviations[deviationMiddle];
  const variance =
    sorted.reduce((sum, sample) => sum + (sample - mean) ** 2, 0) / sorted.length;
  return {
    samples: sorted.length,
    min: round(sorted[0]),
    median: round(median),
    mean: round(mean),
    p95: round(percentile(sorted, 95)),
    max: round(sorted.at(-1)),
    median_absolute_deviation: round(medianAbsoluteDeviation),
    standard_deviation: round(Math.sqrt(variance)),
    values: samples.map(round),
  };
}

function toolVersion(command, commandArguments) {
  return run(command, commandArguments).stdout.trim();
}

function captureToolchain() {
  const cargo = toolVersion("cargo", [cargoToolchain, "--version"]);
  const rustc = toolVersion("rustc", [cargoToolchain, "--version"]);
  const rustcVerbose = toolVersion("rustc", [cargoToolchain, "-vV"]);
  if (!/^cargo 1\.85\.0\b/u.test(cargo) || !/^rustc 1\.85\.0\b/u.test(rustc)) {
    throw new Error(`benchmark requires exact Cargo/rustc 1.85.0; found ${cargo} / ${rustc}`);
  }
  const hostTarget = /^host: (\S+)$/mu.exec(rustcVerbose)?.[1];
  if (!hostTarget) {
    throw new Error(`cannot determine the Rust 1.85.0 host target from:\n${rustcVerbose}`);
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
  const path = relative(root, absolutePath).replaceAll("\\", "/");
  if (!path || path.startsWith("../") || isAbsolute(path)) {
    throw new Error(`tracked benchmark input escapes the repository: ${absolutePath}`);
  }
  if (evidencePath) {
    const result = spawnSync("git", ["show", "--no-textconv", `${commit}:${path}`], {
      cwd: root,
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
  return { status: reportedStatus, tooling, source: "environment" };
}

function powerPlanMetadata() {
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
      const governor = readFileSync(governorPath, "utf8").trim();
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

function sanitizedCargoEnvironmentKeys() {
  return Object.keys(process.env)
    .filter((key) => cargoInfluencePatterns.some((pattern) => pattern.test(key)))
    .sort();
}

function captureEnvironment(toolchain) {
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
      cargo_incremental: true,
      cargo_build_jobs: null,
      cargo_home: relative(root, cargoHomeDirectory).replaceAll("\\", "/"),
      cargo_home_policy: "isolated benchmark directory without user Cargo configuration",
      sanitized_environment_keys: sanitizedCargoEnvironmentKeys(),
      command_timeout_ms: commandTimeoutMs,
    },
    power_plan: powerPlanMetadata(),
    security_tooling: securityToolingMetadata(),
  };
}

function fixture(name, styled) {
  const directory = join(runtime, name);
  const source = join(directory, "src", "main.rs");
  const manifest = join(directory, "Cargo.toml");
  const lockfile = join(directory, "Cargo.lock");
  const lockTemplate = join(lockTemplatesDirectory, `${name}.Cargo.lock`);
  const target = join(runtime, "targets", name);
  mkdirSync(join(directory, "src"), { recursive: true });
  const dependencyPath = relative(directory, join(root, "crates", "pliego-css")).replaceAll(
    "\\",
    "/",
  );
  const dependency = styled
    ? `\n[dependencies]\npliego-css = { path = ${JSON.stringify(dependencyPath)} }\n`
    : "";
  writeFileSync(
    manifest,
    `[package]\nname = "check-${name}"\nversion = "0.0.0"\nedition = "2024"\n\n[workspace]\n${dependency}`,
  );
  const sourceFor = (gap) =>
    styled
      ? `use pliego_css::{Style, pc};\n\nfn card_style() -> Style {\n    pc!("flex items-center gap-[${gap}px] rounded-lg bg-surface p-6")\n}\n\nfn main() {\n    assert!(!card_style().is_empty());\n}\n`
      : `#[derive(Clone, Copy)]\nstruct Style(&'static str);\n\nfn card_style() -> Style {\n    Style("flex items-center gap-[${gap}px] rounded-lg bg-surface p-6")\n}\n\nfn main() {\n    assert!(!card_style().0.is_empty());\n}\n`;
  const body = sourceFor(baseGapPx);
  writeFileSync(source, body);
  const lockBytes = readFileSync(lockTemplate);
  writeFileSync(lockfile, lockBytes);
  return {
    name,
    directory,
    source,
    manifest,
    lockfile,
    lockTemplate,
    lockBytes,
    target,
    body,
    sourceFor,
  };
}

function cargoEnvironment(fixture_) {
  const environment = { ...process.env };
  for (const key of sanitizedCargoEnvironmentKeys()) {
    delete environment[key];
  }
  return {
    ...environment,
    CARGO_HOME: cargoHomeDirectory,
    CARGO_INCREMENTAL: "1",
    CARGO_TARGET_DIR: fixture_.target,
    CARGO_TERM_COLOR: "never",
  };
}

function prepareCargoHome() {
  mkdirSync(cargoHomeDirectory, { recursive: true });
  const info = lstatSync(cargoHomeDirectory);
  const targetRoot = realpathSync(join(root, "target"));
  if (
    !info.isDirectory() ||
    info.isSymbolicLink() ||
    !pathIsWithin(targetRoot, realpathSync(cargoHomeDirectory))
  ) {
    throw new Error("benchmark CARGO_HOME must be a real directory below target/");
  }
  for (const name of ["config", "config.toml"]) {
    if (lstatOrNull(join(cargoHomeDirectory, name))) {
      throw new Error(`benchmark CARGO_HOME must not contain user configuration: ${name}`);
    }
  }
}

function fetchLockedDependencies(fixture_, toolchain) {
  run(
    "cargo",
    [
      cargoToolchain,
      "fetch",
      "--locked",
      "--target",
      toolchain.host_target,
      "--manifest-path",
      fixture_.manifest,
    ],
    { env: cargoEnvironment(fixture_) },
  );
}

function check(fixture_, toolchain) {
  const startedAt = process.hrtime.bigint();
  run(
    "cargo",
    [
      cargoToolchain,
      "check",
      "--quiet",
      "--locked",
      "--offline",
      "--target",
      toolchain.host_target,
      "--manifest-path",
      fixture_.manifest,
    ],
    { env: cargoEnvironment(fixture_) },
  );
  return Number(process.hrtime.bigint() - startedAt);
}

function alternatingOrder(pairIndex) {
  return pairIndex % 2 === 0 ? ["plain", "styled"] : ["styled", "plain"];
}

function measurePair(fixtures, toolchain, pairIndex, mutationGapPx = null) {
  const order = alternatingOrder(pairIndex);
  const elapsed = {};
  for (const fixtureName of order) {
    elapsed[fixtureName] = check(fixtures[fixtureName], toolchain);
  }
  return {
    pairIndex,
    order,
    mutationGapPx,
    plainNs: elapsed.plain,
    styledNs: elapsed.styled,
    deltaNs: elapsed.styled - elapsed.plain,
    deltaPercent: ((elapsed.styled - elapsed.plain) / elapsed.plain) * 100,
  };
}

function scenarioReport(pairs) {
  const plainFirst = pairs.filter((pair) => pair.order[0] === "plain").length;
  return {
    paired_sample_count: pairs.length,
    order_balance: {
      plain_first: plainFirst,
      styled_first: pairs.length - plainFirst,
    },
    plain_ms: summarize(pairs.map((pair) => pair.plainNs / 1_000_000)),
    styled_ms: summarize(pairs.map((pair) => pair.styledNs / 1_000_000)),
    paired_delta_ms: summarize(pairs.map((pair) => pair.deltaNs / 1_000_000)),
    paired_delta_percent: summarize(pairs.map((pair) => pair.deltaPercent)),
    pairs: pairs.map((pair) => ({
      pair: pair.pairIndex + 1,
      order: pair.order,
      mutation_gap_px: pair.mutationGapPx,
      plain_ns: pair.plainNs,
      styled_ns: pair.styledNs,
      delta_ns: pair.deltaNs,
      delta_percent: round(pair.deltaPercent),
    })),
  };
}

function measure(fixtures, toolchain) {
  const cold = [];
  for (let pairIndex = 0; pairIndex < coldPairCount; pairIndex += 1) {
    rmSync(fixtures.plain.target, { recursive: true, force: true });
    rmSync(fixtures.styled.target, { recursive: true, force: true });
    cold.push(measurePair(fixtures, toolchain, pairIndex));
  }

  for (const fixtureName of ["styled", "plain"]) {
    check(fixtures[fixtureName], toolchain);
  }

  const noop = [];
  for (let pairIndex = 0; pairIndex < noopPairCount; pairIndex += 1) {
    noop.push(measurePair(fixtures, toolchain, pairIndex));
  }

  const changed = [];
  for (let pairIndex = 0; pairIndex < changedPairCount; pairIndex += 1) {
    const gap = baseGapPx + pairIndex + 1;
    for (const fixture_ of Object.values(fixtures)) {
      const nextSource = fixture_.sourceFor(gap);
      if (readFileSync(fixture_.source, "utf8") === nextSource) {
        throw new Error(`${fixture_.name} changed fixture did not receive a new source value`);
      }
      writeFileSync(fixture_.source, nextSource);
      if (readFileSync(fixture_.source, "utf8") !== nextSource) {
        throw new Error(`${fixture_.name} changed fixture write was not byte-exact`);
      }
    }
    changed.push(measurePair(fixtures, toolchain, pairIndex, gap));
  }

  return { cold: scenarioReport(cold), noop: scenarioReport(noop), changed: scenarioReport(changed) };
}

function fixtureMetadata(fixture_, lockBytes) {
  return {
    manifest: relative(root, fixture_.manifest).replaceAll("\\", "/"),
    source: relative(root, fixture_.source).replaceAll("\\", "/"),
    target_directory: relative(root, fixture_.target).replaceAll("\\", "/"),
    manifest_sha256: sha256(readFileSync(fixture_.manifest)),
    base_source_sha256: sha256(Buffer.from(fixture_.body, "utf8")),
    cargo_lock_template: relative(root, fixture_.lockTemplate).replaceAll("\\", "/"),
    cargo_lock_sha256: sha256(lockBytes),
  };
}

const runLock = acquireRunLock();
try {
  prepareEvidenceDestination(evidencePath);
  const harnessBytesBefore = readFileSync(harnessPath);
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
  const environment = captureEnvironment(toolchain);

  rmSync(runtime, { recursive: true, force: true });
  mkdirSync(runtime, { recursive: true });
  prepareCargoHome();

  const fixtures = {
    plain: fixture("plain", false),
    styled: fixture("styled", true),
  };
  const lockBytes = { plain: fixtures.plain.lockBytes, styled: fixtures.styled.lockBytes };
  const trackedLockInputs = [
    trackedInputProvenance(
      gitStateBefore.commit,
      fixtures.plain.lockTemplate,
      lockBytes.plain,
    ),
    trackedInputProvenance(
      gitStateBefore.commit,
      fixtures.styled.lockTemplate,
      lockBytes.styled,
    ),
  ];
  fetchLockedDependencies(fixtures.plain, toolchain);
  fetchLockedDependencies(fixtures.styled, toolchain);

  let timing;
  try {
    timing = measure(fixtures, toolchain);
  } finally {
    for (const fixture_ of Object.values(fixtures)) {
      writeFileSync(fixture_.source, fixture_.body);
    }
  }

  for (const fixtureName of ["plain", "styled"]) {
    const fixture_ = fixtures[fixtureName];
    if (!readFileSync(fixture_.source).equals(Buffer.from(fixture_.body, "utf8"))) {
      throw new Error(`${fixtureName} fixture source was not restored after measurement`);
    }
    if (!readFileSync(fixture_.lockfile).equals(lockBytes[fixtureName])) {
      throw new Error(`${fixtureName} Cargo.lock changed during locked measurement`);
    }
    if (!readFileSync(fixture_.lockTemplate).equals(lockBytes[fixtureName])) {
      throw new Error(`${fixtureName} versioned Cargo.lock template changed during measurement`);
    }
  }

  const harnessBytesAfter = readFileSync(harnessPath);
  if (!harnessBytesBefore.equals(harnessBytesAfter)) {
    throw new Error("benchmark harness changed during measurement");
  }
  const gitStateAfter = captureGitState();
  if (JSON.stringify(gitStateBefore) !== JSON.stringify(gitStateAfter)) {
    throw new Error("Git commit or worktree state changed during measurement");
  }

  const report = {
    schema_version: 3,
    benchmark: "PliegoCSS paired Rust cargo-check overhead",
    profile: "rust-msrv",
    generated_at: new Date().toISOString(),
    provenance: {
      git: gitStateBefore,
      harness: harnessProvenance,
      tracked_inputs: trackedLockInputs,
    },
    environment,
    methodology: {
      design: "paired-interleaved-alternating-order",
      order: "odd-numbered pairs run plain then styled; even-numbered pairs run styled then plain",
      paired_delta: "styled_ns - plain_ns within each pair",
      cold_target_cleanup: "both isolated Cargo target directories are removed before each pair",
      noop_warmup_order: ["styled", "plain"],
      changed_mutation:
        "both sources receive the same new arbitrary gap pixel value before either member of the pair runs",
      pair_counts: {
        cold: coldPairCount,
        noop: noopPairCount,
        changed: changedPairCount,
      },
      exclusive_lock: relative(root, runLockPath).replaceAll("\\", "/"),
      command_timeout_ms: commandTimeoutMs,
    },
    fixtures: {
      base_gap_px: baseGapPx,
      plain: fixtureMetadata(fixtures.plain, lockBytes.plain),
      styled: fixtureMetadata(fixtures.styled, lockBytes.styled),
    },
    command:
      "cargo +1.85.0 check --quiet --locked --offline --target <host> --manifest-path <fixture>/Cargo.toml",
    timing,
    caveat:
      "Cold pairs remove per-fixture Cargo targets but retain normal operating-system and Cargo registry caches. Pairing and alternating order reduce sequential periodic-scanner bias; they do not make this a cold-system measurement.",
  };

  const serializedReport = `${JSON.stringify(report, null, 2)}\n`;
  mkdirSync(dirname(resultsPath), { recursive: true });
  writeFileSync(resultsPath, serializedReport, "utf8");
  if (evidencePath) {
    writeFileSync(evidencePath, serializedReport, { encoding: "utf8", flag: "wx" });
  }
  process.stdout.write(serializedReport);
} finally {
  releaseRunLock(runLock);
}
