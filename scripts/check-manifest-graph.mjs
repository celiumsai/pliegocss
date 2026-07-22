import { createHash } from "node:crypto";
import {
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { spawn, spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { cargoTargetRoot, isolatedCargoEnvironment } from "./rust-target.mjs";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const fixture = join(root, "integration-tests", "manifest-graph");
const runtime = join(root, "target", "manifest-graph", `run-${process.pid}-${Date.now()}`);
const project = join(runtime, "project");
const cargoEnvironment = isolatedCargoEnvironment(root, {
  env: process.env,
  toolchain: "1.85.0",
});
const targetDir = cargoTargetRoot(root, cargoEnvironment);
const expectedCss = readFileSync(join(fixture, "expected.css"));
const expectedManifest = readFileSync(join(fixture, "expected.manifest.json"));
const reachabilityName = "pliego.reachability.json";
const updateGoldens = process.env.PLIEGOCSS_UPDATE_GOLDENS === "1";
const cargoBuildTimeout = Number.parseInt(
  process.env.PLIEGOCSS_CARGO_BUILD_TIMEOUT_MS ?? "600000",
  10,
);

function fail(message) {
  throw new Error(message);
}

function assert(condition, message) {
  if (!condition) fail(message);
}

function assertEqual(actual, expected, message) {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    fail(`${message}\nactual: ${JSON.stringify(actual)}\nexpected: ${JSON.stringify(expected)}`);
  }
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function run(program, args, { cwd = root, expectedStatus = 0, timeout = 30_000 } = {}) {
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

  run("cargo", ["+1.85.0", "build", "--locked", "-p", "pliego-cssc"], {
    timeout: cargoBuildTimeout,
  });
  const executable = join(
    targetDir,
    "debug",
    process.platform === "win32" ? "pliego-cssc.exe" : "pliego-cssc",
  );
  assert(existsSync(executable), `Rust 1.85 build did not produce ${executable}`);
  return executable;
}

function stderrLines(value) {
  return value
    .split(/\r?\n/u)
    .map((line) => line.trim())
    .filter(Boolean);
}

function isExpectedTransientWatchFailure(line) {
  return /^compile failed; keeping the last valid artifact: (?:EOF while parsing a value at line 1 column 0|cannot read reachability sidecar `.+`: The process cannot access the file because it is being used by another process\. \(os error 32\))$/u.test(
    line,
  );
}

function runCli(executable, args, expectedStatus = 0) {
  const result = run(executable, args, { cwd: project, expectedStatus });
  if (expectedStatus === 0) {
    assert(result.stderr === "", `successful CLI command emitted stderr:\n${result.stderr}`);
  }
  return result;
}

function compileArguments({ css, manifest, version, reachability, source = "src/card.rs" }) {
  return [
    "compile",
    "--source",
    source,
    "--seed",
    "--targets",
    "modern",
    "--format",
    "minified",
    "--output",
    css,
    ...(manifest ? ["--manifest", manifest] : []),
    ...(version ? ["--manifest-version", String(version)] : []),
    ...(reachability ? ["--reachability", reachability] : []),
  ];
}

function bundleArguments(outputDir, reachability = reachabilityName, check = false) {
  return [
    "bundle",
    "--plan",
    "pliego.bundles.toml",
    "--output-dir",
    outputDir,
    "--manifest-version",
    "4",
    "--reachability",
    reachability,
    ...(check ? ["--check"] : []),
  ];
}

function directorySnapshot(directory) {
  return readdirSync(directory)
    .sort()
    .map((name) => {
      const path = join(directory, name);
      const metadata = statSync(path);
      return [name, metadata.size, metadata.mtimeMs, metadata.ino, readFileSync(path).toString("base64")];
    });
}

function fileStatContract(path) {
  const metadata = statSync(path);
  return {
    size: metadata.size,
    mtimeMs: metadata.mtimeMs,
    ctimeMs: metadata.ctimeMs,
    ino: metadata.ino,
  };
}

function delay(milliseconds) {
  return new Promise((resolveDelay) => setTimeout(resolveDelay, milliseconds));
}

function childExited(child) {
  return child.exitCode !== null || child.signalCode !== null;
}

function waitForChildExit(child, timeout) {
  if (childExited(child)) return Promise.resolve(true);
  return new Promise((resolveExit) => {
    const done = () => {
      clearTimeout(timer);
      child.removeListener("exit", done);
      resolveExit(true);
    };
    const timer = setTimeout(() => {
      child.removeListener("exit", done);
      resolveExit(childExited(child));
    }, timeout);
    child.once("exit", done);
  });
}

async function terminateChild(child) {
  if (childExited(child)) return;
  child.kill("SIGTERM");
  if (await waitForChildExit(child, 2_000)) return;

  if (process.platform === "win32") {
    spawnSync("taskkill", ["/PID", String(child.pid), "/T", "/F"], {
      encoding: "utf8",
      timeout: 5_000,
      windowsHide: true,
    });
  } else {
    child.kill("SIGKILL");
  }
  assert(await waitForChildExit(child, 5_000), `watch process ${child.pid} did not terminate`);
}

async function waitForWatch(state, description, predicate, timeout = 15_000) {
  const started = Date.now();
  let lastError;
  while (Date.now() - started < timeout) {
    if (state.spawnError) {
      fail(`cannot start watch process: ${state.spawnError.message}`);
    }
    if (childExited(state.child)) {
      fail(
        `watch process exited before ${description} (code ${state.child.exitCode}, signal ${state.child.signalCode})\n` +
          `stdout:\n${state.stdout}\nstderr:\n${state.stderr}`,
      );
    }
    try {
      if (predicate()) return;
    } catch (error) {
      lastError = error;
    }
    await delay(25);
  }
  fail(
    `timed out waiting for ${description}${lastError ? `: ${lastError.message}` : ""}\n` +
      `stdout:\n${state.stdout}\nstderr:\n${state.stderr}`,
  );
}

async function assertWatchContract(executable) {
  const watchDirectory = join(project, "watch-v4");
  const watchCss = join(watchDirectory, "card.css");
  const watchManifest = join(watchDirectory, "card.manifest.json");
  const watchReachabilityName = "watch.reachability.json";
  const watchReachability = join(project, watchReachabilityName);
  mkdirSync(watchDirectory);
  writeFileSync(watchReachability, readFileSync(join(project, reachabilityName)));

  const child = spawn(
    executable,
    [
      "watch",
      "--source",
      "src/card.rs",
      "--seed",
      "--targets",
      "modern",
      "--format",
      "minified",
      "--output",
      "watch-v4/card.css",
      "--manifest",
      "watch-v4/card.manifest.json",
      "--manifest-version",
      "4",
      "--reachability",
      watchReachabilityName,
    ],
    {
      cwd: project,
      env: cargoEnvironment,
      stdio: ["ignore", "pipe", "pipe"],
      windowsHide: true,
    },
  );
  const state = { child, spawnError: undefined, stdout: "", stderr: "" };
  const appendLog = (field, chunk) => {
    state[field] = `${state[field]}${chunk.toString("utf8")}`.slice(-65_536);
  };
  child.on("error", (error) => {
    state.spawnError = error;
  });
  child.stdout.on("data", (chunk) => appendLog("stdout", chunk));
  child.stderr.on("data", (chunk) => appendLog("stderr", chunk));

  try {
    await waitForWatch(state, "initial schema-4 CSS and manifest", () => {
      if (!existsSync(watchCss) || !existsSync(watchManifest)) return false;
      const manifest = JSON.parse(readFileSync(watchManifest, "utf8"));
      return manifest.graph?.routes?.some(
        (route) => route.id === "route:home" && route.path === "/card",
      );
    });

    const initialCss = readFileSync(watchCss);
    const initialCssStat = fileStatContract(watchCss);
    const initialManifestBytes = readFileSync(watchManifest);
    const initialManifest = JSON.parse(initialManifestBytes);
    assertEqual(initialCss, expectedCss, "watch initial CSS drifted from the frozen golden");
    assertEqual(
      initialManifestBytes,
      expectedManifest,
      "watch initial manifest drifted from the frozen golden",
    );

    const originalSidecar = readFileSync(watchReachability, "utf8");
    const routeField = '"path": "/card"';
    assert(
      originalSidecar.split(routeField).length === 2,
      "watch sidecar must contain exactly one mutable route path",
    );
    writeFileSync(watchReachability, originalSidecar.replace(routeField, '"path": "/renamed-card"'));

    await waitForWatch(state, "route-only manifest rebuild", () => {
      const bytes = readFileSync(watchManifest);
      if (bytes.equals(initialManifestBytes)) return false;
      const manifest = JSON.parse(bytes);
      return manifest.graph.routes.some(
        (route) => route.id === "route:home" && route.path === "/renamed-card",
      );
    });

    await delay(350);
    const changedManifestBytes = readFileSync(watchManifest);
    const changedManifest = JSON.parse(changedManifestBytes);
    assert(!changedManifestBytes.equals(initialManifestBytes), "watch did not republish the manifest");
    assertEqual(readFileSync(watchCss), initialCss, "route-only watch rebuild changed CSS bytes");
    assertEqual(
      fileStatContract(watchCss),
      initialCssStat,
      "route-only watch rebuild touched CSS metadata or mtime",
    );

    const renamedRoute = changedManifest.graph.routes.find((route) => route.id === "route:home");
    assert(renamedRoute?.path === "/renamed-card", "watch manifest lost the new route path");
    renamedRoute.path = "/card";
    assertEqual(
      changedManifest,
      initialManifest,
      "route-only watch rebuild changed data outside the requested route path",
    );
    const unexpectedFailures = stderrLines(state.stderr)
      .filter((line) => line.startsWith("compile failed;"))
      .filter((line) => !isExpectedTransientWatchFailure(line));
    assert(
      unexpectedFailures.length === 0,
      `watch reported an unexpected compile failure:\n${unexpectedFailures.join("\n")}\n\nfull stderr:\n${state.stderr}`,
    );
  } finally {
    try {
      await terminateChild(child);
    } finally {
      rmSync(watchDirectory, { recursive: true, force: true });
      rmSync(watchReachability, { force: true });
    }
  }
}

function sorted(values) {
  return [...values].sort((left, right) =>
    left < right ? -1 : left > right ? 1 : 0,
  );
}

function assertSortedUnique(values, description) {
  assertEqual(values, sorted(values), `${description} is not canonically sorted`);
  assert(new Set(values).size === values.length, `${description} contains duplicates`);
}

function assertManifestIntegrity(manifest, css, schemaVersion) {
  const expectedFields = [
    "schemaVersion",
    "styleIdFormatVersion",
    "classNameFormatVersion",
    "themeIdFormatVersion",
    "themeId",
    "targets",
    "format",
    "cssSha256",
    "cssBytes",
    "styles",
    ...(schemaVersion === 4 ? ["graph"] : []),
  ];
  assertEqual(Object.keys(manifest), expectedFields, `manifest schema ${schemaVersion} surface drifted`);
  assert(manifest.schemaVersion === schemaVersion, `expected manifest schema ${schemaVersion}`);
  assert(manifest.styleIdFormatVersion === 2, "StyleId format version drifted");
  assert(manifest.classNameFormatVersion === 1, "class-name format version drifted");
  assert(manifest.themeIdFormatVersion === 3, "theme ID format version drifted");
  assert(manifest.targets === "modern", "target contract drifted");
  assert(manifest.format === "minified", "format contract drifted");
  assert(manifest.cssBytes === css.byteLength, "manifest CSS byte length is stale");
  assert(manifest.cssSha256 === sha256(css), "manifest CSS digest is stale");
  assert(manifest.styles.length === 1, "fixture must produce exactly one semantic style");

  const style = manifest.styles[0];
  assert(/^[0-9a-f]{32}$/.test(style.styleId), "manifest StyleId is not fixed-width hex");
  assert(/^pc_[a-z0-9]+$/.test(style.className), "manifest class name is not canonical");
  assert(
    css.toString("utf8").includes(`.${style.className}{`),
    "manifest class identity is absent from emitted CSS",
  );
  assert(style.origins.length === 1, "fixture must preserve one exact source origin");
  const origin = style.origins[0];
  assertEqual(
    [origin.file, origin.byteStart, origin.byteEnd, origin.macroKind, origin.reason],
    ["src/card.rs", 24, 44, "pc", "visible-literal"],
    "source provenance drifted",
  );
  const source = readFileSync(join(project, origin.file));
  assert(
    source.subarray(origin.byteStart, origin.byteEnd).toString("utf8") ===
      'pc!("p-4 bg-accent")',
    "manifest origin no longer selects the exact macro bytes",
  );
}

function edgeKey(edge) {
  return `${edge.kind}\0${edge.from}\0${edge.to}`;
}

function assertGraphContract(manifest) {
  const graph = manifest.graph;
  assert(graph && typeof graph === "object", "schema 4 manifest has no graph");
  assert(graph.schemaVersion === 1, "manifest graph schema drifted");
  assert(graph.declarationIdFormatVersion === 1, "declaration ID format drifted");
  assert(
    graph.originCoverage === "compiler-verified-complete",
    "manifest graph stopped reporting compiler-verified origin coverage",
  );
  assert(
    graph.applicationCoverage === "adapter-attested-complete",
    "manifest graph stopped preserving the adapter coverage attestation",
  );
  assertEqual(
    Object.keys(graph),
    [
      "schemaVersion",
      "declarationIdFormatVersion",
      "originCoverage",
      "applicationCoverage",
      "declarations",
      "tokens",
      "components",
      "routes",
      "islands",
      "edges",
    ],
    "manifest graph field order or surface drifted",
  );

  for (const [description, nodes] of [
    ["declarations", graph.declarations],
    ["tokens", graph.tokens],
    ["components", graph.components],
    ["routes", graph.routes],
    ["islands", graph.islands],
  ]) {
    assertSortedUnique(
      nodes.map((node) => node.id),
      `graph ${description}`,
    );
  }
  assertSortedUnique(graph.edges.map(edgeKey), "graph edges");

  const styleNodes = manifest.styles.map((style) => `style:${style.styleId}`);
  const graphNodes = [
    ...styleNodes,
    ...graph.declarations.map((node) => node.id),
    ...graph.tokens.map((node) => node.id),
    ...graph.components.map((node) => node.id),
    ...graph.routes.map((node) => node.id),
    ...graph.islands.map((node) => node.id),
  ];
  assert(new Set(graphNodes).size === graphNodes.length, "graph node IDs are not globally unique");
  const nodeIds = new Set(graphNodes);
  for (const edge of graph.edges) {
    assert(nodeIds.has(edge.from), `edge source does not exist: ${edge.from}`);
    assert(nodeIds.has(edge.to), `edge destination does not exist: ${edge.to}`);
  }

  const styleId = manifest.styles[0].styleId;
  for (const declaration of graph.declarations) {
    const match = declaration.id.match(
      new RegExp(`^decl:${styleId}:([0-9a-f]{8})$`),
    );
    assert(match, `invalid declaration ID: ${declaration.id}`);
    assert(declaration.styleId === styleId, "declaration references the wrong semantic style");
    assert(Number.parseInt(match[1], 16) === declaration.ordinal, "declaration ordinal drifted");
  }
  assertEqual(
    graph.declarations.map((declaration) => declaration.ordinal),
    [0, 1],
    "fixture semantic declaration count/order drifted",
  );

  for (const token of graph.tokens) {
    assert(
      token.id === `token:${token.kind}:${token.tokenId}` && /^[0-9a-f]{8}$/.test(token.tokenId),
      `invalid token node identity: ${token.id}`,
    );
  }
  assertEqual(
    graph.tokens.map(({ kind, name }) => [kind, name]),
    [
      ["color", "accent"],
      ["spacing", "4"],
    ],
    "fixture token projection drifted",
  );

  const edges = new Set(graph.edges.map(edgeKey));
  const hasEdge = (kind, from, to) => edges.has(edgeKey({ kind, from, to }));
  assert(
    hasEdge("routeUsesComponent", "route:home", "component:card"),
    "route no longer reaches the card component",
  );
  assert(
    hasEdge("islandUsesComponent", "island:card-preview", "component:card"),
    "island no longer reaches the card component",
  );

  const reachedTokenKinds = new Set();
  for (const declaration of graph.declarations) {
    assert(
      hasEdge("styleHasDeclaration", `style:${styleId}`, declaration.id),
      `style does not own ${declaration.id}`,
    );
    assert(
      hasEdge("componentUsesDeclaration", "component:card", declaration.id),
      `card component does not own ${declaration.id}`,
    );
    const tokenEdges = graph.edges.filter(
      (edge) => edge.kind === "declarationUsesToken" && edge.from === declaration.id,
    );
    assert(tokenEdges.length === 1, `${declaration.id} must reference exactly one token`);
    reachedTokenKinds.add(
      graph.tokens.find((token) => token.id === tokenEdges[0].to)?.kind,
    );
  }
  assertEqual(sorted(reachedTokenKinds), ["color", "spacing"], "typed token path is incomplete");
}

function writeJson(path, value) {
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`);
}

function expectFailureWithoutMutation(executable, args, expectedDiagnostic, guardDirectory) {
  const before = directorySnapshot(guardDirectory);
  const result = runCli(executable, args, 1);
  assert(
    expectedDiagnostic.test(result.stderr),
    `failure lost diagnostic ${expectedDiagnostic}:\n${result.stderr}`,
  );
  assertEqual(
    directorySnapshot(guardDirectory),
    before,
    `failed command mutated guarded outputs: ${args.join(" ")}`,
  );
}

process.once("exit", () => rmSync(runtime, { recursive: true, force: true }));

rmSync(runtime, { recursive: true, force: true });
mkdirSync(runtime, { recursive: true });
cpSync(fixture, project, { recursive: true });
const executable = resolveExecutable();

mkdirSync(join(project, "out"));
const schemaThreeArgs = compileArguments({
  css: "out/schema-3.css",
  manifest: "out/schema-3.manifest.json",
  version: 3,
});
const schemaFourArgs = compileArguments({
  css: "out/schema-4.css",
  manifest: "out/schema-4.manifest.json",
  version: 4,
  reachability: reachabilityName,
});
runCli(executable, schemaThreeArgs);
runCli(executable, schemaFourArgs);

const schemaThreeCss = readFileSync(join(project, "out", "schema-3.css"));
const schemaFourCss = readFileSync(join(project, "out", "schema-4.css"));
const schemaThreeManifest = JSON.parse(
  readFileSync(join(project, "out", "schema-3.manifest.json"), "utf8"),
);
const schemaFourManifestBytes = readFileSync(join(project, "out", "schema-4.manifest.json"));
const schemaFourManifest = JSON.parse(schemaFourManifestBytes);

if (updateGoldens) {
  writeFileSync(join(fixture, "expected.css"), schemaFourCss);
  writeFileSync(join(fixture, "expected.manifest.json"), schemaFourManifestBytes);
  process.stdout.write("manifest graph goldens updated\n");
  process.exit(0);
}

assertEqual(schemaThreeCss, schemaFourCss, "manifest version changed emitted CSS");
assertEqual(schemaFourCss, expectedCss, "frozen CSS golden drifted");
assertEqual(schemaFourManifestBytes, expectedManifest, "frozen schema-4 manifest golden drifted");
assertManifestIntegrity(schemaThreeManifest, schemaThreeCss, 3);
assertManifestIntegrity(schemaFourManifest, schemaFourCss, 4);
assert(schemaThreeManifest.graph === undefined, "schema 3 unexpectedly contains graph data");
assertEqual(
  schemaThreeManifest.styles,
  schemaFourManifest.styles,
  "schema 4 changed stable style identity/provenance",
);
assert(
  schemaThreeManifest.themeId === schemaFourManifest.themeId,
  "manifest version changed the active theme identity",
);
assertGraphContract(schemaFourManifest);

const reachability = JSON.parse(readFileSync(join(project, reachabilityName), "utf8"));
const reordered = structuredClone(reachability);
reordered.components.reverse();
reordered.routes.reverse();
reordered.islands.reverse();
for (const component of reordered.components) component.sites.reverse();
for (const route of reordered.routes) route.components.reverse();
for (const island of reordered.islands) island.components.reverse();
writeJson(join(project, "reordered.reachability.json"), reordered);
runCli(
  executable,
  compileArguments({
    css: "out/reordered.css",
    manifest: "out/reordered.manifest.json",
    version: 4,
    reachability: "reordered.reachability.json",
  }),
);
assertEqual(
  readFileSync(join(project, "out", "reordered.css")),
  schemaFourCss,
  "reordered sidecar changed CSS bytes",
);
assertEqual(
  readFileSync(join(project, "out", "reordered.manifest.json")),
  schemaFourManifestBytes,
  "reordered sidecar changed manifest bytes",
);

mkdirSync(join(project, "guard"));
writeFileSync(join(project, "guard", "protected.css"), "protected css\n");
writeFileSync(join(project, "guard", "protected.manifest.json"), "protected manifest\n");
const guardedCompile = (reachabilityFile, version = 4, includeManifest = true) =>
  compileArguments({
    css: "guard/protected.css",
    manifest: includeManifest ? "guard/protected.manifest.json" : undefined,
    version,
    reachability: reachabilityFile,
  });

const invalidOwnership = structuredClone(reachability);
invalidOwnership.components.find((component) => component.id === "card").sites[0].byteStart = 25;
writeJson(join(project, "invalid-ownership.json"), invalidOwnership);
expectFailureWithoutMutation(
  executable,
  guardedCompile("invalid-ownership.json"),
  /has no component(?: owner)?/,
  join(project, "guard"),
);

const dangling = structuredClone(reachability);
dangling.routes[0].components.push("missing");
writeJson(join(project, "dangling.json"), dangling);
expectFailureWithoutMutation(
  executable,
  guardedCompile("dangling.json"),
  /unknown component|component reference/,
  join(project, "guard"),
);

const unknownField = structuredClone(reachability);
unknownField.components[0].unknownField = true;
writeJson(join(project, "unknown-field.json"), unknownField);
expectFailureWithoutMutation(
  executable,
  guardedCompile("unknown-field.json"),
  /unknown field `unknownField`/,
  join(project, "guard"),
);

for (const [name, file] of [
  ["reserved-character", "src/card?.rs"],
  ["reserved-device", "src/CON.rs"],
  ["reserved-port-device", "src/COM¹.rs"],
  ["trailing-dot", "src/card."],
]) {
  const invalidPath = structuredClone(reachability);
  invalidPath.components[0].sites[0].file = file;
  writeJson(join(project, `${name}.json`), invalidPath);
  expectFailureWithoutMutation(
    executable,
    guardedCompile(`${name}.json`),
    /invalid reachability document/,
    join(project, "guard"),
  );
}

const partialCoverage = structuredClone(reachability);
partialCoverage.applicationCoverage = "partial";
writeJson(join(project, "partial-coverage.json"), partialCoverage);
expectFailureWithoutMutation(
  executable,
  guardedCompile("partial-coverage.json"),
  /invalid reachability document/,
  join(project, "guard"),
);

const missingCoverage = structuredClone(reachability);
delete missingCoverage.applicationCoverage;
writeJson(join(project, "missing-coverage.json"), missingCoverage);
expectFailureWithoutMutation(
  executable,
  guardedCompile("missing-coverage.json"),
  /missing field `applicationCoverage`/,
  join(project, "guard"),
);

const cardSourcePath = join(project, "src", "card.rs");
const cardSourceBefore = readFileSync(cardSourcePath);
expectFailureWithoutMutation(
  executable,
  compileArguments({
    css: "src/card.rs",
    manifest: "guard/protected.manifest.json",
    version: 4,
    reachability: reachabilityName,
    source: "src",
  }),
  /Rust source extension|aliases input `input`/,
  join(project, "guard"),
);
assertEqual(readFileSync(cardSourcePath), cardSourceBefore, "compile overwrote an expanded source");
const watchAlias = runCli(
  executable,
  ["watch", "--source", "src", "--seed", "--output", "src/card.rs"],
  1,
);
assert(
  /Rust source extension|aliases input `input`/.test(watchAlias.stderr),
  `watch source/output alias lost its diagnostic:\n${watchAlias.stderr}`,
);
assertEqual(readFileSync(cardSourcePath), cardSourceBefore, "watch overwrote an expanded source");

const symlinkTarget = join(project, "symlink-target-directory");
const symlinkOutput = join(project, "symlink-output.css");
mkdirSync(symlinkTarget);
symlinkSync(symlinkTarget, symlinkOutput, process.platform === "win32" ? "junction" : "dir");
expectFailureWithoutMutation(
  executable,
  compileArguments({
    css: "symlink-output.css",
    manifest: "guard/protected.manifest.json",
    version: 4,
    reachability: reachabilityName,
  }),
  /symbolic link|reparse point/,
  join(project, "guard"),
);
assertEqual(readdirSync(symlinkTarget), [], "reparse output target was mutated");

for (const manifest of [
  "guard/.namespace.css.pliego.lock",
  `guard/.namespace.css.pliego-${process.pid}-0.tmp`,
  `guard/.namespace.css.pliego-${process.pid}-0.bak`,
]) {
  expectFailureWithoutMutation(
    executable,
    compileArguments({
      css: "guard/namespace.css",
      manifest,
      version: 4,
      reachability: reachabilityName,
    }),
    /reserved coordination namespace/,
    join(project, "guard"),
  );
}
expectFailureWithoutMutation(
  executable,
  compileArguments({ css: "guard/.orphan.css.pliego.lock" }),
  /reserved coordination namespace/,
  join(project, "guard"),
);

let invalidUtf8PathTested = false;
if (process.platform !== "win32") {
  const invalidUtf8Name = Buffer.concat([
    Buffer.from("non-utf8-"),
    Buffer.from([0xff]),
    Buffer.from(".rs"),
  ]);
  const invalidUtf8Source = Buffer.concat([
    Buffer.from(`${join(project, "src")}/`),
    invalidUtf8Name,
  ]);
  writeFileSync(invalidUtf8Source, 'fn invalid() { let _ = pc!("flex"); }\n');
  const preservesInvalidUtf8 = readdirSync(join(project, "src"), {
    encoding: "buffer",
  }).some((entry) => entry.equals(invalidUtf8Name));
  if (preservesInvalidUtf8) {
    invalidUtf8PathTested = true;
    expectFailureWithoutMutation(
      executable,
      compileArguments({
        css: "guard/protected.css",
        manifest: "guard/protected.manifest.json",
        version: 4,
        reachability: reachabilityName,
        source: "src",
      }),
      /not valid UTF-8/,
      join(project, "guard"),
    );
  }
  rmSync(invalidUtf8Source);
}

const oversized = join(project, "oversized.reachability.json");
writeFileSync(oversized, Buffer.alloc(16 * 1024 * 1024 + 1, 0x20));
expectFailureWithoutMutation(
  executable,
  guardedCompile("oversized.reachability.json"),
  /regular file of at most 16777216 bytes|exceeds 16777216 bytes|not a bounded regular file/,
  join(project, "guard"),
);

if (process.platform !== "win32") {
  const sourceName = "src\\slash-collision.rs";
  const sourceText = 'fn view() { let _ = pc!("flex"); }\n';
  const macroText = 'pc!("flex")';
  const byteStart = Buffer.byteLength(sourceText.slice(0, sourceText.indexOf(macroText)));
  writeFileSync(join(project, sourceName), sourceText);
  writeJson(join(project, "slash-collision.json"), {
    schema: 1,
    applicationCoverage: "complete",
    components: [
      {
        id: "collision",
        sites: [
          {
            file: "src/slash-collision.rs",
            byteStart,
            byteEnd: byteStart + Buffer.byteLength(macroText),
          },
        ],
      },
    ],
    routes: [],
    islands: [],
  });
  expectFailureWithoutMutation(
    executable,
    compileArguments({
      css: "guard/protected.css",
      manifest: "guard/protected.manifest.json",
      version: 4,
      reachability: "slash-collision.json",
      source: sourceName,
    }),
    /origin path is not portable/,
    join(project, "guard"),
  );
}

const graphComponents = Array.from({ length: 65_520 }, (_, index) => ({
  id: index === 0 ? "card" : `component-${index.toString(16).padStart(4, "0")}`,
  sites: index === 0 ? reachability.components[0].sites : [],
}));
const graphComponentIds = graphComponents.map((component) => component.id);
writeJson(join(project, "edge-limit.json"), {
  schema: 1,
  applicationCoverage: "complete",
  components: graphComponents,
  routes: [{ id: "all", path: "/all", components: graphComponentIds }],
  islands: [
    { id: "shared", name: "Shared", components: graphComponentIds.slice(0, 15) },
  ],
});
expectFailureWithoutMutation(
  executable,
  guardedCompile("edge-limit.json"),
  /manifest graph edge limit exceeded/,
  join(project, "guard"),
);

writeJson(join(project, "node-limit.json"), {
  schema: 1,
  applicationCoverage: "complete",
  components: [
    ...graphComponents,
    ...Array.from({ length: 12 }, (_, index) => ({
      id: `node-limit-${index}`,
      sites: [],
    })),
  ],
  routes: [{ id: "empty", path: "/empty", components: [] }],
  islands: [{ id: "empty", name: "Empty", components: [] }],
});
expectFailureWithoutMutation(
  executable,
  guardedCompile("node-limit.json"),
  /manifest graph node limit exceeded/,
  join(project, "guard"),
);

expectFailureWithoutMutation(
  executable,
  guardedCompile(undefined),
  /manifest version 4 requires `--reachability`/,
  join(project, "guard"),
);
expectFailureWithoutMutation(
  executable,
  guardedCompile(reachabilityName, null),
  /`--reachability` requires `--manifest-version 4`/,
  join(project, "guard"),
);
expectFailureWithoutMutation(
  executable,
  guardedCompile(reachabilityName, 3),
  /`--reachability` requires `--manifest-version 4`/,
  join(project, "guard"),
);
expectFailureWithoutMutation(
  executable,
  guardedCompile(reachabilityName, 4, false),
  /`--manifest-version` requires manifest output/,
  join(project, "guard"),
);
const checkGraphOption = runCli(
  executable,
  ["check", "--style", "flex", "--manifest-version", "3"],
  1,
);
assert(
  /`check` does not accept output, manifest, theme, reachability, or pruning options/.test(
    checkGraphOption.stderr,
  ),
  `check graph-option diagnostic drifted:\n${checkGraphOption.stderr}`,
);

mkdirSync(join(project, "bundle-v4"));
runCli(executable, bundleArguments("bundle-v4"));
const bundleCss = readFileSync(join(project, "bundle-v4", "card.css"));
const bundleManifest = readFileSync(join(project, "bundle-v4", "card.manifest.json"));
assertEqual(bundleCss, expectedCss, "bundle schema 4 CSS drifted from compile output");
assertEqual(bundleManifest, expectedManifest, "bundle schema 4 manifest drifted from golden");
const beforeBundleCheck = directorySnapshot(join(project, "bundle-v4"));
runCli(executable, bundleArguments("bundle-v4", reachabilityName, true));
assertEqual(
  directorySnapshot(join(project, "bundle-v4")),
  beforeBundleCheck,
  "successful graph bundle --check mutated artifacts",
);

mkdirSync(join(project, "bundle-reordered"));
runCli(executable, bundleArguments("bundle-reordered", "reordered.reachability.json"));
assertEqual(
  readFileSync(join(project, "bundle-reordered", "card.css")),
  bundleCss,
  "reordered bundle sidecar changed CSS bytes",
);
assertEqual(
  readFileSync(join(project, "bundle-reordered", "card.manifest.json")),
  bundleManifest,
  "reordered bundle sidecar changed manifest bytes",
);

await assertWatchContract(executable);

console.log(
  `manifest graph gate passed: CSS ${sha256(expectedCss)}, manifest ${sha256(expectedManifest)}, ` +
    `invalid UTF-8 path ${invalidUtf8PathTested ? "tested" : "not representable on filesystem"}`,
);
