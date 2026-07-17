import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
} from "node:fs";
import { dirname, isAbsolute, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const manifestPath = join(root, "integration-tests", "migration-real-corpus", "projects.json");
const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));

function fail(message) {
  throw new Error(message);
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, { encoding: "utf8", maxBuffer: 64 * 1024 * 1024, ...options });
  if (result.error) fail(`cannot run ${command}: ${result.error.message}`);
  if (result.status !== 0) {
    fail(`${command} ${args.join(" ")} failed (${result.status}):\n${result.stderr}`);
  }
  return result;
}

function filesUnder(directory) {
  const output = [];
  for (const name of readdirSync(directory).sort()) {
    if (name === ".git") continue;
    const path = join(directory, name);
    const metadata = statSync(path, { throwIfNoEntry: true });
    if (metadata.isDirectory()) output.push(...filesUnder(path));
    else if (metadata.isFile()) output.push(path);
    else fail(`review scope contains a non-file entry: ${path}`);
  }
  return output;
}

function portable(path) {
  return path.replaceAll("\\", "/");
}

function tuple(role, file) {
  return `${role}\u0000${portable(file)}`;
}

function requireWithin(parent, child, label) {
  const offset = relative(parent, child);
  if (offset === "" || (!offset.startsWith("..") && !isAbsolute(offset))) return;
  fail(`${label} escapes ${parent}`);
}

function sorted(set) {
  return [...set].sort();
}

function metrics(predicted, gold) {
  const truePositives = sorted(new Set([...predicted].filter((item) => gold.has(item))));
  const falsePositives = sorted(new Set([...predicted].filter((item) => !gold.has(item))));
  const falseNegatives = sorted(new Set([...gold].filter((item) => !predicted.has(item))));
  const precision = predicted.size === 0 ? (gold.size === 0 ? 1 : 0) : truePositives.length / predicted.size;
  const recall = gold.size === 0 ? 1 : truePositives.length / gold.size;
  return {
    truePositives: truePositives.length,
    falsePositives: falsePositives.length,
    falseNegatives: falseNegatives.length,
    precision,
    recall,
    falsePositiveTuples: falsePositives,
    falseNegativeTuples: falseNegatives,
  };
}

function predictedRoles(document) {
  const output = new Set();
  for (const source of document.sources ?? []) {
    output.add(tuple(`source:${source.sourceKind}`, source.file));
  }
  for (const auxiliary of document.auxiliaries ?? []) {
    output.add(tuple(`auxiliary:${auxiliary.auxiliaryKind}`, auxiliary.file));
  }
  for (const consumer of document.consumers ?? []) {
    output.add(tuple(`consumer:${consumer.consumerKind}`, consumer.file));
  }
  return output;
}

function goldRoles(entry, projectRoot) {
  const files = filesUnder(projectRoot).map((path) => portable(relative(projectRoot, path)));
  const output = new Set();
  for (const rule of entry.gold.rules) {
    for (const file of files) {
      if (file.startsWith(rule.prefix) && file.endsWith(rule.suffix)) {
        output.add(tuple(rule.role, file));
      }
    }
  }
  for (const exact of entry.gold.exact) output.add(tuple(exact.role, exact.file));
  return { roles: output, reviewedFiles: files.length };
}

if (manifest.schemaVersion !== 1 || manifest.classification !== "reviewed-public-role-corpus") {
  fail("real migration corpus manifest must be schema 1 and reviewed-public-role-corpus");
}
if (manifest.metricBoundary !== "file-role-discovery") {
  fail("real migration corpus metrics must remain bounded to file-role-discovery");
}
if (!Array.isArray(manifest.cases) || manifest.cases.length !== 3) {
  fail("real migration corpus requires the three reviewed public cases");
}

if (process.env.PLIEGOCSS_RUN_NETWORK_CORPUS !== "1") {
  process.stdout.write(
    `${JSON.stringify({
      schemaVersion: 1,
      classification: manifest.classification,
      metricBoundary: manifest.metricBoundary,
      status: "skipped",
      reason: "set PLIEGOCSS_RUN_NETWORK_CORPUS=1 to clone the pinned public repositories",
    }, null, 2)}\n`,
  );
  process.exit(0);
}

const cargo = process.env.CARGO ?? "cargo";
if (!process.env.PLIEGOCSS_BIN) {
  run(cargo, ["build", "--locked", "--quiet", "-p", "pliego-cssc"], { cwd: root });
}
const executable =
  process.env.PLIEGOCSS_BIN ??
  join(root, "target", "debug", process.platform === "win32" ? "pliego-cssc.exe" : "pliego-cssc");
if (!existsSync(executable)) fail(`missing PliegoCSS CLI at ${executable}`);

const temporaryBase = join(root, "target");
mkdirSync(temporaryBase, { recursive: true });
const temporaryRoot = mkdtempSync(join(temporaryBase, "migration-real-corpus-"));
const ids = new Set();
const cases = [];
let aggregateTp = 0;
let aggregateFp = 0;
let aggregateFn = 0;

