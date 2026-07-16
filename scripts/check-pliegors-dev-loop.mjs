import { spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  closeSync,
  cpSync,
  existsSync,
  mkdirSync,
  openSync,
  readFileSync,
  realpathSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { createServer } from "node:net";
import { homedir, release as operatingSystemRelease } from "node:os";
import { basename, dirname, isAbsolute, join, relative, resolve } from "node:path";
import { performance } from "node:perf_hooks";
import { fileURLToPath } from "node:url";
import { verifyPliegorsContract } from "./pliegors-contract.mjs";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const FIXTURE = join(ROOT, "integration-tests", "pliegors-dev-loop");
const PLIEGORS_ROOT = process.env.PLIEGORS_ROOT
  ? resolve(process.env.PLIEGORS_ROOT)
  : resolve(ROOT, "..", "pliegors");
const CONTRACT_PATH = join(
  ROOT,
  "integration-tests",
  "pliegors-smoke",
  "pliegors-contract.json",
);
const TARGET_ROOT = join(ROOT, "target", "pliegors-dev-loop");
const IS_WSL =
  process.platform === "linux" &&
  existsSync("/proc/version") &&
  /microsoft|wsl/iu.test(readFileSync("/proc/version", "utf8"));
const BUILD_TARGET = process.env.PLIEGO_DEV_TARGET_DIR
  ? resolve(process.env.PLIEGO_DEV_TARGET_DIR)
  : process.platform === "win32"
    ? join(ROOT, "target")
    : IS_WSL
      ? join(homedir(), ".cache", "pliegocss-dev-target")
      : join(TARGET_ROOT, "linux-target");
const BROWSER_RUNTIME = join(TARGET_ROOT, "browser");
const BROWSER_STATE = join(TARGET_ROOT, "browser-state.json");
const BROWSER_COMMAND = join(TARGET_ROOT, "browser-command.json");
const BROWSER_RESULT = join(TARGET_ROOT, "browser-result.json");
const BROWSER_LOCK = join(TARGET_ROOT, "browser.lock");
const EVIDENCE_PATH = join(
  ROOT,
  "docs",
  "benchmarks",
  "data",
  "pliegors-dev-loop.local.json",
);
const TOOLCHAIN = "1.85";
const START_TIMEOUT_MS = 120_000;
const CHANGE_TIMEOUT_MS = 45_000;
const QUIET_WINDOW_MS = 2_000;
const RELOAD_SETTLE_MS = 2_000;
const POLL_MS = 25;
const VALID_SAMPLE_COUNT = 20;
const ACTIVE_PROCESSES = new Set();
const BROWSER_ACTIONS = new Set(["valid", "invalid", "restore", "equivalent", "stop"]);

function fail(message) {
  throw new Error(message);
}

function delay(milliseconds) {
  return new Promise((resolveDelay) => setTimeout(resolveDelay, milliseconds));
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function fileSha256(path) {
  return sha256(readFileSync(path));
}

function normalizedPath(path) {
  return resolve(path).replaceAll("\\", "/");
}

function writeFileAtomic(path, content) {
  mkdirSync(dirname(path), { recursive: true });
  const temporary = join(
    dirname(path),
    `.${basename(path)}.pliego-${process.pid}-${Date.now()}.tmp`,
  );
  writeFileSync(temporary, content);
  renameSync(temporary, path);
}

function writeJsonAtomic(path, value) {
  writeFileAtomic(path, `${JSON.stringify(value, null, 2)}\n`);
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: ROOT,
    env: process.env,
    encoding: "utf8",
    windowsHide: true,
    ...options,
  });
  if (result.error) fail(`cannot run ${command}: ${result.error.message}`);
  if (result.status !== 0) {
    fail(
      `${command} ${args.join(" ")} failed\n${result.stdout ?? ""}${result.stderr ?? ""}`,
    );
  }
  return result.stdout ?? "";
}

function safeRuntime(path) {
  const root = realpathSync(ROOT);
  const candidate = resolve(path);
  const targetRoot = resolve(ROOT, "target", "pliegors-dev-loop");
  const fromRoot = relative(root, candidate);
  const fromTarget = relative(targetRoot, candidate);
  if (
    !fromRoot ||
    fromRoot.startsWith("..") ||
    isAbsolute(fromRoot) ||
    !fromTarget ||
    fromTarget.startsWith("..") ||
    isAbsolute(fromTarget)
  ) {
    fail(`refusing unsafe development-loop runtime ${candidate}`);
  }
  return candidate;
}

