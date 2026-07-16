import { spawnSync } from "node:child_process";
import { cpSync, existsSync, mkdirSync, rmSync } from "node:fs";
import { dirname, join, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const fuzzRoot = join(root, "fuzz");
const runtime = join(root, "target", "fuzz-runs", `run-${process.pid}-${Date.now()}`);
const dictionary = join(fuzzRoot, "pliegocss.dict");
const targets = ["parser_pipeline", "binary_artifacts"];
const toolchain = process.env.PLIEGO_FUZZ_TOOLCHAIN || "nightly-2026-06-26";
const cargoFuzz = process.env.CARGO_FUZZ_BIN || "cargo-fuzz";
const targetDir = resolve(
  root,
  process.env.PLIEGO_FUZZ_TARGET_DIR || join("target", "fuzz-build"),
);

function fail(message) {
  throw new Error(message);
}

function positiveInteger(value, name) {
  if (!/^[1-9][0-9]*$/u.test(value)) fail(`${name} must be a positive integer`);
  const parsed = Number(value);
  if (!Number.isSafeInteger(parsed)) fail(`${name} exceeds the safe integer range`);
  return parsed;
}

function parseBudget() {
  if (process.argv.length === 2) return { kind: "runs", value: 20_000 };
  if (process.argv.length !== 4) {
    fail("usage: node scripts/check-fuzz.mjs [--runs COUNT | --seconds COUNT]");
  }
  const value = positiveInteger(process.argv[3], process.argv[2]);
  if (process.argv[2] === "--runs") return { kind: "runs", value };
  if (process.argv[2] === "--seconds") return { kind: "seconds", value };
  fail("usage: node scripts/check-fuzz.mjs [--runs COUNT | --seconds COUNT]");
}

function run(program, args, options = {}) {
  const result = spawnSync(program, args, {
    cwd: fuzzRoot,
    env: { ...process.env, RUSTUP_TOOLCHAIN: toolchain },
    encoding: "utf8",
    maxBuffer: 16 * 1024 * 1024,
    timeout: 900_000,
    windowsHide: true,
    ...options,
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    fail(
      `${program} ${args.join(" ")} exited ${result.status}\n` +
        `stdout:\n${result.stdout ?? ""}\nstderr:\n${result.stderr ?? ""}`,
    );
  }
  return result;
}

function main() {
  const budget = parseBudget();
  if (!existsSync(dictionary)) fail(`missing fuzz dictionary: ${dictionary}`);
  run("cargo", [
    "metadata",
    "--manifest-path",
    join(fuzzRoot, "Cargo.toml"),
    "--locked",
    "--no-deps",
    "--format-version",
    "1",
  ]);
  const version = run(cargoFuzz, ["--version"]).stdout.trim();
  if (version !== "cargo-fuzz 0.13.2") {
    fail(`cargo-fuzz version drifted: ${JSON.stringify(version)}`);
  }

  rmSync(runtime, { recursive: true, force: true });
  mkdirSync(runtime, { recursive: true });
  const completed = [];
  for (const target of targets) {
    const trackedCorpus = join(fuzzRoot, "corpus", target);
    const corpus = join(runtime, "corpus", target);
    const artifacts = join(runtime, "artifacts", target);
    cpSync(trackedCorpus, corpus, { recursive: true });
    mkdirSync(artifacts, { recursive: true });
    const budgetArgument =
      budget.kind === "runs" ? `-runs=${budget.value}` : `-max_total_time=${budget.value}`;
    run(
      cargoFuzz,
      [
        "run",
        "--sanitizer",
        "address",
        "--build-std",
        "--target-dir",
        targetDir,
        target,
        corpus,
        "--",
        "-seed=1886153071",
        "-max_len=1024",
        "-timeout=10",
        "-rss_limit_mb=2048",
        `-dict=${dictionary}`,
        `-artifact_prefix=${artifacts}${sep}`,
        "-print_final_stats=1",
        budgetArgument,
      ],
      { stdio: "inherit" },
    );
    completed.push(target);
  }
  console.log(
    JSON.stringify(
      {
        schemaVersion: 1,
        cargoFuzz: version,
        toolchain,
        budget,
        targets: completed,
        seed: 1_886_153_071,
        maxInputBytes: 1_024,
        sanitizer: "address",
      },
      null,
      2,
    ),
  );
  rmSync(runtime, { recursive: true, force: true });
}

try {
  main();
} catch (error) {
  console.error(`${error.stack ?? error}\nreproduction retained at ${runtime}`);
  process.exitCode = 1;
}