try {
  for (const entry of manifest.cases) {
    if (!entry.id || ids.has(entry.id)) fail(`duplicate or empty corpus id ${entry.id}`);
    ids.add(entry.id);
    const checkout = join(temporaryRoot, entry.id);
    const gitCheckout = portable(relative(root, checkout));
    const gitOptions = { cwd: root };
    run("git", ["init", "--quiet", gitCheckout], gitOptions);
    run("git", ["-C", gitCheckout, "remote", "add", "origin", entry.repository], gitOptions);
    run("git", ["-C", gitCheckout, "sparse-checkout", "init", "--no-cone"], gitOptions);
    run(
      "git",
      ["-C", gitCheckout, "sparse-checkout", "set", "--no-cone", ...entry.sparsePatterns],
      gitOptions,
    );
    run(
      "git",
      ["-C", gitCheckout, "fetch", "--quiet", "--depth", "1", "origin", entry.commit],
      gitOptions,
    );
    run(
      "git",
      ["-C", gitCheckout, "checkout", "--quiet", "--detach", "FETCH_HEAD"],
      gitOptions,
    );

    const actualCommit = run("git", ["-C", gitCheckout, "rev-parse", "HEAD"], gitOptions).stdout.trim();
    if (actualCommit !== entry.commit) fail(`${entry.id}: checked out ${actualCommit}, expected ${entry.commit}`);
    const licensePath = resolve(checkout, entry.license.path);
    requireWithin(checkout, licensePath, `${entry.id}: license path`);
    if (!existsSync(licensePath)) {
      fail(`${entry.id}: sparse checkout omitted ${entry.license.path}; files=${filesUnder(checkout).map((path) => portable(relative(checkout, path))).join(",")}`);
    }
    const licenseSha256 = createHash("sha256").update(readFileSync(licensePath)).digest("hex");
    if (entry.license.spdx !== "MIT" || licenseSha256 !== entry.license.sha256) {
      fail(`${entry.id}: pinned MIT license digest mismatch`);
    }
    const beforeStatus = run(
      "git",
      ["-C", gitCheckout, "status", "--porcelain=v1"],
      gitOptions,
    ).stdout;
    if (beforeStatus !== "") fail(`${entry.id}: checkout is dirty before inventory`);

    const projectRoot = resolve(checkout, entry.projectDirectory);
    requireWithin(checkout, projectRoot, `${entry.id}: project directory`);
    const invocation = run(executable, ["migration-project-inventory", "."], { cwd: projectRoot });
    if (!invocation.stdout.endsWith("\n")) fail(`${entry.id}: stdout lacks trailing LF`);
    if (invocation.stderr !== "") fail(`${entry.id}: successful command wrote stderr`);
    const document = JSON.parse(invocation.stdout);
    for (const [key, expected] of Object.entries(entry.expectedSummary)) {
      if (document.summary?.[key] !== expected) {
        fail(`${entry.id}: summary.${key}=${document.summary?.[key]}, expected ${expected}`);
      }
    }
    for (const expected of entry.expectedDependencies ?? []) {
      const found = document.dependencies?.some((dependency) =>
        Object.entries(expected).every(([key, value]) => dependency[key] === value),
      );
      if (!found) fail(`${entry.id}: missing dependency ${JSON.stringify(expected)}`);
    }

    const predicted = predictedRoles(document);
    const gold = goldRoles(entry, projectRoot);
    const score = metrics(predicted, gold.roles);
    if (score.falsePositives !== 0 || score.falseNegatives !== 0) {
      fail(`${entry.id}: role mismatch ${JSON.stringify(score)}`);
    }
    aggregateTp += score.truePositives;
    aggregateFp += score.falsePositives;
    aggregateFn += score.falseNegatives;

    const afterStatus = run(
      "git",
      ["-C", gitCheckout, "status", "--porcelain=v1"],
      gitOptions,
    ).stdout;
    if (afterStatus !== beforeStatus) fail(`${entry.id}: inventory mutated the checkout`);
    cases.push({
      id: entry.id,
      repository: entry.repository,
      commit: entry.commit,
      license: entry.license.spdx,
      reviewedFiles: gold.reviewedFiles,
      predictedRoles: predicted.size,
      goldRoles: gold.roles.size,
      inventoryBytes: Buffer.byteLength(invocation.stdout),
      metrics: score,
      summary: entry.expectedSummary,
    });
  }
} finally {
  if (process.env.PLIEGOCSS_KEEP_REAL_CORPUS !== "1") {
    rmSync(temporaryRoot, { recursive: true, force: true });
  }
}

const aggregatePredicted = aggregateTp + aggregateFp;
const aggregateGold = aggregateTp + aggregateFn;
process.stdout.write(
  `${JSON.stringify({
    schemaVersion: 1,
    classification: manifest.classification,
    metricBoundary: manifest.metricBoundary,
    status: "passed",
    cases,
    aggregate: {
      truePositives: aggregateTp,
      falsePositives: aggregateFp,
      falseNegatives: aggregateFn,
      precision: aggregatePredicted === 0 ? 1 : aggregateTp / aggregatePredicted,
      recall: aggregateGold === 0 ? 1 : aggregateTp / aggregateGold,
    },
    temporaryCheckout: process.env.PLIEGOCSS_KEEP_REAL_CORPUS === "1" ? temporaryRoot : null,
  }, null, 2)}\n`,
);