function materializeFixture(runtime) {
  mkdirSync(runtime, { recursive: true });
  const project = join(runtime, "project");
  cpSync(FIXTURE, project, { recursive: true, force: false });
  const manifestPath = join(project, "Cargo.toml");
  let manifest = readFileSync(manifestPath, "utf8");
  const replacements = new Map([
    ["../../crates/pliego-css", join(ROOT, "crates", "pliego-css")],
    ["../../../pliegors/crates/pliego-dom", join(PLIEGORS_ROOT, "crates", "pliego-dom")],
    ["../../../pliegors/crates/pliego-ssg", join(PLIEGORS_ROOT, "crates", "pliego-ssg")],
  ]);
  for (const [from, to] of replacements) {
    const before = `path = ${JSON.stringify(from)}`;
    const after = `path = ${JSON.stringify(normalizedPath(to))}`;
    if (!manifest.includes(before)) fail(`fixture manifest is missing ${before}`);
    manifest = manifest.replace(before, after);
  }
  writeFileSync(manifestPath, manifest);
  const driver = join(runtime, "driver.styles.txt");
  writeFileSync(driver, "block\n");
  return {
    project,
    driver,
    source: join(project, "src", "main.rs"),
    css: join(project, "assets", "pliego.css"),
    manifest: join(project, "assets", "pliego.manifest.json"),
  };
}

function sourceFor(probe) {
  const utility = probe === "invalid" ? "p-not-a-token" : probe;
  const initial = readFileSync(join(FIXTURE, "src", "main.rs"), "utf8");
  return initial
    .replace(
      'const PROBE_STYLE: &str = "p-4";',
      `const PROBE_STYLE: &str = ${JSON.stringify(probe)};`,
    )
    .replace(
      'pc!("flex p-4 bg-surface text-ink")',
      `pc!(${JSON.stringify(`flex ${utility} bg-surface text-ink`)})`,
    );
}

function executable(name) {
  return join(BUILD_TARGET, "debug", `${name}${process.platform === "win32" ? ".exe" : ""}`);
}

function buildTools(environment) {
  run("cargo", [`+${TOOLCHAIN}`, "build", "--locked", "-p", "pliego-cssc"], {
    env: environment,
  });
  run(
    "cargo",
    [
      `+${TOOLCHAIN}`,
      "build",
      "--locked",
      "--manifest-path",
      join(PLIEGORS_ROOT, "Cargo.toml"),
      "-p",
      "pliego-cli",
    ],
    { env: environment },
  );
  const cssc = executable("pliego-cssc");
  const pliego = executable("pliego");
  if (!existsSync(cssc) || !existsSync(pliego)) {
    fail("development-loop tool binaries were not produced under the shared target directory");
  }
  return { cssc, pliego };
}

function spawnLogged(name, command, args, options) {
  const child = spawn(command, args, {
    ...options,
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: true,
    detached: process.platform !== "win32",
  });
  const processLog = { name, child, output: "" };
  ACTIVE_PROCESSES.add(processLog);
  const consume = (chunk) => {
    processLog.output += chunk.toString("utf8").replace(/\u001B\[[0-9;]*m/gu, "");
    if (processLog.output.length > 200_000) {
      processLog.output = processLog.output.slice(-100_000);
    }
  };
  child.stdout.on("data", consume);
  child.stderr.on("data", consume);
  child.on("error", (error) => consume(`\nprocess error: ${error.message}\n`));
  child.once("exit", () => ACTIVE_PROCESSES.delete(processLog));
  return processLog;
}

function hasExited(processLog) {
  return (
    processLog.child.exitCode !== null || processLog.child.signalCode !== null
  );
}

function assertRunning(processLog) {
  if (hasExited(processLog)) {
    fail(
      `${processLog.name} exited with ${processLog.child.exitCode ?? processLog.child.signalCode}\n${processLog.output}`,
    );
  }
}

async function waitUntil(description, predicate, timeout = CHANGE_TIMEOUT_MS, processes = []) {
  const deadline = performance.now() + timeout;
  let lastError;
  while (performance.now() < deadline) {
    for (const processLog of processes) assertRunning(processLog);
    try {
      const value = await predicate();
      if (value) return value;
    } catch (error) {
      lastError = error;
    }
    await delay(POLL_MS);
  }
  fail(
    `timed out waiting for ${description}${lastError ? `: ${lastError.message}` : ""}\n${processes.map((entry) => `${entry.name}:\n${entry.output}`).join("\n")}`,
  );
}

async function waitForLog(processLog, offset, text, timeout = CHANGE_TIMEOUT_MS) {
  return waitUntil(
    `${processLog.name} log ${JSON.stringify(text)}`,
    () => processLog.output.slice(offset).includes(text),
    timeout,
    [processLog],
  );
}

async function freePort() {
  const server = createServer();
  await new Promise((resolveListen, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolveListen);
  });
  const address = server.address();
  const port = typeof address === "object" && address ? address.port : 0;
  await new Promise((resolveClose, reject) =>
    server.close((error) => (error ? reject(error) : resolveClose())),
  );
  if (!port) fail("cannot allocate a loopback test port");
  return port;
}

