import { createHash } from "node:crypto";
import { spawn, spawnSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { basename, dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const runtime = join(root, "target", "parallel-determinism", `run-${process.pid}-${Date.now()}`);
const project = join(runtime, "project");
const workers = 8;
const sourceText = `fn app() {
    let _ = pc!("flex items-center justify-between gap-4 p-4 bg-surface text-ink rounded-lg shadow-md");
    let _ = pc!("md:grid md:grid-cols-3 md:gap-6");
    let _ = pc!("dark:hover:bg-accent/20 motion-reduce:opacity-50");
    let _ = pc!("[&>p]:text-sm [&>p]:leading-6");
    let _ = pc!("contrast-more:ring-2 contrast-more:ring-accent");
    let _ = pc!("w-[calc(100%-1rem)] [mask-type:luminance]");
}
`;

function fail(message) {
  throw new Error(message);
}

function assert(condition, message) {
  if (!condition) fail(message);
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function run(program, args, options = {}) {
  const result = spawnSync(program, args, {
    cwd: root,
    encoding: "utf8",
    env: { ...process.env },
    maxBuffer: 16 * 1024 * 1024,
    timeout: 180_000,
    windowsHide: true,
    ...options,
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    fail(
      `${program} ${args.join(" ")} exited ${result.status}\n` +
        `stdout:\n${result.stdout}\nstderr:\n${result.stderr}`,
    );
  }
  return result;
}

function runAsync(program, args, options = {}) {
  return new Promise((resolvePromise, reject) => {
    const child = spawn(program, args, {
      cwd: project,
      env: { ...process.env },
      stdio: ["ignore", "pipe", "pipe"],
      windowsHide: true,
      ...options,
    });
    const timeout = setTimeout(() => {
      child.kill();
      reject(new Error(`${program} ${args.join(" ")} exceeded 60 seconds`));
    }, 60_000);
    const stdout = [];
    const stderr = [];
    child.stdout.on("data", (chunk) => stdout.push(chunk));
    child.stderr.on("data", (chunk) => stderr.push(chunk));
    child.once("error", (error) => {
      clearTimeout(timeout);
      reject(error);
    });
    child.once("close", (code, signal) => {
      clearTimeout(timeout);
      resolvePromise({
        code,
        signal,
        stdout: Buffer.concat(stdout).toString("utf8"),
        stderr: Buffer.concat(stderr).toString("utf8"),
      });
    });
  });
}

function resolveExecutable() {
  if (process.env.PLIEGO_CSSC) {
    const configured = resolve(root, process.env.PLIEGO_CSSC);
    assert(existsSync(configured), `PLIEGO_CSSC does not exist: ${configured}`);
    return configured;
  }
  run("cargo", ["+1.85.0", "build", "--locked", "-p", "pliego-cssc"]);
  const executable = join(
    root,
    "target",
    "debug",
    process.platform === "win32" ? "pliego-cssc.exe" : "pliego-cssc",
  );
  assert(existsSync(executable), `cargo did not produce ${executable}`);
  return executable;
}

function resolveLockHolder() {
  if (process.env.PLIEGO_LOCK_HOLDER) {
    const configured = resolve(root, process.env.PLIEGO_LOCK_HOLDER);
    assert(existsSync(configured), `PLIEGO_LOCK_HOLDER does not exist: ${configured}`);
    return configured;
  }
  run("cargo", [
    "+1.85.0",
    "build",
    "--locked",
    "--manifest-path",
    "integration-tests/publication-lock-holder/Cargo.toml",
    "--target-dir",
    "target",
  ]);
  const executable = join(
    root,
    "target",
    "debug",
    process.platform === "win32"
      ? "pliego-publication-lock-holder.exe"
      : "pliego-publication-lock-holder",
  );
  assert(existsSync(executable), `cargo did not produce ${executable}`);
  return executable;
}

function publicationLockPath(destination) {
  const absolute = join(project, destination);
  return join(dirname(absolute), `.${basename(absolute)}.pliego.lock`);
}

function startLockHolder(executable, lockPaths) {
  const child = spawn(executable, lockPaths, {
    cwd: project,
    env: { ...process.env },
    stdio: ["pipe", "pipe", "pipe"],
    windowsHide: true,
  });
  const stderr = [];
  let stdout = "";
  let readySettled = false;
  let readyTimer;
  let resolveReady;
  let rejectReady;
  const ready = new Promise((resolvePromise, reject) => {
    resolveReady = resolvePromise;
    rejectReady = reject;
  });
  const closed = new Promise((resolvePromise, reject) => {
    child.once("error", (error) => {
      if (!readySettled) {
        readySettled = true;
        clearTimeout(readyTimer);
        rejectReady(error);
      }
      reject(error);
    });
    child.once("close", (code, signal) => {
      if (!readySettled) {
        readySettled = true;
        clearTimeout(readyTimer);
        rejectReady(
          new Error(
            `lock holder exited before ready: code=${code} signal=${signal} stderr=${Buffer.concat(stderr)}`,
          ),
        );
      }
      if (code === 0 && signal === null) resolvePromise();
      else {
        reject(
          new Error(
            `lock holder failed: code=${code} signal=${signal} stderr=${Buffer.concat(stderr)}`,
          ),
        );
      }
    });
  });
  child.stderr.on("data", (chunk) => stderr.push(chunk));
  child.stdout.on("data", (chunk) => {
    stdout += chunk.toString("utf8");
    if (!readySettled && stdout.includes("\n")) {
      readySettled = true;
      clearTimeout(readyTimer);
      if (stdout === "ready\n" || stdout === "ready\r\n") resolveReady();
      else rejectReady(new Error(`unexpected lock-holder stdout: ${JSON.stringify(stdout)}`));
    }
  });
  readyTimer = setTimeout(() => {
    if (!readySettled) {
      readySettled = true;
      child.kill();
      rejectReady(new Error("lock holder did not become ready within 10 seconds"));
    }
  }, 10_000);
  return { child, ready, closed };
}

function sourceSites(bytes) {
  const sites = [];
  let offset = 0;
  while (true) {
    const start = bytes.indexOf('pc!("', offset);
    if (start < 0) break;
    const end = bytes.indexOf('");', start);
    assert(end >= 0, "parallel fixture contains an unterminated pc! invocation");
    sites.push({ file: "src/app.rs", byteStart: start, byteEnd: end + 2 });
    offset = end + 2;
  }
  assert(sites.length === 6, `parallel fixture contains ${sites.length} sites instead of 6`);
  return sites;
}

function compileArguments({ css, manifest, targets, format }) {
  return [
    "compile",
    "--source",
    "src/app.rs",
    "--seed",
    "--theme",
    "--targets",
    targets,
    "--format",
    format,
    "--output",
    css,
    "--manifest",
    manifest,
    "--manifest-version",
    "5",
    "--reachability",
    "pliego.reachability.json",
  ];
}

function readPair(css, manifest) {
  return {
    css: readFileSync(join(project, css)),
    manifest: readFileSync(join(project, manifest)),
  };
}

function assertPair(actual, expected, context) {
  assert(
    actual.css.equals(expected.css),
    `${context} CSS drifted: ${sha256(actual.css)} != ${sha256(expected.css)}`,
  );
  assert(
    actual.manifest.equals(expected.manifest),
    `${context} manifest drifted: ${sha256(actual.manifest)} != ${sha256(expected.manifest)}`,
  );
}

function walk(directory) {
  const output = [];
  for (const name of readdirSync(directory)) {
    const path = join(directory, name);
    if (statSync(path).isDirectory()) output.push(...walk(path));
    else output.push(path);
  }
  return output;
}

async function runUniqueCohort(executable, profile, reference) {
  const jobs = Array.from({ length: workers }, (_, index) => {
    const css = `out/${profile.name}/worker-${index}.css`;
    const manifest = `out/${profile.name}/worker-${index}.manifest.json`;
    return {
      css,
      manifest,
      promise: runAsync(
        executable,
        compileArguments({ css, manifest, targets: profile.targets, format: profile.format }),
      ),
    };
  });
  const results = await Promise.all(jobs.map((job) => job.promise));
  for (const [index, result] of results.entries()) {
    assert(result.code === 0 && result.signal === null, `${profile.name} worker ${index} failed`);
    assert(result.stdout === "", `${profile.name} worker ${index} emitted stdout`);
    assert(result.stderr === "", `${profile.name} worker ${index} emitted stderr`);
    assertPair(readPair(jobs[index].css, jobs[index].manifest), reference, `${profile.name} worker ${index}`);
  }
}

async function runSharedCohort(executable, profile, reference) {
  const css = `shared/${profile.name}.css`;
  const manifest = `shared/${profile.name}.manifest.json`;
  const args = compileArguments({ css, manifest, targets: profile.targets, format: profile.format });
  const results = await Promise.all(
    Array.from({ length: workers }, () => runAsync(executable, args)),
  );
  let successful = 0;
  let locked = 0;
  for (const [index, result] of results.entries()) {
    if (result.code === 0) {
      successful += 1;
      assert(result.signal === null, `${profile.name} shared worker ${index} exited by signal`);
      assert(result.stdout === "", `${profile.name} shared worker ${index} emitted stdout`);
      assert(result.stderr === "", `${profile.name} shared worker ${index} emitted stderr`);
      continue;
    }
    assert(result.stdout === "", `${profile.name} locked worker ${index} emitted stdout`);
    assert(
      /cannot acquire publication lock .*another writer may be active/isu.test(result.stderr),
      `${profile.name} worker ${index} failed outside the lock contract:\n${result.stderr}`,
    );
    locked += 1;
  }
  assert(successful >= 1, `${profile.name} shared cohort had no successful publisher`);
  assert(successful + locked === workers, `${profile.name} shared cohort lost a worker result`);
  assertPair(readPair(css, manifest), reference, `${profile.name} shared destination`);
  return { successful, locked };
}

async function runCrossProfileCohort(executable, profiles) {
  const css = "shared/cross-profile.css";
  const manifest = "shared/cross-profile.manifest.json";
  const results = await Promise.all(
    Array.from({ length: workers }, (_, index) => {
      const profile = profiles[index % profiles.length];
      return runAsync(
        executable,
        compileArguments({ css, manifest, targets: profile.targets, format: profile.format }),
      );
    }),
  );
  let successful = 0;
  let locked = 0;
  for (const [index, result] of results.entries()) {
    if (result.code === 0) {
      successful += 1;
      assert(result.signal === null, `cross-profile worker ${index} exited by signal`);
      assert(result.stdout === "", `cross-profile worker ${index} emitted stdout`);
      assert(result.stderr === "", `cross-profile worker ${index} emitted stderr`);
      continue;
    }
    assert(result.stdout === "", `cross-profile locked worker ${index} emitted stdout`);
    assert(
      /cannot acquire publication lock .*another writer may be active/isu.test(result.stderr),
      `cross-profile worker ${index} failed outside the lock contract:\n${result.stderr}`,
    );
    locked += 1;
  }
  assert(successful >= 1, "cross-profile shared cohort had no successful publisher");
  assert(successful + locked === workers, "cross-profile shared cohort lost a worker result");
  const finalPair = readPair(css, manifest);
  const winner = profiles.find(
    (profile) =>
      finalPair.css.equals(profile.reference.css) &&
      finalPair.manifest.equals(profile.reference.manifest),
  );
  assert(winner, "cross-profile shared destination contains a torn or unknown CSS/manifest pair");
  return { successful, locked, finalProfile: winner.name };
}

async function runDeterministicLockProbe(executable, lockHolder, profile) {
  const css = "locked/probe.css";
  const manifest = "locked/probe.manifest.json";
  mkdirSync(join(project, "locked"), { recursive: true });
  const holder = startLockHolder(lockHolder, [
    publicationLockPath(css),
    publicationLockPath(manifest),
  ]);
  await holder.ready;
  let result;
  try {
    result = await runAsync(
      executable,
      compileArguments({ css, manifest, targets: profile.targets, format: profile.format }),
    );
  } finally {
    holder.child.stdin.end();
    await holder.closed;
  }
  assert(result.code !== 0 && result.signal === null, "locked writer unexpectedly succeeded");
  assert(result.stdout === "", "locked writer emitted stdout");
  assert(
    /cannot acquire publication lock .*another writer may be active/isu.test(result.stderr),
    `locked writer failed outside the lock contract:\n${result.stderr}`,
  );
  assert(!existsSync(join(project, css)), "locked writer created CSS");
  assert(!existsSync(join(project, manifest)), "locked writer created a manifest");
  return { rejected: 1, diagnostic: "publication-lock" };
}

async function main() {
  rmSync(runtime, { recursive: true, force: true });
  mkdirSync(join(project, "src"), { recursive: true });
  const source = Buffer.from(sourceText);
  writeFileSync(join(project, "src", "app.rs"), source);
  writeFileSync(
    join(project, "pliego.reachability.json"),
    `${JSON.stringify(
      {
        schema: 1,
        applicationCoverage: "complete",
        components: [{ id: "app::root", sites: sourceSites(source) }],
        routes: [{ id: "home", path: "/", components: ["app::root"] }],
        islands: [],
      },
      null,
      2,
    )}\n`,
  );

  const executable = resolveExecutable();
  const lockHolder = resolveLockHolder();
  const profiles = [
    { name: "modern-minified", targets: "modern", format: "minified" },
    { name: "none-pretty", targets: "none", format: "pretty" },
  ];
  const summary = [];
  const profileReferences = [];
  for (const profile of profiles) {
    mkdirSync(join(project, "reference"), { recursive: true });
    mkdirSync(join(project, "shared"), { recursive: true });
    mkdirSync(join(project, "out", profile.name), { recursive: true });
    const css = `reference/${profile.name}.css`;
    const manifest = `reference/${profile.name}.manifest.json`;
    const baseline = run(executable, compileArguments({ css, manifest, ...profile }), {
      cwd: project,
    });
    assert(baseline.stdout === "" && baseline.stderr === "", `${profile.name} baseline was noisy`);
    const reference = readPair(css, manifest);
    const document = JSON.parse(reference.manifest.toString("utf8"));
    assert(document.schemaVersion === 5, `${profile.name} did not emit manifest schema 5`);
    assert(document.graph?.schemaVersion === 2, `${profile.name} did not emit graph schema 2`);
    profileReferences.push({ ...profile, reference });
    await runUniqueCohort(executable, profile, reference);
    const shared = await runSharedCohort(executable, profile, reference);
    summary.push({
      profile: profile.name,
      workers,
      cssSha256: sha256(reference.css),
      manifestSha256: sha256(reference.manifest),
      shared,
    });
  }
  const deterministicLockProbe = await runDeterministicLockProbe(
    executable,
    lockHolder,
    profileReferences[0],
  );
  const crossProfileShared = await runCrossProfileCohort(executable, profileReferences);

  const residue = walk(project)
    .map((path) => relative(project, path).replaceAll("\\", "/"))
    .filter((path) => /\.pliego-.*\.(?:tmp|bak)$/u.test(path));
  assert(residue.length === 0, `parallel publication left temporary/backup files: ${residue}`);
  console.log(
    JSON.stringify(
      {
        schemaVersion: 1,
        workersPerCohort: workers,
        maximumConcurrentCompilerProcesses: workers,
        referenceProcesses: profiles.length,
        uniqueProcesses: workers * profiles.length,
        sharedProcesses: workers * profiles.length,
        crossProfileSharedProcesses: workers,
        deterministicLockProbeProcesses: 1,
        workerProcesses: workers * (profiles.length * 2 + 1),
        totalCompilerInvocations: profiles.length + workers * (profiles.length * 2 + 1) + 1,
        profiles: summary,
        deterministicLockProbe,
        crossProfileShared,
        temporaryOrBackupResidue: 0,
      },
      null,
      2,
    ),
  );
  rmSync(runtime, { recursive: true, force: true });
}

main().catch((error) => {
  console.error(`${error.stack ?? error}\nreproduction retained at ${runtime}`);
  process.exitCode = 1;
});
