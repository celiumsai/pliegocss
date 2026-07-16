import { spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  cpSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const fixture = join(root, "integration-tests", "physical-trace");
const runtime = join(root, "target", "physical-trace", `run-${process.pid}-${Date.now()}`);
const project = join(runtime, "project");
const targetDir = join(root, "target");
const reachabilityName = "pliego.reachability.json";
const expectedCss = readFileSync(join(fixture, "expected.css"));
const expectedManifest = readFileSync(join(fixture, "expected.manifest.json"));

const physicalEdgeKinds = new Set([
  "declarationContributesToPhysicalDeclaration",
  "physicalDeclarationBelongsToRule",
  "ruleNestedInRule",
  "syntheticProducerProducesPhysicalDeclaration",
]);

function fail(message) {
  throw new Error(message);
}

function assert(condition, message) {
  if (!condition) fail(message);
}

function render(value) {
  const serialized = JSON.stringify(value);
  return serialized && serialized.length > 2_000
    ? `${serialized.slice(0, 2_000)}...`
    : serialized;
}

function assertEqual(actual, expected, message) {
  if (Buffer.isBuffer(actual) && Buffer.isBuffer(expected)) {
    if (!actual.equals(expected)) {
      fail(
        `${message}\nactual sha256: ${sha256(actual)}\nexpected sha256: ${sha256(expected)}`,
      );
    }
    return;
  }
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    fail(`${message}\nactual: ${render(actual)}\nexpected: ${render(expected)}`);
  }
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function sorted(values) {
  return [...values].sort((left, right) => (left < right ? -1 : left > right ? 1 : 0));
}

function assertSortedUnique(values, description) {
  assertEqual(values, sorted(values), `${description} is not canonically sorted`);
  assert(new Set(values).size === values.length, `${description} contains duplicates`);
}

function edgeKey(edge) {
  return `${edge.kind}\0${edge.from}\0${edge.to}`;
}

function run(program, args, { cwd = root, expectedStatus = 0, timeout = 30_000 } = {}) {
  const result = spawnSync(program, args, {
    cwd,
    encoding: "utf8",
    env: { ...process.env, CARGO_TARGET_DIR: targetDir },
    maxBuffer: 16 * 1024 * 1024,
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
    timeout: 180_000,
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
  const result = run(executable, args, { cwd: project, expectedStatus });
  if (expectedStatus === 0) {
    assert(result.stderr === "", `successful CLI command emitted stderr:\n${result.stderr}`);
  }
  return result;
}

function compileArguments({
  css,
  manifest,
  version,
  reachability,
  targets = "modern",
  format = "minified",
  theme = true,
  source = "src/card.rs",
}) {
  return [
    "compile",
    "--source",
    source,
    "--seed",
    ...(theme ? ["--theme"] : []),
    "--targets",
    targets,
    "--format",
    format,
    "--output",
    css,
    ...(manifest ? ["--manifest", manifest] : []),
    ...(version ? ["--manifest-version", String(version)] : []),
    ...(reachability ? ["--reachability", reachability] : []),
  ];
}

function bundleArguments(outputDir, { reachability, check = false } = {}) {
  return [
    "bundle",
    "--plan",
    "pliego.bundles.toml",
    "--output-dir",
    outputDir,
    "--manifest-version",
    "5",
    ...(reachability ? ["--reachability", reachability] : []),
    ...(check ? ["--check"] : []),
  ];
}

function walkFiles(directory, base = directory) {
  const output = [];
  for (const name of readdirSync(directory).sort()) {
    const path = join(directory, name);
    const metadata = statSync(path);
    if (metadata.isDirectory()) {
      output.push(...walkFiles(path, base));
    } else {
      output.push([relative(base, path).replaceAll("\\", "/"), path, metadata]);
    }
  }
  return output;
}

function directorySnapshot(directory) {
  return walkFiles(directory).map(([name, path, metadata]) => [
    name,
    metadata.size,
    metadata.mtimeMs,
    metadata.ctimeMs,
    metadata.ino,
    sha256(readFileSync(path)),
  ]);
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
    if (state.spawnError) fail(`cannot start watch process: ${state.spawnError.message}`);
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
  const directory = join(project, "watch-v5");
  const cssPath = join(directory, "card.css");
  const manifestPath = join(directory, "card.manifest.json");
  const sidecarName = "watch.reachability.json";
  const sidecarPath = join(project, sidecarName);
  mkdirSync(directory);
  writeFileSync(sidecarPath, readFileSync(join(project, reachabilityName)));

  const child = spawn(
    executable,
    [
      "watch",
      "--source",
      "src/card.rs",
      "--seed",
      "--theme",
      "--targets",
      "modern",
      "--format",
      "minified",
      "--output",
      "watch-v5/card.css",
      "--manifest",
      "watch-v5/card.manifest.json",
      "--manifest-version",
      "5",
      "--reachability",
      sidecarName,
    ],
    {
      cwd: project,
      env: { ...process.env, CARGO_TARGET_DIR: targetDir },
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
    await waitForWatch(state, "initial schema-5 CSS and manifest", () => {
      if (!existsSync(cssPath) || !existsSync(manifestPath)) return false;
      const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
      return (
        manifest.schemaVersion === 5 &&
        manifest.graph?.physicalCoverage === "compiler-verified-complete" &&
        manifest.graph.routes.some((route) => route.id === "route:home" && route.path === "/")
      );
    });

    const initialCss = readFileSync(cssPath);
    const initialCssStat = fileStatContract(cssPath);
    const initialManifestBytes = readFileSync(manifestPath);
    const initialManifest = JSON.parse(initialManifestBytes);
    assertEqual(initialCss, expectedCss, "watch schema-5 CSS drifted from the golden");
    assertEqual(initialManifestBytes, expectedManifest, "watch schema-5 manifest drifted from the golden");

    const sidecar = readFileSync(sidecarPath, "utf8");
    const routeField = '"path": "/"';
    assert(sidecar.split(routeField).length === 2, "watch sidecar route path is not unique");
    writeFileSync(sidecarPath, sidecar.replace(routeField, '"path": "/renamed"'));

    await waitForWatch(state, "schema-5 route-only manifest rebuild", () => {
      const bytes = readFileSync(manifestPath);
      if (bytes.equals(initialManifestBytes)) return false;
      const manifest = JSON.parse(bytes);
      return manifest.graph.routes.some(
        (route) => route.id === "route:home" && route.path === "/renamed",
      );
    });
    await delay(350);

    const changedManifestBytes = readFileSync(manifestPath);
    const changedManifest = JSON.parse(changedManifestBytes);
    assert(!changedManifestBytes.equals(initialManifestBytes), "watch did not republish schema 5");
    assertEqual(readFileSync(cssPath), initialCss, "route-only schema-5 watch changed CSS bytes");
    assertEqual(
      fileStatContract(cssPath),
      initialCssStat,
      "route-only schema-5 watch touched CSS metadata or mtime",
    );
    const route = changedManifest.graph.routes.find((item) => item.id === "route:home");
    assert(route?.path === "/renamed", "watch schema 5 lost the changed route path");
    route.path = "/";
    assertEqual(
      changedManifest,
      initialManifest,
      "route-only watch changed schema-5 semantic or physical data",
    );
    assert(!state.stderr.includes("compile failed"), `watch reported a failure:\n${state.stderr}`);
  } finally {
    try {
      await terminateChild(child);
    } finally {
      rmSync(directory, { recursive: true, force: true });
      rmSync(sidecarPath, { force: true });
    }
  }
}

function writeJson(path, value) {
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`);
}

function expectFailureWithoutMutation(executable, args, diagnostic, guardedDirectory) {
  const before = directorySnapshot(guardedDirectory);
  const result = runCli(executable, args, 1);
  assert(
    diagnostic.test(result.stderr),
    `failure lost diagnostic ${diagnostic}:\n${result.stderr}`,
  );
  assertEqual(
    directorySnapshot(guardedDirectory),
    before,
    `failed command mutated guarded outputs: ${args.join(" ")}`,
  );
}

function assertKeys(value, expected, description) {
  assertEqual(Object.keys(value), expected, `${description} surface or field order drifted`);
}

function assertOffset(value, cssLength, description) {
  assert(Number.isSafeInteger(value), `${description} is not a safe JSON integer`);
  assert(value >= 0 && value <= cssLength, `${description} is outside the CSS artifact`);
}

function sliceUtf8(css, start, end, description) {
  assertOffset(start, css.length, `${description} start`);
  assertOffset(end, css.length, `${description} end`);
  assert(start < end, `${description} is empty or reversed`);
  const bytes = css.subarray(start, end);
  const text = bytes.toString("utf8");
  assert(Buffer.from(text).equals(bytes), `${description} splits UTF-8 or selects invalid UTF-8`);
  return text;
}

function isAsciiWhitespace(byte) {
  return byte === 0x09 || byte === 0x0a || byte === 0x0d || byte === 0x20;
}

function assertWhitespace(css, start, end, description) {
  for (let index = start; index < end; index += 1) {
    assert(isAsciiWhitespace(css[index]), `${description} contains non-whitespace byte ${css[index]}`);
  }
}

function assertDeclarationSeparator(css, start, end, description) {
  for (let index = start; index < end; index += 1) {
    assert(
      css[index] === 0x3b || isAsciiWhitespace(css[index]),
      `${description} contains unexpected byte ${css[index]}`,
    );
  }
}

function assertManifest(manifest, css, schemaVersion, targets, format) {
  assertKeys(
    manifest,
    [
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
      ...(schemaVersion >= 4 ? ["graph"] : []),
    ],
    `manifest schema ${schemaVersion}`,
  );
  assert(manifest.schemaVersion === schemaVersion, `expected manifest schema ${schemaVersion}`);
  assert(manifest.styleIdFormatVersion === 2, "StyleId format version drifted");
  assert(manifest.classNameFormatVersion === 1, "class-name format version drifted");
  assert(manifest.themeIdFormatVersion === 1, "theme ID format version drifted");
  assert(/^[0-9a-f]{32}$/.test(manifest.themeId), "ThemeId is not fixed-width hex");
  assert(manifest.targets === targets, `expected target contract ${targets}`);
  assert(manifest.format === format, `expected CSS format ${format}`);
  assert(manifest.cssBytes === css.length, "manifest CSS byte length is stale");
  assert(manifest.cssSha256 === sha256(css), "manifest CSS digest is stale");
  assert(css.at(-1) === 0x0a, "CSS artifact lost its final newline");
  assert(manifest.styles.length === 1, "physical fixture must produce one semantic style");

  const style = manifest.styles[0];
  assertKeys(style, ["styleId", "className", "origins"], "manifest style");
  assert(/^[0-9a-f]{32}$/.test(style.styleId), "StyleId is not fixed-width hex");
  assert(/^pc_[a-z0-9]+$/.test(style.className), "class name is not canonical");
  assert(css.toString("utf8").includes(`.${style.className}`), "class is absent from final CSS");
  assert(style.origins.length === 1, "fixture must retain one exact source origin");
  const origin = style.origins[0];
  assert(origin.file === "src/card.rs", "source origin path drifted");
  assert(origin.byteStart === 24 && origin.byteEnd === 188, "source origin range drifted");
  const source = readFileSync(join(project, origin.file));
  const macro = source.subarray(origin.byteStart, origin.byteEnd).toString("utf8");
  assert(macro.startsWith('pc!("') && macro.endsWith('")'), "source origin no longer selects pc!");
  assert(macro.includes(origin.source), "source origin and utility payload diverged");
}

function projectGraphTwo(graph) {
  return {
    schemaVersion: 1,
    declarationIdFormatVersion: graph.declarationIdFormatVersion,
    originCoverage: graph.originCoverage,
    applicationCoverage: graph.applicationCoverage,
    declarations: graph.declarations,
    tokens: graph.tokens,
    components: graph.components,
    routes: graph.routes,
    islands: graph.islands,
    edges: graph.edges.filter((edge) => !physicalEdgeKinds.has(edge.kind)),
  };
}

function groupBy(values, key) {
  const groups = new Map();
  for (const value of values) {
    const itemKey = key(value);
    const group = groups.get(itemKey);
    if (group) group.push(value);
    else groups.set(itemKey, [value]);
  }
  return groups;
}

function assertRuleRanges(graph, css, edgeGroups) {
  const rules = graph.physicalRules;
  const byId = new Map(rules.map((rule) => [rule.id, rule]));
  const nesting = graph.edges.filter((edge) => edge.kind === "ruleNestedInRule");
  const parentByChild = new Map();
  for (const edge of nesting) {
    assert(!parentByChild.has(edge.from), `${edge.from} has more than one parent rule`);
    parentByChild.set(edge.from, edge.to);
  }

  let previousStart = -1;
  for (const [index, rule] of rules.entries()) {
    assertKeys(
      rule,
      [
        "id",
        "ordinal",
        "kind",
        "byteStart",
        "byteEnd",
        "headerByteStart",
        "headerByteEnd",
      ],
      `physical rule ${index}`,
    );
    assert(rule.ordinal === index, `physical rule ordinal ${index} drifted`);
    assert(rule.id === `css-rule:${index.toString(16).padStart(8, "0")}`, `invalid rule ID ${rule.id}`);
    assert(rule.kind === "qualified" || rule.kind === "media", `unknown rule kind ${rule.kind}`);
    assert(rule.byteStart > previousStart, "physical rule preorder is not strictly increasing");
    previousStart = rule.byteStart;
    const whole = sliceUtf8(css, rule.byteStart, rule.byteEnd, `${rule.id} whole range`);
    const header = sliceUtf8(
      css,
      rule.headerByteStart,
      rule.headerByteEnd,
      `${rule.id} header range`,
    );
    assert(rule.headerByteStart === rule.byteStart, `${rule.id} header does not start at the rule`);
    assert(rule.headerByteEnd < rule.byteEnd, `${rule.id} header is outside the rule`);
    assert(whole.endsWith("}"), `${rule.id} does not select its closing brace`);
    assert(header === header.trimEnd(), `${rule.id} header contains trailing whitespace`);
    assert(
      rule.kind === "media" ? header.startsWith("@media ") : !header.startsWith("@"),
      `${rule.id} kind does not match its header: ${header}`,
    );
    let opening = rule.headerByteEnd;
    while (opening < rule.byteEnd && isAsciiWhitespace(css[opening])) opening += 1;
    assert(css[opening] === 0x7b, `${rule.id} header is not followed by an opening brace`);
  }

  for (const rule of rules) {
    const possibleParents = rules
      .filter(
        (candidate) =>
          candidate.kind === "media" &&
          candidate.byteStart < rule.byteStart &&
          rule.byteEnd < candidate.byteEnd,
      )
      .sort(
        (left, right) =>
          left.byteEnd - left.byteStart - (right.byteEnd - right.byteStart),
      );
    const expectedParent = possibleParents[0]?.id;
    assert(
      parentByChild.get(rule.id) === expectedParent,
      `${rule.id} direct media containment is incomplete or incorrect`,
    );
  }

  for (const edge of nesting) {
    const child = byId.get(edge.from);
    const parent = byId.get(edge.to);
    assert(parent.kind === "media", `${edge.to} is not a media parent`);
    assert(
      parent.byteStart < child.byteStart && child.byteEnd < parent.byteEnd,
      `${edge.from} is not strictly nested inside ${edge.to}`,
    );
  }
  for (const media of rules.filter((rule) => rule.kind === "media")) {
    assert(
      nesting.some((edge) => edge.to === media.id),
      `${media.id} is an empty or unrepresented media wrapper`,
    );
  }

  const topLevel = rules.filter((rule) => !parentByChild.has(rule.id));
  let cursor = 0;
  for (const rule of topLevel) {
    assertWhitespace(css, cursor, rule.byteStart, "top-level CSS gap");
    assert(rule.byteStart >= cursor, "top-level physical rules overlap");
    cursor = rule.byteEnd;
  }
  assertWhitespace(css, cursor, css.length, "CSS suffix outside physical rules");

  const belongs = edgeGroups.get("physicalDeclarationBelongsToRule") ?? [];
  const declarationsByRule = groupBy(belongs, (edge) => edge.to);
  for (const media of rules.filter((rule) => rule.kind === "media")) {
    assert(!(declarationsByRule.get(media.id)?.length), `${media.id} directly owns declarations`);
  }
  return { byId, parentByChild, declarationsByRule };
}

function assertDeclarationRanges(graph, css, rulesById, edgeGroups, fixtureSpecific) {
  const declarations = graph.physicalDeclarations;
  const byId = new Map(declarations.map((declaration) => [declaration.id, declaration]));
  const belongs = edgeGroups.get("physicalDeclarationBelongsToRule") ?? [];
  const ownerByDeclaration = new Map();
  for (const edge of belongs) {
    assert(!ownerByDeclaration.has(edge.from), `${edge.from} belongs to more than one rule`);
    ownerByDeclaration.set(edge.from, edge.to);
  }
  assert(ownerByDeclaration.size === declarations.length, "physical declaration ownership is incomplete");

  const grouped = new Map();
  for (const declaration of declarations) {
    assertKeys(
      declaration,
      [
        "id",
        "ordinal",
        "property",
        "important",
        "generated",
        "byteStart",
        "byteEnd",
        "propertyByteStart",
        "propertyByteEnd",
        "valueByteStart",
        "valueByteEnd",
      ],
      declaration.id,
    );
    const match = declaration.id.match(/^css-decl:([0-9a-f]{8}):([0-9a-f]{8})$/);
    assert(match, `invalid physical declaration ID ${declaration.id}`);
    assert(Number.parseInt(match[2], 16) === declaration.ordinal, `${declaration.id} ordinal drifted`);
    assert(typeof declaration.important === "boolean", `${declaration.id} important is not boolean`);
    assert(typeof declaration.generated === "boolean", `${declaration.id} generated is not boolean`);
    const ownerId = ownerByDeclaration.get(declaration.id);
    const owner = rulesById.get(ownerId);
    assert(owner?.kind === "qualified", `${declaration.id} does not belong to a qualified rule`);
    assert(
      Number.parseInt(match[1], 16) === owner.ordinal,
      `${declaration.id} encodes a different owning rule`,
    );

    const whole = sliceUtf8(css, declaration.byteStart, declaration.byteEnd, declaration.id);
    const property = sliceUtf8(
      css,
      declaration.propertyByteStart,
      declaration.propertyByteEnd,
      `${declaration.id} property`,
    );
    const value = sliceUtf8(
      css,
      declaration.valueByteStart,
      declaration.valueByteEnd,
      `${declaration.id} value`,
    );
    assert(declaration.byteStart === declaration.propertyByteStart, `${declaration.id} has leading bytes`);
    assert(
      declaration.byteStart < declaration.propertyByteEnd &&
        declaration.propertyByteEnd <= declaration.valueByteStart &&
        declaration.valueByteStart < declaration.valueByteEnd &&
        declaration.valueByteEnd <= declaration.byteEnd,
      `${declaration.id} subranges are invalid`,
    );
    assert(
      owner.byteStart < declaration.byteStart && declaration.byteEnd < owner.byteEnd,
      `${declaration.id} lies outside ${owner.id}`,
    );
    assert(property === declaration.property, `${declaration.id} property slice drifted`);
    assert(whole.startsWith(property), `${declaration.id} complete slice lost its property`);
    const separator = css
      .subarray(declaration.propertyByteEnd, declaration.valueByteStart)
      .toString("utf8")
      .trim();
    assert(separator === ":", `${declaration.id} property/value separator is invalid`);
    if (declaration.important) {
      const suffix = css
        .subarray(declaration.valueByteEnd, declaration.byteEnd)
        .toString("utf8");
      assert(/^\s*!important$/.test(suffix), `${declaration.id} important suffix is invalid`);
    } else {
      assert(declaration.valueByteEnd === declaration.byteEnd, `${declaration.id} has unexplained suffix`);
    }
    assert(value.length > 0, `${declaration.id} value is empty`);

    const list = grouped.get(owner.id) ?? [];
    list.push(declaration);
    grouped.set(owner.id, list);
  }

  for (const [ruleId, items] of grouped) {
    items.sort((left, right) => left.ordinal - right.ordinal);
    for (const [index, item] of items.entries()) {
      assert(item.ordinal === index, `${ruleId} declaration ordinals are not contiguous`);
      if (index > 0) {
        const previous = items[index - 1];
        assert(previous.byteEnd < item.byteStart, `${ruleId} declarations overlap`);
        assertDeclarationSeparator(
          css,
          previous.byteEnd,
          item.byteStart,
          `${ruleId} declaration separator`,
        );
      }
    }
  }

  if (fixtureSpecific) {
    const unicode = declarations.find((declaration) => declaration.property === "--label");
    assert(unicode, "fixture lost its Unicode custom property");
    assert(
      sliceUtf8(css, unicode.valueByteStart, unicode.valueByteEnd, "Unicode value") === '"😀;"',
      "Unicode/semicolon value range drifted",
    );
    const important = declarations.find((declaration) => declaration.property === "opacity");
    assert(important?.important, "fixture lost its important physical declaration");
  }
  return { byId, ownerByDeclaration };
}

function assertThemeProducer(graph, css, rulesById, declarationsById, edgeGroups) {
  assertEqual(
    graph.syntheticProducers,
    [{ id: "producer:theme", kind: "theme" }],
    "theme producer contract drifted",
  );
  const rootRule = graph.physicalRules.find(
    (rule) =>
      sliceUtf8(css, rule.headerByteStart, rule.headerByteEnd, `${rule.id} header`) === ":root",
  );
  assert(rootRule, "theme output has no :root physical rule");
  const belongs = edgeGroups.get("physicalDeclarationBelongsToRule") ?? [];
  const rootDeclarations = new Set(
    belongs.filter((edge) => edge.to === rootRule.id).map((edge) => edge.from),
  );
  assert(rootDeclarations.size > 0, "theme :root rule contains no physical declarations");

  const themeEdges = edgeGroups.get("syntheticProducerProducesPhysicalDeclaration") ?? [];
  assert(themeEdges.length === rootDeclarations.size, "theme producer coverage is incomplete");
  assert(
    themeEdges.every(
      (edge) => edge.from === "producer:theme" && rootDeclarations.has(edge.to),
    ),
    "theme producer reached a non-theme declaration",
  );
  const semanticTargets = new Set(
    (edgeGroups.get("declarationContributesToPhysicalDeclaration") ?? []).map(
      (edge) => edge.to,
    ),
  );
  for (const id of rootDeclarations) {
    const declaration = declarationsById.get(id);
    assert(declaration.property.startsWith("--"), `${id} theme property is not custom`);
    assert(!declaration.generated, `${id} theme declaration is marked compiler-generated`);
    assert(!semanticTargets.has(id), `${id} theme declaration has a semantic contributor`);
  }
  assert(rulesById.get(rootRule.id) === rootRule, "theme rule identity map drifted");
}

function assertGeneratedEffects(graph, declarationsById, edgeGroups) {
  const generated = graph.physicalDeclarations.filter((declaration) => declaration.generated);
  assertEqual(
    generated.map((declaration) => declaration.property),
    ["--pc-shadow", "--pc-ring-width", "--pc-ring-color", "box-shadow"],
    "ring/shadow generated declaration set drifted",
  );
  const contributions = edgeGroups.get("declarationContributesToPhysicalDeclaration") ?? [];
  const contributorSets = generated.map((declaration) =>
    sorted(
      contributions
        .filter((edge) => edge.to === declaration.id)
        .map((edge) => edge.from),
    ),
  );
  assert(contributorSets.every((contributors) => contributors.length === 3), "generated effects lost many-to-many lineage");
  for (const contributors of contributorSets.slice(1)) {
    assertEqual(contributors, contributorSets[0], "generated effect contributor sets diverged");
  }
  assertEqual(
    contributorSets[0].map((id) => id.slice(-8)),
    ["00000004", "00000005", "00000006"],
    "ring/shadow semantic contributors drifted",
  );
  for (const contributor of contributorSets[0]) {
    const outputs = contributions.filter((edge) => edge.from === contributor && declarationsById.get(edge.to)?.generated);
    assert(outputs.length === 4, `${contributor} no longer contributes to every generated effect`);
  }
}

function assertGraphTwo(manifest, css, { theme = true, fixtureSpecific = true } = {}) {
  const graph = manifest.graph;
  assert(graph && typeof graph === "object", "schema 5 has no graph");
  assertKeys(
    graph,
    [
      "schemaVersion",
      "declarationIdFormatVersion",
      "physicalRuleIdFormatVersion",
      "physicalDeclarationIdFormatVersion",
      "originCoverage",
      "applicationCoverage",
      "physicalCoverage",
      "declarations",
      "tokens",
      "components",
      "routes",
      "islands",
      "syntheticProducers",
      "physicalRules",
      "physicalDeclarations",
      "edges",
    ],
    "manifest graph schema 2",
  );
  assert(graph.schemaVersion === 2, "schema 5 graph is not schema 2");
  assert(graph.declarationIdFormatVersion === 1, "semantic declaration ID format drifted");
  assert(graph.physicalRuleIdFormatVersion === 1, "physical rule ID format drifted");
  assert(
    graph.physicalDeclarationIdFormatVersion === 1,
    "physical declaration ID format drifted",
  );
  assert(graph.originCoverage === "compiler-verified-complete", "origin coverage is incomplete");
  assert(
    graph.applicationCoverage === "adapter-attested-complete",
    "application coverage is incomplete",
  );
  assert(
    graph.physicalCoverage === "compiler-verified-complete",
    "physical coverage is incomplete",
  );

  for (const [description, nodes] of [
    ["semantic declarations", graph.declarations],
    ["tokens", graph.tokens],
    ["components", graph.components],
    ["routes", graph.routes],
    ["islands", graph.islands],
    ["synthetic producers", graph.syntheticProducers],
    ["physical rules", graph.physicalRules],
    ["physical declarations", graph.physicalDeclarations],
  ]) {
    assertSortedUnique(nodes.map((node) => node.id), `graph ${description}`);
  }
  assertSortedUnique(graph.edges.map(edgeKey), "graph edges");

  const styleIds = new Set(manifest.styles.map((style) => `style:${style.styleId}`));
  const declarations = new Set(graph.declarations.map((node) => node.id));
  const tokens = new Set(graph.tokens.map((node) => node.id));
  const components = new Set(graph.components.map((node) => node.id));
  const routes = new Set(graph.routes.map((node) => node.id));
  const islands = new Set(graph.islands.map((node) => node.id));
  const producers = new Set(graph.syntheticProducers.map((node) => node.id));
  const rules = new Set(graph.physicalRules.map((node) => node.id));
  const physicalDeclarations = new Set(graph.physicalDeclarations.map((node) => node.id));
  const allNodes = [
    ...styleIds,
    ...declarations,
    ...tokens,
    ...components,
    ...routes,
    ...islands,
    ...producers,
    ...rules,
    ...physicalDeclarations,
  ];
  assert(new Set(allNodes).size === allNodes.length, "graph node IDs are not globally unique");
  assert(allNodes.length <= 65_535, "graph node budget exceeded");
  assert(graph.edges.length <= 65_535, "graph edge budget exceeded");

  for (const declaration of graph.declarations) {
    const match = declaration.id.match(/^decl:([0-9a-f]{32}):([0-9a-f]{8})$/);
    assert(match, `invalid semantic declaration ID ${declaration.id}`);
    assert(declaration.styleId === match[1], `${declaration.id} styleId field drifted`);
    assert(styleIds.has(`style:${match[1]}`), `${declaration.id} references an unknown style`);
    assert(Number.parseInt(match[2], 16) === declaration.ordinal, `${declaration.id} ordinal drifted`);
  }
  for (const token of graph.tokens) {
    assert(
      token.id === `token:${token.kind}:${token.tokenId}` && /^[0-9a-f]{8}$/.test(token.tokenId),
      `invalid token identity ${token.id}`,
    );
  }

  const endpointChecks = {
    styleHasDeclaration: (edge) => styleIds.has(edge.from) && declarations.has(edge.to),
    declarationUsesToken: (edge) => declarations.has(edge.from) && tokens.has(edge.to),
    componentUsesDeclaration: (edge) => components.has(edge.from) && declarations.has(edge.to),
    routeUsesComponent: (edge) => routes.has(edge.from) && components.has(edge.to),
    islandUsesComponent: (edge) => islands.has(edge.from) && components.has(edge.to),
    declarationContributesToPhysicalDeclaration: (edge) =>
      declarations.has(edge.from) && physicalDeclarations.has(edge.to),
    syntheticProducerProducesPhysicalDeclaration: (edge) =>
      producers.has(edge.from) && physicalDeclarations.has(edge.to),
    physicalDeclarationBelongsToRule: (edge) =>
      physicalDeclarations.has(edge.from) && rules.has(edge.to),
    ruleNestedInRule: (edge) => rules.has(edge.from) && rules.has(edge.to),
  };
  for (const edge of graph.edges) {
    const check = endpointChecks[edge.kind];
    assert(check, `unknown graph edge kind ${edge.kind}`);
    assert(check(edge), `invalid typed endpoints for ${edgeKey(edge)}`);
  }

  const edgeGroups = groupBy(graph.edges, (edge) => edge.kind);
  const contributionEdges = edgeGroups.get("declarationContributesToPhysicalDeclaration") ?? [];
  for (const declaration of graph.declarations) {
    assert(
      contributionEdges.some((edge) => edge.from === declaration.id),
      `${declaration.id} lacks a physical contribution`,
    );
  }
  const semanticTargets = groupBy(contributionEdges, (edge) => edge.to);
  const syntheticTargets = groupBy(
    edgeGroups.get("syntheticProducerProducesPhysicalDeclaration") ?? [],
    (edge) => edge.to,
  );
  for (const declaration of graph.physicalDeclarations) {
    const semanticCount = semanticTargets.get(declaration.id)?.length ?? 0;
    const syntheticCount = syntheticTargets.get(declaration.id)?.length ?? 0;
    assert(semanticCount + syntheticCount > 0, `${declaration.id} has no producer`);
    assert(!(semanticCount && syntheticCount), `${declaration.id} mixes semantic and synthetic producers`);
  }

  const { byId: rulesById } = assertRuleRanges(graph, css, edgeGroups);
  const { byId: declarationsById } = assertDeclarationRanges(
    graph,
    css,
    rulesById,
    edgeGroups,
    fixtureSpecific,
  );
  if (theme) {
    assertThemeProducer(graph, css, rulesById, declarationsById, edgeGroups);
  } else {
    assertEqual(graph.syntheticProducers, [], "theme-free trace contains a synthetic producer");
    assert(
      !(edgeGroups.get("syntheticProducerProducesPhysicalDeclaration")?.length),
      "theme-free trace contains synthetic producer edges",
    );
    assert(
      !graph.physicalRules.some(
        (rule) =>
          sliceUtf8(css, rule.headerByteStart, rule.headerByteEnd, `${rule.id} header`) === ":root",
      ),
      "theme-free trace contains a :root rule",
    );
  }
  if (fixtureSpecific) assertGeneratedEffects(graph, declarationsById, edgeGroups);
  return { edgeGroups, rulesById, declarationsById };
}

function physicalValue(css, declaration) {
  return sliceUtf8(
    css,
    declaration.valueByteStart,
    declaration.valueByteEnd,
    `${declaration.id} value`,
  );
}

function contributionSources(graph, physicalId) {
  return graph.edges
    .filter(
      (edge) =>
        edge.kind === "declarationContributesToPhysicalDeclaration" && edge.to === physicalId,
    )
    .map((edge) => edge.from);
}

process.once("exit", () => rmSync(runtime, { recursive: true, force: true }));

rmSync(runtime, { recursive: true, force: true });
mkdirSync(runtime, { recursive: true });
cpSync(fixture, project, { recursive: true });
const executable = resolveExecutable();

mkdirSync(join(project, "out"));
for (const version of [3, 4, 5]) {
  runCli(
    executable,
    compileArguments({
      css: `out/schema-${version}.css`,
      manifest: `out/schema-${version}.manifest.json`,
      version,
      reachability: version >= 4 ? reachabilityName : undefined,
    }),
  );
}

const cssThree = readFileSync(join(project, "out", "schema-3.css"));
const cssFour = readFileSync(join(project, "out", "schema-4.css"));
const cssFive = readFileSync(join(project, "out", "schema-5.css"));
const manifestThree = JSON.parse(readFileSync(join(project, "out", "schema-3.manifest.json")));
const manifestFour = JSON.parse(readFileSync(join(project, "out", "schema-4.manifest.json")));
const manifestFiveBytes = readFileSync(join(project, "out", "schema-5.manifest.json"));
const manifestFive = JSON.parse(manifestFiveBytes);

assertEqual(cssThree, cssFour, "schema 4 changed schema-3 CSS bytes");
assertEqual(cssFour, cssFive, "schema 5 changed established CSS bytes");
assertEqual(cssFive, expectedCss, "physical trace CSS golden drifted");
assertEqual(manifestFiveBytes, expectedManifest, "manifest schema-5 golden drifted");
assertManifest(manifestThree, cssThree, 3, "modern", "minified");
assertManifest(manifestFour, cssFour, 4, "modern", "minified");
assertManifest(manifestFive, cssFive, 5, "modern", "minified");
assert(manifestThree.graph === undefined, "schema 3 unexpectedly contains a graph");
assert(manifestFour.graph.schemaVersion === 1, "schema 4 graph is not schema 1");
assertEqual(manifestThree.styles, manifestFive.styles, "schema 5 changed stable style provenance");
assertEqual(projectGraphTwo(manifestFive.graph), manifestFour.graph, "graph 2 does not project exactly to graph 1");
assertGraphTwo(manifestFive, cssFive);

const mediaSource = Buffer.from(
  'fn media() {\n    let _ = pc!("md:block");\n    let _ = pc!("md:flex");\n}\n',
);
writeFileSync(join(project, "src", "media.rs"), mediaSource);
const mediaSites = ['pc!("md:block")', 'pc!("md:flex")'].map((invocation) => {
  const byteStart = mediaSource.indexOf(invocation);
  assert(byteStart >= 0, `media merge fixture lost ${invocation}`);
  return {
    file: "src/media.rs",
    byteStart,
    byteEnd: byteStart + Buffer.byteLength(invocation),
  };
});
writeJson(join(project, "media.reachability.json"), {
  schema: 1,
  applicationCoverage: "complete",
  components: [{ id: "app::media", sites: mediaSites }],
  routes: [{ id: "media", path: "/media", components: ["app::media"] }],
  islands: [],
});
for (const version of [3, 4, 5]) {
  runCli(
    executable,
    compileArguments({
      css: `out/media-${version}.css`,
      manifest: `out/media-${version}.manifest.json`,
      version,
      reachability: version >= 4 ? "media.reachability.json" : undefined,
      theme: false,
      source: "src/media.rs",
    }),
  );
}
const mediaCssThree = readFileSync(join(project, "out", "media-3.css"));
const mediaCssFour = readFileSync(join(project, "out", "media-4.css"));
const mediaCssFive = readFileSync(join(project, "out", "media-5.css"));
const mediaManifestThree = JSON.parse(
  readFileSync(join(project, "out", "media-3.manifest.json")),
);
const mediaManifestFour = JSON.parse(
  readFileSync(join(project, "out", "media-4.manifest.json")),
);
const mediaManifestFive = JSON.parse(
  readFileSync(join(project, "out", "media-5.manifest.json")),
);
assertEqual(mediaCssThree, mediaCssFour, "media merge changed schema-4 CSS bytes");
assertEqual(mediaCssFour, mediaCssFive, "media merge changed schema-5 CSS bytes");
assertEqual(
  (mediaCssFive.toString("utf8").match(/@media/gu) ?? []).length,
  1,
  "adjacent compatible media queries were not merged",
);
assertEqual(mediaManifestThree.styles, mediaManifestFive.styles, "media merge changed style provenance");
assertEqual(
  projectGraphTwo(mediaManifestFive.graph),
  mediaManifestFour.graph,
  "merged media graph 2 does not project exactly to graph 1",
);
assertGraphTwo(mediaManifestFive, mediaCssFive, { theme: false, fixtureSpecific: false });
assertEqual(
  mediaManifestFive.graph.physicalRules.map((rule) => rule.kind),
  ["media", "qualified", "qualified"],
  "merged media physical topology drifted",
);
assertEqual(
  mediaManifestFive.graph.edges.filter((edge) => edge.kind === "ruleNestedInRule").length,
  2,
  "merged media children lost physical nesting",
);

for (const [name, targets, format] of [
  ["media-pretty-modern", "modern", "pretty"],
  ["media-minified-none", "none", "minified"],
  ["media-pretty-none", "none", "pretty"],
]) {
  runCli(
    executable,
    compileArguments({
      css: `out/${name}.css`,
      manifest: `out/${name}.manifest.json`,
      version: 5,
      reachability: "media.reachability.json",
      targets,
      format,
      theme: false,
      source: "src/media.rs",
    }),
  );
  const profileCss = readFileSync(join(project, "out", `${name}.css`));
  const profileManifest = JSON.parse(
    readFileSync(join(project, "out", `${name}.manifest.json`)),
  );
  assertEqual(
    (profileCss.toString("utf8").match(/@media/gu) ?? []).length,
    1,
    `${name} did not retain one merged media wrapper`,
  );
  assertGraphTwo(profileManifest, profileCss, { theme: false, fixtureSpecific: false });
  assertEqual(
    profileManifest.graph.physicalRules.map((rule) => rule.kind),
    ["media", "qualified", "qualified"],
    `${name} changed merged media topology`,
  );
  assertEqual(
    projectGraphTwo(profileManifest.graph),
    mediaManifestFour.graph,
    `${name} changed merged media semantic graph`,
  );
}

runCli(
  executable,
  compileArguments({
    css: "out/no-theme-3.css",
    manifest: "out/no-theme-3.manifest.json",
    version: 3,
    theme: false,
  }),
);
runCli(
  executable,
  compileArguments({
    css: "out/no-theme-5.css",
    manifest: "out/no-theme-5.manifest.json",
    version: 5,
    reachability: reachabilityName,
    theme: false,
  }),
);
const noThemeCssThree = readFileSync(join(project, "out", "no-theme-3.css"));
const noThemeCssFive = readFileSync(join(project, "out", "no-theme-5.css"));
const noThemeManifest = JSON.parse(
  readFileSync(join(project, "out", "no-theme-5.manifest.json")),
);
assertEqual(noThemeCssFive, noThemeCssThree, "schema 5 changed theme-free CSS bytes");
assertManifest(noThemeManifest, noThemeCssFive, 5, "modern", "minified");
assertEqual(
  projectGraphTwo(noThemeManifest.graph),
  manifestFour.graph,
  "theme emission changed the semantic graph",
);
assertGraphTwo(noThemeManifest, noThemeCssFive, { theme: false });

runCli(
  executable,
  compileArguments({
    css: "out/pretty.css",
    manifest: "out/pretty.manifest.json",
    version: 5,
    reachability: reachabilityName,
    format: "pretty",
  }),
);
const prettyCss = readFileSync(join(project, "out", "pretty.css"));
const prettyManifest = JSON.parse(readFileSync(join(project, "out", "pretty.manifest.json")));
assertManifest(prettyManifest, prettyCss, 5, "modern", "pretty");
assertGraphTwo(prettyManifest, prettyCss);
assertEqual(projectGraphTwo(prettyManifest.graph), manifestFour.graph, "pretty output changed the semantic graph");
assert(!prettyCss.equals(cssFive), "pretty CSS unexpectedly equals minified CSS");
assert(prettyCss.toString("utf8").includes("\n  "), "pretty CSS has no Lightning indentation");
assertEqual(
  prettyManifest.graph.physicalRules.map(({ id, kind }) => [id, kind]),
  manifestFive.graph.physicalRules.map(({ id, kind }) => [id, kind]),
  "pretty output changed physical rule topology",
);
assertEqual(
  prettyManifest.graph.physicalDeclarations.map(({ id, property, important, generated }) => [
    id,
    property,
    important,
    generated,
  ]),
  manifestFive.graph.physicalDeclarations.map(({ id, property, important, generated }) => [
    id,
    property,
    important,
    generated,
  ]),
  "pretty output changed physical declaration topology",
);
assert(
  prettyManifest.graph.physicalDeclarations.some(
    (declaration, index) =>
      declaration.byteStart !== manifestFive.graph.physicalDeclarations[index].byteStart,
  ),
  "pretty physical ranges did not move with formatting",
);

runCli(
  executable,
  compileArguments({
    css: "out/none.css",
    manifest: "out/none.manifest.json",
    version: 5,
    reachability: reachabilityName,
    targets: "none",
  }),
);
const noneCss = readFileSync(join(project, "out", "none.css"));
const noneManifest = JSON.parse(readFileSync(join(project, "out", "none.manifest.json")));
assertManifest(noneManifest, noneCss, 5, "none", "minified");
assertGraphTwo(noneManifest, noneCss);
assertEqual(projectGraphTwo(noneManifest.graph), manifestFour.graph, "target selection changed the semantic graph");
assert(!noneCss.equals(cssFive), "modern and none target artifacts unexpectedly match");
const modernColor = manifestFive.graph.physicalDeclarations.find(
  (declaration) => declaration.property === "color",
);
const noneColor = noneManifest.graph.physicalDeclarations.find(
  (declaration) => declaration.property === "color",
);
assert(modernColor && noneColor, "light-dark fixture lost its color declaration");
assert(
  physicalValue(cssFive, modernColor) ===
    "var(--lightningcss-light,red)var(--lightningcss-dark,#00f)",
  "modern target no longer lowers light-dark through Lightning CSS",
);
assert(
  physicalValue(noneCss, noneColor) === "light-dark(red,#00f)",
  "none target no longer preserves light-dark",
);
assert(modernColor.id === noneColor.id, "target-only value rewrite changed declaration topology");
assertEqual(
  contributionSources(manifestFive.graph, modernColor.id),
  contributionSources(noneManifest.graph, noneColor.id),
  "target-only value rewrite changed semantic lineage",
);

const baseReachability = JSON.parse(readFileSync(join(project, reachabilityName), "utf8"));
const expanded = structuredClone(baseReachability);
expanded.components.push({ id: "app::shell", sites: [] });
expanded.routes[0].components.push("app::shell");
expanded.routes.push({ id: "settings", path: "/settings", components: ["app::shell"] });
expanded.islands[0].components.push("app::shell");
expanded.islands.push({ id: "shell", name: "Shell", components: ["app::shell"] });
const reordered = structuredClone(expanded);
reordered.components.reverse();
reordered.routes.reverse();
reordered.islands.reverse();
for (const component of reordered.components) component.sites.reverse();
for (const route of reordered.routes) route.components.reverse();
for (const island of reordered.islands) island.components.reverse();
writeJson(join(project, "expanded.reachability.json"), expanded);
writeJson(join(project, "reordered.reachability.json"), reordered);
for (const name of ["expanded", "reordered"]) {
  runCli(
    executable,
    compileArguments({
      css: `out/${name}.css`,
      manifest: `out/${name}.manifest.json`,
      version: 5,
      reachability: `${name}.reachability.json`,
    }),
  );
}
assertEqual(
  readFileSync(join(project, "out", "expanded.css")),
  readFileSync(join(project, "out", "reordered.css")),
  "reachability order changed CSS bytes",
);
assertEqual(
  readFileSync(join(project, "out", "expanded.manifest.json")),
  readFileSync(join(project, "out", "reordered.manifest.json")),
  "reachability order changed canonical schema-5 bytes",
);

mkdirSync(join(project, "bundle-v5"));
runCli(executable, bundleArguments("bundle-v5", { reachability: reachabilityName }));
const bundleCssPath = join(project, "bundle-v5", "card.css");
const bundleManifestPath = join(project, "bundle-v5", "card.manifest.json");
assertEqual(readFileSync(bundleCssPath), expectedCss, "bundle schema-5 CSS drifted from compile");
assertEqual(
  readFileSync(bundleManifestPath),
  expectedManifest,
  "bundle schema-5 manifest drifted from golden",
);
const bundleBeforeCheck = directorySnapshot(join(project, "bundle-v5"));
runCli(
  executable,
  bundleArguments("bundle-v5", { reachability: reachabilityName, check: true }),
);
assertEqual(
  directorySnapshot(join(project, "bundle-v5")),
  bundleBeforeCheck,
  "successful schema-5 bundle --check mutated artifacts",
);
writeFileSync(bundleCssPath, Buffer.concat([expectedCss, Buffer.from("/* stale */\n")]));
const staleBundle = directorySnapshot(join(project, "bundle-v5"));
const staleCheck = runCli(
  executable,
  bundleArguments("bundle-v5", { reachability: reachabilityName, check: true }),
  1,
);
assert(staleCheck.stderr.trim().length > 0, "stale bundle --check lost its diagnostic");
assertEqual(
  directorySnapshot(join(project, "bundle-v5")),
  staleBundle,
  "failed schema-5 bundle --check mutated artifacts",
);

mkdirSync(join(project, "guard"));
writeFileSync(join(project, "guard", "protected.css"), "protected css\n");
writeFileSync(join(project, "guard", "protected.manifest.json"), "protected manifest\n");
const guardedCompile = ({ version, reachability, manifest = true }) =>
  compileArguments({
    css: "guard/protected.css",
    manifest: manifest ? "guard/protected.manifest.json" : undefined,
    version,
    reachability,
  });

expectFailureWithoutMutation(
  executable,
  guardedCompile({ version: 5 }),
  /manifest version 5 requires `--reachability`/,
  join(project, "guard"),
);
expectFailureWithoutMutation(
  executable,
  guardedCompile({ reachability: reachabilityName }),
  /`--reachability` requires `--manifest-version 4` or `5`/,
  join(project, "guard"),
);
expectFailureWithoutMutation(
  executable,
  guardedCompile({ version: 3, reachability: reachabilityName }),
  /`--reachability` requires `--manifest-version 4` or `5`/,
  join(project, "guard"),
);
expectFailureWithoutMutation(
  executable,
  guardedCompile({ version: 5, reachability: reachabilityName, manifest: false }),
  /`--manifest-version` requires manifest output/,
  join(project, "guard"),
);
expectFailureWithoutMutation(
  executable,
  guardedCompile({ version: 6, reachability: reachabilityName }),
  /manifest version must be 3, 4, or 5/,
  join(project, "guard"),
);
const invalidOwnership = structuredClone(baseReachability);
invalidOwnership.components[0].sites[0].byteStart += 1;
writeJson(join(project, "invalid-ownership.json"), invalidOwnership);
expectFailureWithoutMutation(
  executable,
  guardedCompile({ version: 5, reachability: "invalid-ownership.json" }),
  /has no component(?: owner)?/,
  join(project, "guard"),
);
expectFailureWithoutMutation(
  executable,
  bundleArguments("guard"),
  /manifest version 5 requires `--reachability`/,
  join(project, "guard"),
);
const guardBeforeCheckOptions = directorySnapshot(join(project, "guard"));
const checkOptions = runCli(
  executable,
  ["check", "--style", "flex", "--manifest-version", "5"],
  1,
);
assert(
  /`check` does not accept output, manifest, theme, reachability, or pruning options/.test(
    checkOptions.stderr,
  ),
  `check schema-5 option diagnostic drifted:\n${checkOptions.stderr}`,
);
assertEqual(
  directorySnapshot(join(project, "guard")),
  guardBeforeCheckOptions,
  "check option validation mutated guarded outputs",
);

await assertWatchContract(executable);

console.log(
  `physical trace gate passed: CSS ${sha256(expectedCss)}, manifest ${sha256(expectedManifest)}, ` +
    `${manifestFive.graph.physicalRules.length} rules, ` +
    `${manifestFive.graph.physicalDeclarations.length} declarations`,
);