async function fetchText(url) {
  const response = await fetch(url, { cache: "no-store" });
  if (!response.ok) fail(`${url} returned HTTP ${response.status}`);
  return response.text();
}

async function fetchBytes(url) {
  const response = await fetch(url, { cache: "no-store" });
  if (!response.ok) fail(`${url} returned HTTP ${response.status}`);
  return new Uint8Array(await response.arrayBuffer());
}

function attribute(tag, name) {
  return new RegExp(`\\b${name}="([^"]*)"`, "u").exec(tag)?.[1];
}

function htmlState(html) {
  const tag = html.match(/<main\b[^>]*>/gu)?.find((candidate) => candidate.includes('id="style-probe"'));
  const generation = Number(/\/_pliego\/reload\?since=(\d+)/u.exec(html)?.[1]);
  if (!tag || !Number.isSafeInteger(generation)) {
    fail("development HTML is missing the style probe or reload generation");
  }
  const probe = attribute(tag, "data-style-probe");
  const className = attribute(tag, "class");
  if (!probe || !/^pc_[a-z0-9]+$/u.test(className ?? "")) {
    fail(`development HTML has an invalid probe/class: ${tag}`);
  }
  return { probe, className, generation };
}

function expectedPadding(probe) {
  if (probe === "p-4") return "1rem";
  if (probe === "p-6") return "1.5rem";
  fail(`development HTML exposed unexpected probe ${JSON.stringify(probe)}`);
}

