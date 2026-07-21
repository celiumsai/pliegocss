import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, isAbsolute, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptPath = fileURLToPath(import.meta.url);
const root = resolve(dirname(scriptPath), "..");
const evidenceRoot = join(root, "benchmarks", "evidence");
const emptySha256 = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
const summaryTolerance = 0.0011;
const evidenceProfileContracts = {
  "matched-core": {
    filePrefix: "pliego-gate-a",
    harnessPath: "scripts/measure-pliego-gate-a.mjs",
    trackedInputPaths: ["benchmarks/pliego-gate-a/core-fixture.html"],
  },
  complete: {
    filePrefix: "pliego-gate-b",
    harnessPath: "scripts/measure-pliego-gate-b.mjs",
    trackedInputPaths: ["benchmarks/tailwind-v4/fixtures.html"],
  },
  "rust-msrv": {
    filePrefix: "rust-check",
    harnessPath: "scripts/measure-rust-check.mjs",
    trackedInputPaths: [
      "benchmarks/rust-check/locks/plain.Cargo.lock",
      "benchmarks/rust-check/locks/styled.Cargo.lock",
    ],
  },
};
const approvedEvidenceSha256ByFile = new Map([
  [
    "pliego-gate-a-2026-07-21-d16fe5d.json",
    "b5eb11fb92d16ef0bc08e4d55d43d6e19824e4cb35789fe89e0affed05f795ef",
  ],
  [
    "pliego-gate-b-2026-07-21-d16fe5d.json",
    "900debaadc76485906f7edd36fbb297b478faff0ddab0838c9a4a9e27e8011bf",
  ],
  [
    "rust-check-2026-07-21-d16fe5d.json",
    "2a3e54bfc4172c9e3a77322af67d68ad8924ed7e41a8790ff317214140238fd5",
  ],
]);
const frozenTailwindBaselineSha256ByHarness = new Map([
  [
    "971605479e7b610e5f559e9dcd2116f9a0b9c4f3b7f0c0489965927b2f011ef4",
    "2263ae7857e59d72ca541d2deac34cb4bcf2dd75a7d691ed14a2c92c26bc4e6b",
  ],
]);

function fail(message) {
  throw new Error(message);
}

function assert(condition, message) {
  if (!condition) {
    fail(message);
  }
}

function assertEqual(actual, expected, label) {
  if (actual !== expected) {
    fail(`${label}: expected ${JSON.stringify(expected)}, received ${JSON.stringify(actual)}`);
  }
}

function assertClose(actual, expected, label, tolerance = summaryTolerance) {
  if (!Number.isFinite(actual) || Math.abs(actual - expected) > tolerance) {
    fail(`${label}: expected ${expected} ± ${tolerance}, received ${actual}`);
  }
}

function isSha256(value) {
  return typeof value === "string" && /^[0-9a-f]{64}$/u.test(value) && !/^0{64}$/u.test(value);
}

