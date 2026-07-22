import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { cargoTargetRoot, isolatedCargoEnvironment } from "./rust-target.mjs";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const cargoEnvironment = isolatedCargoEnvironment(root, {
  env: process.env,
  toolchain: "1.85",
});
const targetDir = cargoTargetRoot(root, cargoEnvironment);
const runtime = join(targetDir, "compatibility-policy", `run-${process.pid}-${Date.now()}`);
const expected = readFileSync(
  join(root, "integration-tests", "compatibility-policy", "expected.baseline-widely.json"),
);

function fail(message) {
  throw new Error(message);
}

function assert(condition, message) {
  if (!condition) fail(message);
}

function run(program, args, { cwd = runtime, expectedStatus = 0, timeout = 30_000 } = {}) {
  const result = spawnSync(program, args, {
    cwd,
    encoding: "utf8",
    env: cargoEnvironment,
    timeout,
    windowsHide: true,
  });
  if (result.error) throw result.error;
  if (result.status !== expectedStatus) {
    fail(
      `${program} ${args.join(" ")} exited ${result.status}, expected ${expectedStatus}\n` +
        `stdout:\n${result.stdout}\nstderr:\n${result.stderr}`,
    );
  }
  return result;
}

function resolveExecutable() {
  if (process.env.PLIEGO_CSSC) {
    const configured = resolve(root, process.env.PLIEGO_CSSC);
    assert(existsSync(configured), `PLIEGO_CSSC does not exist: ${configured}`);
    return configured;
  }

  run("cargo", ["+1.85", "build", "--locked", "-p", "pliego-cssc"], {
    cwd: root,
    timeout: 300_000,
  });
  const executable = join(
    targetDir,
    "debug",
    process.platform === "win32" ? "pliego-cssc.exe" : "pliego-cssc",
  );
  assert(existsSync(executable), `Rust 1.85 build did not produce ${executable}`);
  return executable;
}

function runCli(executable, args, expectedStatus = 0) {
  return run(executable, args, { expectedStatus });
}

rmSync(runtime, { recursive: true, force: true });
mkdirSync(runtime, { recursive: true });

try {
  const executable = resolveExecutable();
  const first = runCli(executable, ["compatibility", "--targets", "baseline-widely"]);
  const second = runCli(executable, ["compatibility", "--targets", "baseline-widely"]);
  assert(first.stderr === "", `compatibility emitted stderr:\n${first.stderr}`);
  assert(Buffer.compare(Buffer.from(first.stdout), expected) === 0, "baseline policy bytes drifted");
  assert(first.stdout === second.stdout, "baseline policy is not deterministic");

  for (const profile of ["modern", "none"]) {
    const result = runCli(executable, ["compatibility", "--targets", profile]);
    assert(result.stderr === "", `${profile} policy emitted stderr:\n${result.stderr}`);
    const policy = JSON.parse(result.stdout);
    assert(policy.schemaVersion === 2, `${profile} policy schema drifted`);
    assert(policy.policyVersion === 7, `${profile} policy version drifted`);
    assert(policy.profile === profile, `${profile} policy mislabeled itself`);
    assert(
      policy.compatibilityData?.webFeatures?.version === "3.32.0",
      `${profile} web-features version drifted`,
    );
    assert(
      policy.compatibilityData?.baselineBrowserMapping?.version === "2.10.43",
      `${profile} Baseline mapping version drifted`,
    );
    assert(
      policy.compatibilityData?.capturedOn === "2026-07-14",
      `${profile} compatibility capture date drifted`,
    );
    assert(policy.reset.implicit === false, `${profile} enabled an implicit reset`);
    assert(policy.scope.profile === "standard-class", `${profile} scope boundary drifted`);
  }

  const css = join(runtime, "app.css");
  const manifest = join(runtime, "app.manifest.json");
  const compiled = runCli(executable, [
    "compile",
    "--style",
    "container-inline layer-components:cq-sm:rtl:aria-[sort=ascending]:data-[density=compact]:data-[loading]:hover:flex items-center gap-4 writing-vertical-rl",
    "--seed",
    "--targets",
    "baseline-widely",
    "--output",
    css,
    "--manifest",
    manifest,
  ]);
  assert(compiled.stdout === "", "file compile unexpectedly emitted stdout");
  assert(compiled.stderr === "", `typed baseline compile emitted stderr:\n${compiled.stderr}`);
  const compiledCss = readFileSync(css, "utf8");
  assert(compiledCss.endsWith("\n"), "typed baseline CSS lacks final newline");
  assert(
    compiledCss.includes(
      ":hover:dir(rtl)[aria-sort=ascending][data-density=compact][data-loading]",
    ),
    "typed configurable attribute/direction variants were not emitted",
  );
  assert(
    compiledCss.includes("@container (width>=40rem)"),
    "typed container query was not emitted",
  );
  assert(compiledCss.includes("writing-mode:vertical-rl"), "typed writing mode was not emitted");
  assert(
    compiledCss.includes("@layer pliego.base,pliego.components,pliego.utilities,pliego.overrides"),
    "typed cascade layer order was not emitted",
  );
  assert(compiledCss.includes("@layer pliego.components"), "typed cascade layer was not emitted");
  assert(JSON.parse(readFileSync(manifest, "utf8")).targets === "baseline-widely", "manifest target drifted");

  const sentinel = Buffer.from("keep-last-valid-artifact\n");
  writeFileSync(css, sentinel);
  for (const [style, code] of [
    ["w-[13px]", "CMP001"],
    ["[mask-type:luminance]", "CMP002"],
    ["[&>span]:flex", "CMP003"],
  ]) {
    const rejected = runCli(
      executable,
      [
        "--diagnostic-format",
        "json",
        "compile",
        "--style",
        style,
        "--seed",
        "--targets",
        "baseline-widely",
        "--output",
        css,
      ],
      1,
    );
    assert(rejected.stdout === "", `${code} rejection emitted stdout`);
    const diagnostic = JSON.parse(rejected.stderr);
    assert(diagnostic.schemaVersion === 1, `${code} diagnostic schema drifted`);
    assert(diagnostic.diagnostics?.[0]?.code === code, `${code} was not preserved in JSON`);
    assert(diagnostic.diagnostics[0].category === "compatibility", `${code} category drifted`);
    assert(Buffer.compare(readFileSync(css), sentinel) === 0, `${code} mutated the previous CSS`);
  }

  console.log("compatibility policy gate: pass");
} finally {
  rmSync(runtime, { recursive: true, force: true });
}