async function pageState(url, cssPath) {
  const before = htmlState(await fetchText(url));
  const cssBytes = await fetchBytes(new URL("/assets/pliego.css", url));
  const after = htmlState(await fetchText(url));
  if (
    before.probe !== after.probe ||
    before.className !== after.className ||
    before.generation !== after.generation
  ) {
    fail("PliegoRS page changed while capturing one HTML/CSS state");
  }
  const css = new TextDecoder().decode(cssBytes);
  const cssSha256 = sha256(cssBytes);
  const cssDiskSha256 = fileSha256(cssPath);
  if (cssSha256 !== cssDiskSha256) {
    fail("served CSS bytes do not match the compiler's published artifact");
  }
  const classRules = [...css.matchAll(/\.(pc_[a-z0-9]+)\s*\{/gu)].map(
    (match) => match[1],
  );
  const currentClassRules = classRules.filter(
    (className) => className === before.className,
  );
  if (currentClassRules.length !== 1) {
    fail(
      `served CSS must contain the current generated class exactly once; got ${JSON.stringify(classRules)}`,
    );
  }
  const rule = new RegExp(`\\.${before.className}\\s*\\{([^}]*)\\}`, "u").exec(css)?.[1];
  const padding = /\bpadding\s*:\s*([^;]+);/u.exec(rule ?? "")?.[1]?.trim();
  const wantedPadding = expectedPadding(before.probe);
  if (padding !== wantedPadding) {
    fail(
      `served ${before.className} has padding ${JSON.stringify(padding)}, expected ${wantedPadding}`,
    );
  }
  return {
    ...before,
    padding,
    cssSha256,
    cssDiskSha256,
    cssClassCount: classRules.length,
    cssClasses: classRules,
  };
}

async function stablePageState(context, processes) {
  let previous;
  let equal = 0;
  return waitUntil(
    "a stable PliegoRS reload generation",
    async () => {
      const current = await pageState(context.url, context.paths.css);
      if (
        previous &&
        current.generation === previous.generation &&
        current.probe === previous.probe &&
        current.className === previous.className &&
        current.cssSha256 === previous.cssSha256 &&
        current.padding === previous.padding
      ) {
        equal += 1;
      }
      else equal = 0;
      previous = current;
      return equal >= 3 ? current : null;
    },
    CHANGE_TIMEOUT_MS,
    processes,
  );
}

function samePageState(left, right) {
  return (
    left.generation === right.generation &&
    left.probe === right.probe &&
    left.className === right.className &&
    left.padding === right.padding &&
    left.cssSha256 === right.cssSha256 &&
    left.cssDiskSha256 === right.cssDiskSha256 &&
    left.cssClassCount === right.cssClassCount &&
    JSON.stringify(left.cssClasses) === JSON.stringify(right.cssClasses)
  );
}

async function assertPageStateFor(context, expected, duration, processes) {
  const deadline = performance.now() + duration;
  do {
    for (const processLog of processes) assertRunning(processLog);
    const current = await pageState(context.url, context.paths.css);
    if (!samePageState(current, expected)) {
      fail(
        `page state changed during the ${duration} ms settlement window: ${JSON.stringify({ expected, current })}`,
      );
    }
    await delay(POLL_MS);
  } while (performance.now() < deadline);
}

async function waitForReload(url, since, processes) {
  return waitUntil(
    `SSE generation after ${since}`,
    async () => {
      const controller = new AbortController();
      const timer = setTimeout(() => controller.abort(), 30_000);
      try {
        const response = await fetch(
          new URL(`/_pliego/reload?since=${since}`, url),
          { signal: controller.signal },
        );
        if (!response.ok) {
          fail(`PliegoRS reload endpoint returned HTTP ${response.status}`);
        }
        const body = await response.text();
        const generation = Number(/^data: (\d+)$/mu.exec(body)?.[1]);
        return Number.isSafeInteger(generation) && generation > since ? generation : null;
      } finally {
        clearTimeout(timer);
      }
    },
    35_000,
    processes,
  );
}

async function mutateValid(context, probe) {
  const processes = [context.cssWatch, context.pliegoDev];
  const before = await stablePageState(context, processes);
  const previousCss = fileSha256(context.paths.css);
  const started = performance.now();
  const reload = waitForReload(context.url, before.generation, processes);
  writeFileSync(context.paths.source, sourceFor(probe));
  const cssChangedAt = await waitUntil(
    `CSS publication for ${probe}`,
    () => fileSha256(context.paths.css) !== previousCss && performance.now(),
    CHANGE_TIMEOUT_MS,
    processes,
  );
  const sseGeneration = await reload;
  if (sseGeneration !== before.generation + 1) {
    fail(
      `valid edit advanced SSE from ${before.generation} to ${sseGeneration}; exactly one coherent rebuild is required`,
    );
  }
  const converged = await waitUntil(
    `coherent generation ${sseGeneration} for ${probe}`,
    async () => {
      const state = await pageState(context.url, context.paths.css);
      return state.probe === probe && state.generation === sseGeneration
        ? state
        : null;
    },
    CHANGE_TIMEOUT_MS,
    processes,
  );
  const convergedAt = performance.now();
  if (converged.cssClasses.includes(before.className)) {
    fail(`valid ${probe} edit retained stale class ${before.className} in served CSS`);
  }
  await assertPageStateFor(context, converged, RELOAD_SETTLE_MS, processes);
  return {
    probe,
    editToCssMs: Number((cssChangedAt - started).toFixed(3)),
    editToSiteSseMs: Number((convergedAt - started).toFixed(3)),
    generationBefore: before.generation,
    generationAfter: converged.generation,
    sseGeneration,
    className: converged.className,
    padding: converged.padding,
    cssSha256: converged.cssSha256,
    cssMatchesDisk: converged.cssSha256 === converged.cssDiskSha256,
    reloadSettledMs: RELOAD_SETTLE_MS,
  };
}

async function proveInvalidRetainsLastValid(context) {
  const processes = [context.cssWatch, context.pliegoDev];
  const before = await stablePageState(context, processes);
  const cssDigest = fileSha256(context.paths.css);
  const manifestDigest = fileSha256(context.paths.manifest);
  const cssModified = statSync(context.paths.css).mtimeMs;
  const manifestModified = statSync(context.paths.manifest).mtimeMs;
  const watchOffset = context.cssWatch.output.length;
  const devOffset = context.pliegoDev.output.length;
  writeFileSync(context.paths.source, sourceFor("invalid"));
  await Promise.all([
    waitForLog(context.cssWatch, watchOffset, "compile failed; keeping the last valid artifact"),
    waitForLog(context.pliegoDev, devOffset, "PLIEGO dev: rebuild failed"),
  ]);
  await delay(QUIET_WINDOW_MS);
  const after = await pageState(context.url, context.paths.css);
  if (
    !samePageState(after, before) ||
    fileSha256(context.paths.css) !== cssDigest ||
    fileSha256(context.paths.manifest) !== manifestDigest ||
    statSync(context.paths.css).mtimeMs !== cssModified ||
    statSync(context.paths.manifest).mtimeMs !== manifestModified
  ) {
    fail("invalid edit changed the last valid publication group, page, or reload generation");
  }
  return {
    generation: after.generation,
    retainedProbe: after.probe,
    retainedClassName: after.className,
    cssSha256: cssDigest,
    manifestSha256: manifestDigest,
    publicationMtimePreserved: true,
    quietWindowMs: QUIET_WINDOW_MS,
  };
}

async function restoreLastValid(context, probe = "p-6") {
  const processes = [context.cssWatch, context.pliegoDev];
  const before = await stablePageState(context, processes);
  const devOffset = context.pliegoDev.output.length;
  const reload = waitForReload(context.url, before.generation, processes);
  writeFileSync(context.paths.source, sourceFor(probe));
  await waitForLog(context.pliegoDev, devOffset, "PLIEGO dev: rebuilt");
  const sseGeneration = await reload;
  if (sseGeneration !== before.generation + 1) {
    fail(
      `valid restoration advanced SSE from ${before.generation} to ${sseGeneration}; exactly one rebuild is required`,
    );
  }
  const after = await waitUntil(
    `restored ${probe} page`,
    async () => {
      const state = await pageState(context.url, context.paths.css);
      return state.probe === probe && state.generation === sseGeneration ? state : null;
    },
    CHANGE_TIMEOUT_MS,
    [context.cssWatch, context.pliegoDev],
  );
  await assertPageStateFor(context, after, RELOAD_SETTLE_MS, processes);
  return {
    generationBefore: before.generation,
    generationAfter: after.generation,
    sseGeneration,
    probe,
    className: after.className,
    padding: after.padding,
    cssSha256: after.cssSha256,
  };
}

async function proveEquivalentInputIsNoop(context) {
  const processes = [context.cssWatch, context.pliegoDev];
  const before = await stablePageState(context, processes);
  const cssDigest = fileSha256(context.paths.css);
  const manifestDigest = fileSha256(context.paths.manifest);
  const cssModified = statSync(context.paths.css).mtimeMs;
  const manifestModified = statSync(context.paths.manifest).mtimeMs;
  const watchOffset = context.cssWatch.output.length;
  writeFileSync(context.paths.driver, "block\n\n");
  await waitForLog(
    context.cssWatch,
    watchOffset,
    "generated artifact bytes are unchanged",
  );
  await delay(QUIET_WINDOW_MS);
  const after = await pageState(context.url, context.paths.css);
  if (
    !samePageState(after, before) ||
    fileSha256(context.paths.css) !== cssDigest ||
    fileSha256(context.paths.manifest) !== manifestDigest ||
    statSync(context.paths.css).mtimeMs !== cssModified ||
    statSync(context.paths.manifest).mtimeMs !== manifestModified
  ) {
    fail("equivalent external input replaced the publication group or advanced reload");
  }
  return {
    generation: after.generation,
    cssSha256: cssDigest,
    manifestSha256: manifestDigest,
    publicationMtimePreserved: true,
    quietWindowMs: QUIET_WINDOW_MS,
  };
}

function percentile(values, fraction) {
  const sorted = [...values].sort((left, right) => left - right);
  return sorted[Math.max(0, Math.ceil(sorted.length * fraction) - 1)];
}

function latencySummary(samples, field) {
  const values = samples.map((sample) => sample[field]);
  return {
    p50: percentile(values, 0.5),
    p95: percentile(values, 0.95),
    min: Math.min(...values),
    max: Math.max(...values),
  };
}

async function waitForProcessExit(processLog, timeout) {
  if (hasExited(processLog)) return true;
  return Promise.race([
    new Promise((resolveExit) =>
      processLog.child.once("exit", () => resolveExit(true)),
    ),
    delay(timeout).then(() => false),
  ]);
}

function signalProcessTree(processLog, signal, force = false) {
  if (hasExited(processLog)) return;
  if (process.platform === "win32") {
    spawnSync(
      "taskkill",
      ["/PID", String(processLog.child.pid), "/T", ...(force ? ["/F"] : [])],
      { windowsHide: true, encoding: "utf8" },
    );
    return;
  }
  try {
    process.kill(-processLog.child.pid, signal);
  } catch (error) {
    if (error.code !== "ESRCH") throw error;
  }
}

async function stopProcess(processLog) {
  if (!processLog || hasExited(processLog)) return;
  signalProcessTree(processLog, "SIGTERM");
  if (!(await waitForProcessExit(processLog, 2_000))) {
    signalProcessTree(processLog, "SIGKILL", true);
    if (!(await waitForProcessExit(processLog, 2_000))) {
      fail(`could not stop ${processLog.name} process tree`);
    }
  }
  ACTIVE_PROCESSES.delete(processLog);
}

let handlingSignal = false;
for (const [signal, exitCode] of [
  ["SIGINT", 130],
  ["SIGTERM", 143],
]) {
  process.once(signal, async () => {
    if (handlingSignal) return;
    handlingSignal = true;
    try {
      await Promise.all([...ACTIVE_PROCESSES].map((entry) => stopProcess(entry)));
    } finally {
      process.exit(exitCode);
    }
  });
}

async function startPliegoDevelopmentServer(paths, tools, environment, cssWatch) {
  for (let attempt = 1; attempt <= 5; attempt += 1) {
    const port = await freePort();
    const url = `http://127.0.0.1:${port}/`;
    const pliegoDev = spawnLogged(
      "pliego dev",
      tools.pliego,
      ["dev", String(port)],
      { cwd: paths.project, env: environment },
    );
    try {
      await waitUntil(
        "initial PliegoRS development page",
        async () => {
          const state = await pageState(url, paths.css);
          return state.probe === "p-4" && state;
        },
        START_TIMEOUT_MS,
        [cssWatch, pliegoDev],
      );
      return { pliegoDev, url };
    } catch (error) {
      const portRace = /address already in use|os error (?:98|10048)/iu.test(
        pliegoDev.output,
      );
      await stopProcess(pliegoDev);
      if (!portRace || attempt === 5) throw error;
    }
  }
  fail("could not start PliegoRS on an ephemeral loopback port");
}

async function startHarness(runtime) {
  const contract = verifyPliegorsContract(PLIEGORS_ROOT, CONTRACT_PATH);
  const paths = materializeFixture(runtime);
  const environment = {
    ...process.env,
    CARGO_TARGET_DIR: BUILD_TARGET,
    CARGO_TERM_COLOR: "never",
    PLIEGORS_SOURCE_REV: contract.revision,
    RUSTUP_TOOLCHAIN: TOOLCHAIN,
  };
  const tools = buildTools(environment);
  const cssWatch = spawnLogged(
    "pliego-cssc watch",
    tools.cssc,
    [
      "watch",
      "--source",
      join(paths.project, "src"),
      "--input",
      paths.driver,
      "--theme",
      "--format",
      "pretty",
      "--output",
      paths.css,
      "--manifest",
      paths.manifest,
    ],
    { cwd: paths.project, env: environment },
  );
  let pliegoDev;
  try {
    await waitUntil(
      "initial PliegoCSS watch publication",
      () => {
        if (cssWatch.output.includes("compile failed; keeping the last valid artifact")) {
          fail(`initial PliegoCSS fixture is invalid\n${cssWatch.output}`);
        }
        return existsSync(paths.css) && existsSync(paths.manifest);
      },
      START_TIMEOUT_MS,
      [cssWatch],
    );
    const started = await startPliegoDevelopmentServer(
      paths,
      tools,
      environment,
      cssWatch,
    );
    pliegoDev = started.pliegoDev;
    const { url } = started;
    return { contract, paths, cssWatch, pliegoDev, url, runtime };
  } catch (error) {
    await stopProcess(pliegoDev);
    await stopProcess(cssWatch);
    throw error;
  }
}

async function stopHarness(context) {
  await stopProcess(context?.pliegoDev);
  await stopProcess(context?.cssWatch);
}

function sourceCacheEvidence(lines) {
  const latest = lines.at(-1);
  const match = latest?.match(
    /^source cache: (\d+) discovered, (\d+) scan hits?, (\d+) parsed, (\d+) semantic hits?, (\d+) lowered, (\d+) removed$/u,
  );
  if (!match) fail(`missing structured source-cache evidence: ${JSON.stringify(latest)}`);
  const [discovered, scanHits, parsed, semanticHits, lowered, removed] = match
    .slice(1)
    .map(Number);
  if (
    discovered !== 1 ||
    scanHits !== 1 ||
    parsed !== 0 ||
    semanticHits !== 1 ||
    lowered !== 0 ||
    removed !== 0
  ) {
    fail(`final no-op did not reuse both source caches: ${latest}`);
  }
  return {
    observations: lines.length,
    latest,
    discovered,
    scanHits,
    parsed,
    semanticHits,
    lowered,
    removed,
  };
}

function assertGenerationChain(samples, invalid, restored, equivalent) {
  for (let index = 1; index < samples.length; index += 1) {
    if (samples[index].generationBefore !== samples[index - 1].generationAfter) {
      fail(
        `reload generation advanced between valid samples ${index} and ${index + 1}`,
      );
    }
  }
  const lastGeneration = samples.at(-1)?.generationAfter;
  if (
    invalid.generation !== lastGeneration ||
    restored.generationBefore !== invalid.generation ||
    equivalent.generation !== restored.generationAfter
  ) {
    fail(
      `reload generation advanced outside an accepted build: ${JSON.stringify({ lastGeneration, invalid: invalid.generation, restored, equivalent: equivalent.generation })}`,
    );
  }
}

async function automatedCheck(recordEvidence = false) {
  const pliegocssRevision = run("git", ["rev-parse", "HEAD"]).trim();
  const dirtyAtStart = run("git", ["status", "--porcelain"]).trim();
  if (recordEvidence && dirtyAtStart) {
    fail("--record-evidence requires a clean PliegoCSS worktree");
  }
  mkdirSync(TARGET_ROOT, { recursive: true });
  const runtime = safeRuntime(
    join(TARGET_ROOT, `run-${process.pid}-${Date.now()}`),
  );
  let context;
  try {
    context = await startHarness(runtime);
    const samples = [];
    for (let index = 0; index < VALID_SAMPLE_COUNT; index += 1) {
      const probe = index % 2 === 0 ? "p-6" : "p-4";
      samples.push(await mutateValid(context, probe));
    }
    const invalid = await proveInvalidRetainsLastValid(context);
    const restored = await restoreLastValid(context);
    const equivalent = await proveEquivalentInputIsNoop(context);
    assertGenerationChain(samples, invalid, restored, equivalent);
    const sourceCacheLines = context.cssWatch.output
      .split(/\r?\n/u)
      .filter((line) => line.startsWith("source cache:"));
    const report = {
      schemaVersion: 2,
      status: "ok",
      environment: {
        node: process.version,
        rust: TOOLCHAIN,
        platform: process.platform,
        architecture: process.arch,
        osRelease: operatingSystemRelease(),
        wsl: IS_WSL,
        buildTarget: process.env.PLIEGO_DEV_TARGET_DIR
          ? "explicit"
          : IS_WSL
            ? "wsl-home-cache"
            : "default",
      },
      pliegocss: {
        revision: pliegocssRevision,
        dirtyAtStart: Boolean(dirtyAtStart),
      },
      pliegors: context.contract,
      sampleCount: samples.length,
      percentileMethod: "nearest-rank",
      samples,
      latencyMs: {
        editToCss: latencySummary(samples, "editToCssMs"),
        editToSiteSse: latencySummary(samples, "editToSiteSseMs"),
      },
      invalid,
      restored,
      equivalent,
      cache: sourceCacheEvidence(sourceCacheLines),
    };
    if (recordEvidence) writeJsonAtomic(EVIDENCE_PATH, report);
    process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
  } finally {
    await stopHarness(context);
    if (existsSync(runtime)) rmSync(runtime, { recursive: true, force: true });
  }
}

function writeBrowserState(context, phase, extra = {}) {
  writeJsonAtomic(BROWSER_STATE, {
    schemaVersion: 1,
    pid: process.pid,
    url: context.url,
    runtime: context.runtime,
    commandPath: BROWSER_COMMAND,
    resultPath: BROWSER_RESULT,
    phase,
    ...extra,
  });
}

function processIsRunning(pid) {
  if (!Number.isSafeInteger(pid) || pid <= 0) return false;
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    return error.code === "EPERM";
  }
}

