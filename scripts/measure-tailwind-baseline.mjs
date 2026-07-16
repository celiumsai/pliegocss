import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { cpus, freemem, platform, release, totalmem } from "node:os";
import { dirname, join, resolve } from "node:path";
import { performance } from "node:perf_hooks";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const benchmarkRoot = join(root, "benchmarks", "tailwind-v4");
const fixturesPath = join(benchmarkRoot, "fixtures.html");
const dist = join(benchmarkRoot, "dist");
const controlled = join(benchmarkRoot, "controlled");
const coreFixturePath = join(controlled, "core", "index.html");
const controlFixturePath = join(controlled, "control", "index.html");
const resultsDir = join(root, "benchmarks", "results");
const packageJson = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
const cliPackage = JSON.parse(
  readFileSync(join(root, "node_modules", "@tailwindcss", "cli", "package.json"), "utf8"),
);
const tailwindPackage = JSON.parse(
  readFileSync(join(root, "node_modules", "tailwindcss", "package.json"), "utf8"),
);
const tailwindCli = join(root, "node_modules", "@tailwindcss", "cli", cliPackage.bin.tailwindcss);

const profiles = [
  { name: "full-complete", input: "input.css", fixture: fixturesPath },
  { name: "no-preflight-complete", input: "input-no-preflight.css", fixture: fixturesPath },
  { name: "full-core", input: "input-core.css", fixture: coreFixturePath },
  {
    name: "no-preflight-core",
    input: "input-core-no-preflight.css",
    fixture: coreFixturePath,
  },
];

mkdirSync(dist, { recursive: true });
mkdirSync(controlled, { recursive: true });
mkdirSync(dirname(coreFixturePath), { recursive: true });
mkdirSync(dirname(controlFixturePath), { recursive: true });
mkdirSync(resultsDir, { recursive: true });

function sha256(buffer) {
  return createHash("sha256").update(buffer).digest("hex");
}

function median(sorted) {
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 0
    ? (sorted[middle - 1] + sorted[middle]) / 2
    : sorted[middle];
}

function summarize(samples) {
  const sorted = [...samples].sort((a, b) => a - b);
  const center = median(sorted);
  const deviations = samples.map((sample) => Math.abs(sample - center)).sort((a, b) => a - b);
  return {
    samplesMs: samples.map((sample) => Number(sample.toFixed(3))),
    medianMs: Number(center.toFixed(3)),
    p95Ms: Number(sorted[Math.ceil(sorted.length * 0.95) - 1].toFixed(3)),
    minMs: Number(sorted[0].toFixed(3)),
    maxMs: Number(sorted.at(-1).toFixed(3)),
    medianAbsoluteDeviationMs: Number(median(deviations).toFixed(3)),
  };
}

function runCommand(input, output, minified) {
  const args = [tailwindCli, "-i", input, "-o", output];
  if (minified) args.push("--minify");
  const start = performance.now();
  execFileSync(process.execPath, args, { cwd: root, stdio: "ignore", timeout: 30_000 });
  return performance.now() - start;
}

function runBuild(input, output, minified) {
  for (let index = 0; index < 5; index += 1) runCommand(input, output, minified);

  const samples = [];
  const hashes = new Map();
  for (let index = 0; index < 30; index += 1) {
    samples.push(runCommand(input, output, minified));
    const bytes = readFileSync(output);
    const hash = sha256(bytes);
    if (!hashes.has(hash)) {
      writeFileSync(`${output}.variant-${hashes.size + 1}.css`, bytes);
    }
    hashes.set(hash, (hashes.get(hash) ?? 0) + 1);
  }
  return {
    ...summarize(samples),
    distinctOutputHashes: hashes.size,
    outputHashFrequencies: Object.fromEntries(hashes),
  };
}

const html = readFileSync(fixturesPath);
const htmlText = html.toString("utf8");
const classValues = [...htmlText.matchAll(/class="([^"]+)"/g)].map((match) => match[1]);
const utilityTokens = classValues.flatMap((value) => value.trim().split(/\s+/u));
const classOnlyHtml = `<div class="${utilityTokens.join(" ")}"></div>\n`;
const coreHtml = htmlText.replace(/class="([^"]+)"/gu, (_, value) => {
  const core = value
    .trim()
    .split(/\s+/u)
    .filter((token) => !token.includes(":"))
    .join(" ");
  return `class="${core}"`;
});
writeFileSync(controlFixturePath, classOnlyHtml);
writeFileSync(coreFixturePath, coreHtml);

const controlSmokeOutput = join(dist, "full-control.smoke.css");
runCommand(join(benchmarkRoot, "input-control.css"), controlSmokeOutput, true);
if (!readFileSync(controlSmokeOutput, "utf8").includes(".flex")) {
  throw new Error("Class-only control source was not scanned by Tailwind");
}