function isPositiveInteger(value) {
  return Number.isSafeInteger(value) && value > 0;
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function canonicalJson(value) {
  if (Array.isArray(value)) {
    return `[${value.map(canonicalJson).join(",")}]`;
  }
  if (value !== null && typeof value === "object") {
    const entries = Object.keys(value)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`);
    return `{${entries.join(",")}}`;
  }
  return JSON.stringify(value);
}

function canonicalJsonSha256(value) {
  return sha256(canonicalJson(value));
}

function round(value) {
  return Number(value.toFixed(3));
}

function conventionalMedian(sorted) {
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 0
    ? (sorted[middle - 1] + sorted[middle]) / 2
    : sorted[middle];
}

function percentile(sorted, percentage) {
  const index = Math.ceil((percentage / 100) * sorted.length) - 1;
  return sorted[Math.max(0, Math.min(index, sorted.length - 1))];
}

function summarize(samples) {
  assert(samples.length > 0, "cannot summarize an empty sample set");
  assert(samples.every((sample) => Number.isFinite(sample)), "timing samples must be finite");
  const sorted = [...samples].sort((left, right) => left - right);
  const mean = sorted.reduce((sum, sample) => sum + sample, 0) / sorted.length;
  const median = conventionalMedian(sorted);
  const deviations = sorted
    .map((sample) => Math.abs(sample - median))
    .sort((left, right) => left - right);
  const variance =
    sorted.reduce((sum, sample) => sum + (sample - mean) ** 2, 0) / sorted.length;
  return {
    samples: sorted.length,
    min: round(sorted[0]),
    median: round(median),
    mean: round(mean),
    p95: round(percentile(sorted, 95)),
    max: round(sorted.at(-1)),
    medianAbsoluteDeviation: round(conventionalMedian(deviations)),
    standardDeviation: round(Math.sqrt(variance)),
  };
}

function verifyStatisticsContract() {
  const even = summarize([4, 1, 3, 2]);
  const odd = summarize([3, 1, 2]);
  assertEqual(even.median, 2.5, "statistics.even.median");
  assertEqual(even.medianAbsoluteDeviation, 1, "statistics.even.mad");
  assertEqual(odd.median, 2, "statistics.odd.median");
  assertEqual(odd.medianAbsoluteDeviation, 1, "statistics.odd.mad");
}

verifyStatisticsContract();

function verifySummary(samples, report, label, suffix = "") {
  const expected = summarize(samples);
  assertEqual(suffix ? report.runs : report.samples, expected.samples, `${label}.samples`);
  assertClose(report[`min${suffix}`], expected.min, `${label}.min`);
  assertClose(report[`median${suffix}`], expected.median, `${label}.median`);
  assertClose(report[`mean${suffix}`], expected.mean, `${label}.mean`);
  assertClose(report[`p95${suffix}`], expected.p95, `${label}.p95`);
  assertClose(report[`max${suffix}`], expected.max, `${label}.max`);
  assertClose(
    report[`median_absolute_deviation${suffix}`],
    expected.medianAbsoluteDeviation,
    `${label}.median_absolute_deviation`,
  );
  assertClose(
    report[`standard_deviation${suffix}`],
    expected.standardDeviation,
    `${label}.standard_deviation`,
  );
}

function verifyCommit(commit, fileName) {
  assert(/^[0-9a-f]{40}$/u.test(commit), `${fileName}: invalid evidence commit`);
  const type = spawnSync("git", ["cat-file", "-t", commit], {
    cwd: root,
    encoding: "utf8",
    windowsHide: true,
  });
  assertEqual(type.status, 0, `${fileName}.commit.object_status`);
  assertEqual(type.stdout.trim(), "commit", `${fileName}.commit.object_type`);
  const ancestor = spawnSync("git", ["merge-base", "--is-ancestor", commit, "HEAD"], {
    cwd: root,
    encoding: "utf8",
    windowsHide: true,
  });
  assertEqual(ancestor.status, 0, `${fileName}.commit.reachable_from_head`);
  const timestamp = spawnSync("git", ["show", "-s", "--format=%cI", commit], {
    cwd: root,
    encoding: "utf8",
    windowsHide: true,
  });
  assertEqual(timestamp.status, 0, `${fileName}.commit.timestamp_status`);
  const parsed = Date.parse(timestamp.stdout.trim());
  assert(Number.isFinite(parsed), `${fileName}: invalid commit timestamp`);
  return parsed;
}

function gitBlob(commit, path) {
  assert(/^[0-9a-f]{40}$/u.test(commit), `invalid evidence commit: ${commit}`);
  assert(
    typeof path === "string" &&
      path.length > 0 &&
      !isAbsolute(path) &&
      !path.includes("\\") &&
      !path.split("/").includes(".."),
    `invalid evidence source path: ${JSON.stringify(path)}`,
  );
  const result = spawnSync("git", ["show", "--no-textconv", `${commit}:${path}`], {
    cwd: root,
    encoding: null,
    maxBuffer: 16 * 1024 * 1024,
    windowsHide: true,
  });
  if (result.error) {
    fail(`cannot read ${commit}:${path}: ${result.error.message}`);
  }
  if (result.status !== 0) {
    fail(`cannot read ${commit}:${path}: ${result.stderr.toString("utf8").trim()}`);
  }
  return result.stdout;
}

function verifyCommon(document, fileName) {
  const profileContract = evidenceProfileContracts[document.profile];
  assert(profileContract, `${fileName}: unknown evidence profile ${JSON.stringify(document.profile)}`);
  const git = document.provenance?.git;
  assert(git, `${fileName}: missing Git provenance`);
  const commitTimestamp = verifyCommit(git.commit, fileName);
  assertEqual(git.dirty, false, `${fileName}.provenance.git.dirty`);
  assertEqual(git.status_entry_count, 0, `${fileName}.provenance.git.status_entry_count`);
  assertEqual(git.status_sha256, emptySha256, `${fileName}.provenance.git.status_sha256`);
  assert(
    typeof document.generated_at === "string" &&
      !Number.isNaN(Date.parse(document.generated_at)) &&
      new Date(document.generated_at).toISOString() === document.generated_at,
    `${fileName}: generated_at is invalid`,
  );
  const generatedTimestamp = Date.parse(document.generated_at);
  assert(
    generatedTimestamp >= commitTimestamp && generatedTimestamp <= commitTimestamp + 24 * 60 * 60 * 1000,
    `${fileName}: generated_at must be within 24 hours after the source commit`,
  );
  const expectedFileName = `${profileContract.filePrefix}-${document.generated_at.slice(0, 10)}-${git.commit.slice(0, 7)}.json`;
  assertEqual(fileName, expectedFileName, `${fileName}.filename`);

  const harness = document.provenance?.harness;
  assert(harness && /^[0-9a-f]{64}$/u.test(harness.sha256), `${fileName}: bad harness hash`);
  assertEqual(harness.path, profileContract.harnessPath, `${fileName}.harness.path`);
  assertEqual(sha256(gitBlob(git.commit, harness.path)), harness.sha256, `${fileName}.harness`);

  const trackedInputs = document.provenance.tracked_inputs;
  assert(Array.isArray(trackedInputs), `${fileName}: tracked_inputs must be an array`);
  assertEqual(
    JSON.stringify(trackedInputs.map((input) => input?.path)),
    JSON.stringify(profileContract.trackedInputPaths),
    `${fileName}.tracked_input_paths`,
  );
  for (const [index, input] of trackedInputs.entries()) {
    assert(input && /^[0-9a-f]{64}$/u.test(input.sha256), `${fileName}: bad tracked input ${index}`);
    assertEqual(
      sha256(gitBlob(git.commit, input.path)),
      input.sha256,
      `${fileName}.tracked_inputs[${index}]`,
    );
  }

  const security = document.environment?.security_tooling;
  const allowedSecurityStates = new Set(["active", "disabled", "not-installed", "unknown"]);
  assert(
    security &&
      typeof security.status === "string" &&
      allowedSecurityStates.has(security.status),
    `${fileName}: security state is undeclared`,
  );
  const allowedPowerPlanStates = new Set([
    "declared",
    "empty",
    "error",
    "recorded",
    "unavailable",
    "unsupported",
  ]);
  assert(
    document.environment?.power_plan &&
      typeof document.environment.power_plan.status === "string" &&
      allowedPowerPlanStates.has(document.environment.power_plan.status),
    `${fileName}: power-plan metadata is missing`,
  );
  const sanitizedKeys = document.environment?.build?.sanitized_environment_keys;
  assert(Array.isArray(sanitizedKeys), `${fileName}: environment policy is missing`);
  assert(
    sanitizedKeys.every((key) => typeof key === "string" && key.length > 0) &&
      JSON.stringify(sanitizedKeys) === JSON.stringify([...new Set(sanitizedKeys)].sort()),
    `${fileName}: sanitized environment keys must be sorted, unique strings`,
  );
}

function verifyFreshProcess(document, fileName, expected) {
  assertEqual(document.schema_version, expected.schema, `${fileName}.schema_version`);
  assertEqual(document.profile, expected.profile, `${fileName}.profile`);
  assertEqual(
    document.fixture.html,
    document.provenance.tracked_inputs[0].path,
    `${fileName}.fixture.html`,
  );
  assertEqual(
    JSON.stringify(document.fixture.groups),
    JSON.stringify(["button", "card", "navbar", "form", "dashboard"]),
    `${fileName}.fixture.groups`,
  );
  assertEqual(document.fixture.unique_style_inputs, expected.uniqueStyleInputs, `${fileName}.unique_styles`);
  assertEqual(document.fixture.utility_occurrences, expected.utilityOccurrences, `${fileName}.utility_occurrences`);
  assertEqual(document.fixture.unique_utility_tokens, expected.uniqueUtilityTokens, `${fileName}.unique_utility_tokens`);
  assertEqual(document.fixture.theme_included, true, `${fileName}.fixture.theme_included`);
  assert(isSha256(document.fixture.derived_input_sha256), `${fileName}: bad derived-input hash`);
  const fixtureBytes = gitBlob(document.provenance.git.commit, document.fixture.html);
  const classValues = [...fixtureBytes.toString("utf8").matchAll(/class="([^"]*)"/gu)].map(
    (match) => match[1],
  );
  assertEqual(classValues.length, 44, `${fileName}.fixture.git_class_attribute_count`);
  assertEqual(
    sha256(Buffer.from(`${classValues.join("\n")}\n`, "utf8")),
    document.fixture.derived_input_sha256,
    `${fileName}.fixture.derived_input_sha256`,
  );
  assertEqual(document.command, expected.command, `${fileName}.command`);
  assertEqual(
    document.environment.toolchain.cargo,
    "cargo 1.96.0 (30a34c682 2026-05-25)",
    `${fileName}.cargo`,
  );
  assertEqual(
    document.environment.toolchain.rustc,
    "rustc 1.96.0 (ac68faa20 2026-05-25)",
    `${fileName}.rustc`,
  );
  assertEqual(document.environment.toolchain.host_target, "x86_64-pc-windows-msvc", `${fileName}.host`);
  assertEqual(document.environment.build.host_target, document.environment.toolchain.host_target, `${fileName}.build.host`);
  assertEqual(document.environment.build.cargo_target_dir, expected.cargoTargetDir, `${fileName}.cargo_target_dir`);
  assertEqual(document.environment.build.cargo_incremental, false, `${fileName}.cargo_incremental`);
  assertEqual(document.environment.build.cargo_home, expected.cargoHome, `${fileName}.cargo_home`);
  assertEqual(
    document.environment.build.cargo_home_policy,
    "isolated benchmark directory without user Cargo configuration",
    `${fileName}.cargo_home_policy`,
  );
  assertEqual(document.timing.warmup_processes, 5, `${fileName}.warmups`);
  assertEqual(document.timing.runs, 30, `${fileName}.runs`);
  assertEqual(document.timing.samples_ms.length, 30, `${fileName}.sample_count`);
  assert(document.timing.samples_ms.every((sample) => sample > 0), `${fileName}: timings must be positive`);
  verifySummary(document.timing.samples_ms, document.timing, `${fileName}.timing`, "_ms");

  assertEqual(
    document.statistics?.median,
    "average of the two middle values for even samples",
    `${fileName}.statistics.median`,
  );
  assertEqual(document.statistics?.p95, "nearest-rank", `${fileName}.statistics.p95`);
  assertEqual(
    document.statistics?.median_absolute_deviation,
    "conventional median of absolute deviations from the median",
    `${fileName}.statistics.median_absolute_deviation`,
  );
  assertEqual(document.determinism.verified_runs, 30, `${fileName}.determinism.runs`);
  assertEqual(document.determinism.deterministic, true, `${fileName}.determinism.deterministic`);
  assertEqual(document.determinism.sha256, document.output.css_sha256, `${fileName}.css_sha256`);
  for (const [name, value] of Object.entries({
    css: document.output.css_sha256,
    executable: document.output.executable_sha256,
    manifest: document.output.manifest_sha256,
    utilityHtml: document.output.html?.utility_strings?.sha256,
    generatedHtml: document.output.html?.generated_classes?.sha256,
  })) {
    assert(isSha256(value), `${fileName}: bad ${name} hash`);
  }
  assert(
    isPositiveInteger(document.output.css_raw_bytes) &&
      isPositiveInteger(document.output.css_gzip_bytes) &&
      document.output.css_gzip_bytes <= document.output.css_raw_bytes,
    `${fileName}: invalid CSS sizes`,
  );
  assertEqual(document.output.html_unchanged_outside_class_values, true, `${fileName}.html_integrity`);
  assertEqual(
    document.fixture.html_sha256,
    document.provenance.tracked_inputs[0].sha256,
    `${fileName}.fixture.html_sha256`,
  );
  assertEqual(
    document.output.html.utility_strings.sha256,
    document.fixture.html_sha256,
    `${fileName}.utility_html.sha256`,
  );
  for (const [name, report] of Object.entries(document.output.html)) {
    assert(
      isPositiveInteger(report.raw_bytes) &&
        isPositiveInteger(report.gzip_bytes) &&
        isPositiveInteger(report.transfer_html_css_gzip_bytes) &&
        report.gzip_bytes <= report.raw_bytes,
      `${fileName}.${name}: invalid HTML sizes`,
    );
    assertEqual(
      report.transfer_html_css_gzip_bytes,
      report.gzip_bytes + document.output.css_gzip_bytes,
      `${fileName}.${name}.transfer`,
    );
  }
  assertEqual(
    document.output.transfer_gzip_bytes,
    document.output.html.generated_classes.transfer_html_css_gzip_bytes,
    `${fileName}.output.transfer_gzip_bytes`,
  );
  assertEqual(document.fixture.class_attribute_count, 44, `${fileName}.class_attribute_count`);
}

function percentChange(reference, candidate) {
  return round(((candidate - reference) / reference) * 100);
}

function verifyGateBComparisons(document, fileName) {
  const expectedVariants = [
    "disabled",
    "focus",
    "focus-visible",
    "hover",
    "lg",
    "md",
    "placeholder",
    "sm",
  ];
  assertEqual(document.fixture.class_attributes_resolved, 44, `${fileName}.resolved_classes`);
  assertEqual(document.fixture.all_class_attributes_resolved, true, `${fileName}.all_classes_resolved`);
  for (const field of ["variants", "required_variants", "emitted_variant_markers_verified"]) {
    assertEqual(
      JSON.stringify(document.fixture[field]),
      JSON.stringify(expectedVariants),
      `${fileName}.fixture.${field}`,
    );
  }
  const harnessSha256 = document.provenance.harness.sha256;
  const expectedBaselineSha256 = frozenTailwindBaselineSha256ByHarness.get(harnessSha256);
  assert(
    expectedBaselineSha256,
    `${fileName}: Gate B harness has no approved Tailwind baseline`,
  );
  assertEqual(
    canonicalJsonSha256(document.frozen_tailwind_baseline),
    expectedBaselineSha256,
    `${fileName}.frozen_tailwind_baseline`,
  );

  for (const [profile, baseline] of Object.entries(document.frozen_tailwind_baseline.profiles)) {
    const comparison = document.comparisons[profile];
    assert(comparison, `${fileName}: missing ${profile} comparison`);
    assertEqual(
      comparison.median_time_change_percent,
      percentChange(baseline.median_ms, document.timing.median_ms),
      `${fileName}.${profile}.median_time_change_percent`,
    );
    assertEqual(
      comparison.median_speed_ratio,
      round(baseline.median_ms / document.timing.median_ms),
      `${fileName}.${profile}.median_speed_ratio`,
    );
    assertEqual(
      comparison.css_raw_change_percent,
      percentChange(baseline.css_raw_bytes, document.output.css_raw_bytes),
      `${fileName}.${profile}.css_raw_change_percent`,
    );
    assertEqual(
      comparison.css_gzip_change_percent,
      percentChange(baseline.css_gzip_bytes, document.output.css_gzip_bytes),
      `${fileName}.${profile}.css_gzip_change_percent`,
    );
    assertEqual(
      comparison.transfer_gzip_change_percent,
      percentChange(baseline.transfer_gzip_bytes, document.output.transfer_gzip_bytes),
      `${fileName}.${profile}.transfer_gzip_change_percent`,
    );
  }
}

function verifyRustScenario(document, name, expectedCount) {
  const scenario = document.timing[name];
  assertEqual(scenario.paired_sample_count, expectedCount, `${name}.paired_sample_count`);
  assertEqual(scenario.pairs.length, expectedCount, `${name}.pairs.length`);
  assertEqual(scenario.order_balance.plain_first, expectedCount / 2, `${name}.plain_first`);
  assertEqual(scenario.order_balance.styled_first, expectedCount / 2, `${name}.styled_first`);

  const plain = [];
  const styled = [];
  const deltas = [];
  const percentages = [];
  for (const [index, pair] of scenario.pairs.entries()) {
    const pairNumber = index + 1;
    const expectedOrder = index % 2 === 0 ? ["plain", "styled"] : ["styled", "plain"];
    assertEqual(pair.pair, pairNumber, `${name}.pair[${index}].pair`);
    assertEqual(JSON.stringify(pair.order), JSON.stringify(expectedOrder), `${name}.pair[${index}].order`);
    assert(Number.isSafeInteger(pair.plain_ns) && pair.plain_ns > 0, `${name}.pair[${index}].plain_ns`);
    assert(Number.isSafeInteger(pair.styled_ns) && pair.styled_ns > 0, `${name}.pair[${index}].styled_ns`);
    assertEqual(pair.delta_ns, pair.styled_ns - pair.plain_ns, `${name}.pair[${index}].delta_ns`);
    const percent = (pair.delta_ns / pair.plain_ns) * 100;
    assertEqual(pair.delta_percent, round(percent), `${name}.pair[${index}].delta_percent`);
    assertEqual(
      pair.mutation_gap_px,
      name === "changed" ? 5 + index : null,
      `${name}.pair[${index}].mutation_gap_px`,
    );
    plain.push(pair.plain_ns / 1_000_000);
    styled.push(pair.styled_ns / 1_000_000);
    deltas.push(pair.delta_ns / 1_000_000);
    percentages.push(percent);
  }

  const expectedValues = {
    plain_ms: plain.map(round),
    styled_ms: styled.map(round),
    paired_delta_ms: deltas.map(round),
    paired_delta_percent: percentages.map(round),
  };
  for (const [series, values] of Object.entries(expectedValues)) {
    assert(Array.isArray(scenario[series].values), `${name}.${series}.values must be an array`);
    assertEqual(
      JSON.stringify(scenario[series].values),
      JSON.stringify(values),
      `${name}.${series}.values`,
    );
  }

  verifySummary(plain, scenario.plain_ms, `${name}.plain_ms`);
  verifySummary(styled, scenario.styled_ms, `${name}.styled_ms`);
  verifySummary(deltas, scenario.paired_delta_ms, `${name}.paired_delta_ms`);
  verifySummary(percentages, scenario.paired_delta_percent, `${name}.paired_delta_percent`);
}

function verifyRustCheck(document, fileName) {
  assertEqual(document.schema_version, 3, `${fileName}.schema_version`);
  assertEqual(document.profile, "rust-msrv", `${fileName}.profile`);
  assertEqual(
    document.command,
    "cargo +1.85.0 check --quiet --locked --offline --target <host> --manifest-path <fixture>/Cargo.toml",
    `${fileName}.command`,
  );
  assertEqual(
    document.environment.toolchain.cargo,
    "cargo 1.85.0 (d73d2caf9 2024-12-31)",
    `${fileName}.cargo`,
  );
  assertEqual(
    document.environment.toolchain.rustc,
    "rustc 1.85.0 (4d91de4e4 2025-02-17)",
    `${fileName}.rustc`,
  );
  assertEqual(document.environment.toolchain.host_target, "x86_64-pc-windows-msvc", `${fileName}.host`);
  assertEqual(document.environment.build.host_target, document.environment.toolchain.host_target, `${fileName}.build.host`);
  assertEqual(document.environment.build.cargo_incremental, true, `${fileName}.cargo_incremental`);
  assertEqual(document.environment.build.cargo_build_jobs, null, `${fileName}.cargo_build_jobs`);
  assertEqual(document.environment.build.cargo_home, "target/benchmarks/rust-check/cargo-home", `${fileName}.cargo_home`);
  assertEqual(
    document.environment.build.cargo_home_policy,
    "isolated benchmark directory without user Cargo configuration",
    `${fileName}.cargo_home_policy`,
  );
  assertEqual(document.environment.build.command_timeout_ms, 120000, `${fileName}.build.timeout`);
  const methodologyContract = {
    design: "paired-interleaved-alternating-order",
    order: "odd-numbered pairs run plain then styled; even-numbered pairs run styled then plain",
    paired_delta: "styled_ns - plain_ns within each pair",
    cold_target_cleanup: "both isolated Cargo target directories are removed before each pair",
    changed_mutation:
      "both sources receive the same new arbitrary gap pixel value before either member of the pair runs",
    exclusive_lock: "target/benchmarks/rust-check/run.lock",
    command_timeout_ms: 120000,
  };
  for (const [field, expected] of Object.entries(methodologyContract)) {
    assertEqual(document.methodology[field], expected, `${fileName}.methodology.${field}`);
  }
  assertEqual(
    JSON.stringify(document.methodology.noop_warmup_order),
    JSON.stringify(["styled", "plain"]),
    `${fileName}.methodology.noop_warmup_order`,
  );
  assertEqual(document.methodology.pair_counts.cold, 6, `${fileName}.cold_count`);
  assertEqual(document.methodology.pair_counts.noop, 30, `${fileName}.noop_count`);
  assertEqual(document.methodology.pair_counts.changed, 30, `${fileName}.changed_count`);
  assertEqual(document.fixtures.base_gap_px, 4, `${fileName}.fixtures.base_gap_px`);
  const fixtureContracts = {
    plain: {
      manifest: "benchmarks/rust-check/runtime/plain/Cargo.toml",
      source: "benchmarks/rust-check/runtime/plain/src/main.rs",
      target_directory: "benchmarks/rust-check/runtime/targets/plain",
      manifest_sha256: "8cc0751ac64795b6fc57ca4c04ef69fb5fc989d9c307df646bd21e514be9338f",
      base_source_sha256: "c608013aceccafc99491f0a5f01e7d9851eec0ce4db8b280eb61e28b04c9a785",
      cargo_lock_template: "benchmarks/rust-check/locks/plain.Cargo.lock",
    },
    styled: {
      manifest: "benchmarks/rust-check/runtime/styled/Cargo.toml",
      source: "benchmarks/rust-check/runtime/styled/src/main.rs",
      target_directory: "benchmarks/rust-check/runtime/targets/styled",
      manifest_sha256: "c53d8a7f67916f765666a7a4f7696937e75022f57cf73dd12baad2fa9fff3374",
      base_source_sha256: "10328410485436f478bee05706e862b1abc4ec697c66e2da4dbc1a0f7ac814f7",
      cargo_lock_template: "benchmarks/rust-check/locks/styled.Cargo.lock",
    },
  };
  for (const [index, name] of ["plain", "styled"].entries()) {
    const fixture = document.fixtures[name];
    for (const [field, expected] of Object.entries(fixtureContracts[name])) {
      assertEqual(fixture[field], expected, `${fileName}.fixtures.${name}.${field}`);
    }
    assertEqual(
      fixture.cargo_lock_sha256,
      document.provenance.tracked_inputs[index].sha256,
      `${fileName}.fixtures.${name}.cargo_lock_sha256`,
    );
  }
  verifyRustScenario(document, "cold", 6);
  verifyRustScenario(document, "noop", 30);
  verifyRustScenario(document, "changed", 30);
}

const files = readdirSync(evidenceRoot)
  .filter((name) => name.endsWith(".json"))
  .sort();
assert(files.length > 0, "no frozen benchmark evidence found");
assertEqual(
  JSON.stringify(files),
  JSON.stringify([...approvedEvidenceSha256ByFile.keys()].sort()),
  "approved evidence snapshot set",
);

const profiles = new Set();
const results = [];
for (const fileName of files) {
  const path = join(evidenceRoot, fileName);
  const bytes = readFileSync(path);
  assertEqual(
    sha256(bytes),
    approvedEvidenceSha256ByFile.get(fileName),
    `${fileName}.snapshot_sha256`,
  );
  const document = JSON.parse(bytes.toString("utf8"));
  verifyCommon(document, fileName);
  profiles.add(document.profile);
  if (document.profile === "matched-core") {
    verifyFreshProcess(document, fileName, {
      schema: 4,
      profile: "matched-core",
      uniqueStyleInputs: 28,
      utilityOccurrences: 257,
      uniqueUtilityTokens: 65,
      command:
        "pliego-cssc compile --input <derived-from-core-class-attributes> --seed --theme --output <temporary-file>",
      cargoTargetDir: "target/benchmarks/pliego-gate-a",
      cargoHome: "target/benchmarks/cargo-home-gate-a",
    });
  } else if (document.profile === "complete") {
    verifyFreshProcess(document, fileName, {
      schema: 3,
      profile: "complete",
      uniqueStyleInputs: 31,
      utilityOccurrences: 302,
      uniqueUtilityTokens: 90,
      command:
        "pliego-cssc compile --input <derived-from-complete-class-attributes> --seed --theme --output <temporary-file>",
      cargoTargetDir: "target/benchmarks/pliego-gate-b",
      cargoHome: "target/benchmarks/cargo-home-gate-b",
    });
    verifyGateBComparisons(document, fileName);
  } else if (document.profile === "rust-msrv") {
    verifyRustCheck(document, fileName);
  } else {
    fail(`${fileName}: unknown evidence profile ${JSON.stringify(document.profile)}`);
  }
  results.push({ file: fileName, profile: document.profile, commit: document.provenance.git.commit });
}

for (const required of ["matched-core", "complete", "rust-msrv"]) {
  assert(profiles.has(required), `missing ${required} evidence`);
}

process.stdout.write(`${JSON.stringify({ status: "ok", snapshots: results }, null, 2)}\n`);