function acquireBrowserLock() {
  for (let attempt = 0; attempt < 2; attempt += 1) {
    try {
      const descriptor = openSync(BROWSER_LOCK, "wx");
      writeFileSync(descriptor, `${process.pid}\n`);
      closeSync(descriptor);
      return;
    } catch (error) {
      if (error.code !== "EEXIST") throw error;
      const owner = Number(readFileSync(BROWSER_LOCK, "utf8").trim());
      if (processIsRunning(owner)) {
        fail(`browser harness is already running as process ${owner}`);
      }
      rmSync(BROWSER_LOCK, { force: true });
    }
  }
  fail("could not acquire the browser harness lock");
}

function releaseBrowserLock() {
  if (!existsSync(BROWSER_LOCK)) return;
  const owner = Number(readFileSync(BROWSER_LOCK, "utf8").trim());
  if (owner === process.pid) rmSync(BROWSER_LOCK, { force: true });
}

async function serveBrowser() {
  mkdirSync(TARGET_ROOT, { recursive: true });
  acquireBrowserLock();
  let context;
  let lastCommand;
  try {
    for (const path of [BROWSER_RUNTIME, BROWSER_STATE, BROWSER_COMMAND, BROWSER_RESULT]) {
      if (existsSync(path)) rmSync(path, { recursive: true, force: true });
    }
    const runtime = safeRuntime(BROWSER_RUNTIME);
    context = await startHarness(runtime);
    writeBrowserState(context, "initial");
    process.stdout.write(
      `${JSON.stringify({ status: "ready", url: context.url, state: BROWSER_STATE })}\n`,
    );
    for (;;) {
      if (!existsSync(BROWSER_COMMAND)) {
        await delay(50);
        continue;
      }
      const command = JSON.parse(readFileSync(BROWSER_COMMAND, "utf8"));
      if (
        typeof command.id !== "string" ||
        !BROWSER_ACTIONS.has(command.action) ||
        command.id === lastCommand
      ) {
        await delay(50);
        continue;
      }
      lastCommand = command.id;
      let result;
      if (command.action === "valid") {
        result = await mutateValid(context, "p-6");
      } else if (command.action === "invalid") {
        result = await proveInvalidRetainsLastValid(context);
      } else if (command.action === "restore") {
        result = await restoreLastValid(context);
      } else if (command.action === "equivalent") {
        result = await proveEquivalentInputIsNoop(context);
      } else if (command.action === "stop") {
        result = { stopped: true };
      } else {
        fail(`unknown browser harness action ${JSON.stringify(command.action)}`);
      }
      const response = { schemaVersion: 1, id: command.id, action: command.action, result };
      writeJsonAtomic(BROWSER_RESULT, response);
      writeBrowserState(context, command.action, { lastResult: response });
      process.stdout.write(`${JSON.stringify(response)}\n`);
      if (command.action === "stop") break;
    }
  } finally {
    await stopHarness(context);
    for (const path of [BROWSER_STATE, BROWSER_COMMAND]) {
      if (existsSync(path)) rmSync(path, { force: true });
    }
    releaseBrowserLock();
  }
}