const immutablePaths = [
  fixturesPath,
  join(benchmarkRoot, "theme.css"),
  ...profiles.map((profile) => join(benchmarkRoot, profile.input)),
  fileURLToPath(import.meta.url),
  join(root, "package.json"),
  join(root, "pnpm-lock.yaml"),
];
const hashesBefore = Object.fromEntries(
  immutablePaths.map((path) => [path, sha256(readFileSync(path))]),
);

const measuredProfiles = {};
for (const profile of profiles) {
  const input = join(benchmarkRoot, profile.input);
  const developmentOutput = join(dist, `${profile.name}.dev.css`);
  const productionOutput = join(dist, `${profile.name}.min.css`);
  const developmentBuild = runBuild(input, developmentOutput, false);
  const productionBuild = runBuild(input, productionOutput, true);
  const css = readFileSync(productionOutput);
  const fixture = readFileSync(profile.fixture);
  const cssText = css.toString("utf8");
  const htmlGzipBytes = gzipSync(fixture, { level: 9 }).byteLength;
  const cssGzipBytes = gzipSync(css, { level: 9 }).byteLength;
  measuredProfiles[profile.name] = {
    build: { freshProcessDevelopment: developmentBuild, freshProcessMinified: productionBuild },
    output: {
      cssBytes: css.byteLength,
      cssGzipBytes,
      htmlBytes: fixture.byteLength,
      htmlGzipBytes,
      transferGzipBytes: cssGzipBytes + htmlGzipBytes,
      approximateRuleCount: (cssText.match(/\{/gu) ?? []).length,
      customPropertyDeclarations: (cssText.match(/--[a-z0-9-]+\s*:/giu) ?? []).length,
    },
    hashes: { inputSha256: sha256(readFileSync(input)), outputSha256: sha256(css) },
  };
}

const controlOutput = join(dist, "full-control.min.css");
runCommand(join(benchmarkRoot, "input-control.css"), controlOutput, true);
const scannedHash = measuredProfiles["full-complete"].hashes.outputSha256;
const controlHash = sha256(readFileSync(controlOutput));
const scannerMatchesClassOnlySource = scannedHash === controlHash;

const hashesAfter = Object.fromEntries(
  immutablePaths.map((path) => [path, sha256(readFileSync(path))]),
);
if (JSON.stringify(hashesBefore) !== JSON.stringify(hashesAfter)) {
  throw new Error("Benchmark inputs changed during measurement");
}

function git(args, fallback) {
  try {
    return (
      execFileSync("git", args, { cwd: root, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] })
        .trim() || fallback
    );
  } catch {
    return fallback;
  }
}

const result = {
  schemaVersion: 2,
  generatedAtUtc: new Date().toISOString(),
  tool: "tailwindcss",
  version: tailwindPackage.version,
  cliVersion: cliPackage.version,
  fixtureCount: 5,
  sourceMetrics: {
    classAttributes: classValues.length,
    classValueBytes: Buffer.byteLength(classValues.join("")),
    utilityOccurrences: utilityTokens.length,
    uniqueUtilities: new Set(utilityTokens).size,
  },
  profiles: measuredProfiles,
  scannerControl: {
    matchesClassOnlySource: scannerMatchesClassOnlySource,
    scannedHash,
    controlHash,
  },
  determinism: Object.fromEntries(
    Object.entries(measuredProfiles).map(([name, profile]) => [
      name,
      {
        developmentDistinctHashes: profile.build.freshProcessDevelopment.distinctOutputHashes,
        minifiedDistinctHashes: profile.build.freshProcessMinified.distinctOutputHashes,
      },
    ]),
  ),
  inputHashes: hashesBefore,
  git: {
    commit: git(["rev-parse", "HEAD"], "unborn"),
    dirty: git(["status", "--porcelain"], "").length > 0,
  },
  environment: {
    platform: platform(),
    release: release(),
    arch: process.arch,
    cpu: cpus()[0]?.model ?? "unknown",
    logicalCpus: cpus().length,
    totalMemoryBytes: totalmem(),
    freeMemoryBytesAtReport: freemem(),
    node: process.version,
    zlib: process.versions.zlib,
    packageManager: packageJson.packageManager,
  },
  caveats: [
    "Fresh-process timings retain normal OS caches and existing output files.",
    "Persistent rebuild latency, visual equivalence, and mutation diagnostics are separate gates.",
    "Rule count is approximate until a CSS parser is used for public reports.",
  ],
};

writeFileSync(join(resultsDir, "tailwind-v4.local.json"), `${JSON.stringify(result, null, 2)}\n`);
rmSync(controlled, { recursive: true });
process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