async function browserCommand(action) {
  if (!BROWSER_ACTIONS.has(action)) {
    fail(`invalid browser harness action ${JSON.stringify(action)}`);
  }
  if (!existsSync(BROWSER_STATE)) fail("browser harness is not running");
  const state = JSON.parse(readFileSync(BROWSER_STATE, "utf8"));
  if (
    state.schemaVersion !== 1 ||
    !processIsRunning(state.pid) ||
    typeof state.runtime !== "string" ||
    typeof state.commandPath !== "string" ||
    typeof state.resultPath !== "string" ||
    resolve(state.runtime) !== resolve(BROWSER_RUNTIME) ||
    resolve(state.commandPath) !== resolve(BROWSER_COMMAND) ||
    resolve(state.resultPath) !== resolve(BROWSER_RESULT)
  ) {
    fail("invalid browser harness state");
  }
  const id = `${process.pid}-${Date.now()}`;
  writeJsonAtomic(BROWSER_COMMAND, { id, action });
  const result = await waitUntil(
    `browser harness command ${action}`,
    () => {
      if (!existsSync(BROWSER_RESULT)) return null;
      const candidate = JSON.parse(readFileSync(BROWSER_RESULT, "utf8"));
      return candidate.id === id ? candidate : null;
    },
    START_TIMEOUT_MS,
  );
  process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
}

const arguments_ = process.argv.slice(2);
if (arguments_.length === 0) {
  await automatedCheck();
} else if (arguments_.length === 1 && arguments_[0] === "--record-evidence") {
  await automatedCheck(true);
} else if (arguments_.length === 1 && arguments_[0] === "--serve-browser") {
  await serveBrowser();
} else if (arguments_.length === 2 && arguments_[0] === "--browser-command") {
  await browserCommand(arguments_[1]);
} else {
  fail(
    "usage: node scripts/check-pliegors-dev-loop.mjs [--record-evidence|--serve-browser|--browser-command valid|invalid|restore|equivalent|stop]",
  );
}
