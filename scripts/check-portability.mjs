import { createHash } from "node:crypto";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readlinkSync,
  readdirSync,
  rmSync,
  statSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { spawn, spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const executable = resolve(
  process.env.PLIEGO_CSSC ??
    join(root, "target", "debug", process.platform === "win32" ? "pliego-cssc.exe" : "pliego-cssc"),
);
const runtime = resolve(
  process.env.PLIEGO_PORTABILITY_RUNTIME ??
    join(root, "target", "portability contract", `run-${process.pid}-${Date.now()}`),
);
const project = join(runtime, "project");
const foreignCwd = join(runtime, "foreign-cwd");
const sourceName = "café.rs";
const logicalSource = `src/${sourceName}`;
const expectedCssSha256 = "d208d2960bd28fb29354b1a89c74c106686c8a8a2d71a9cc7cc9971eb2e97070";
const expectedManifestSha256 = "3a89e04856d5ff8244c29e91325271d6a57e33eec99242b121308c477facd169";
const expectedControlHashes = {
  "pliego.css.findings.json": "75015787cd20bd52e4c1d504943b675785f55d7376a583a103cad4ab5c7de464",
  "pliego.css.manifest.json": "bac4099153b23cfd990fc2e70506153d08b725ca27661ad102934f45df721ac1",
  "pliego.css.receipt.json": "cb680d32a41321b089cf2fd98b7f60501fa8ba306f408d5f448a11551598b9b1",
};
const expectedBundleControlHashes = {
  "app.css": "d208d2960bd28fb29354b1a89c74c106686c8a8a2d71a9cc7cc9971eb2e97070",
  "app.css.map": "39a03b016583b0827ec8bc92a2a1677dcb4d151324dcdd9c77d303af29b1dddf",
  "app.manifest.json": "0b04b7cf34339e598aeb256c5667a808461dfd02e90ce18aaec0a60de501eb9a",
  "pliego.assets.json": "b004fd1b8c28a13b1af0f80c105fc71a7615bab45ea3a1619f7f65084d1b343e",
  "pliego.css.findings.json": "e03d275890f0db3ccceaddfb6dc5b43acd0f0f731b99c77bd0b9bd9b6bf61d40",
  "pliego.css.manifest.json": "19bbb01d407c958d0938ae536c4c08889d1084fd86f6ee246bae0d0326a98db3",
  "pliego.css.receipt.json": "2bac654c6060a50f4a1fa21ea22baa1ef5d87c7f741aa37c41a5068dba1bf52d",
  "pliego.index.json": "6d60bcce6f91e4dc71ca98d921e9ce8bb267cb4d027d51d664b4e87477313dec",
  "pliego.tokens.json": "bb525a903714688d1ada713bff7a0c831631ee1c08b10434d1d03c79d742d204",
};
const expectedAssetPlanControlHashes = {
  "pliego.css.findings.json": "67e1d24a75adc042312286ade5572379ec706df1ad177031b9160862d27c5e59",
  "pliego.css.manifest.json": "004d39320c6aab32a6293896db98bbef21a040ec457afc2114e82fc67e7ab795",
  "pliego.css.receipt.json": "d392124440e0295760fb5e8629fb5e4d07271c1f5fcffc03b628bdfbdbab65ce",
};
const expectedCompileControlHashes = {
  "app.css": "d208d2960bd28fb29354b1a89c74c106686c8a8a2d71a9cc7cc9971eb2e97070",
  "app.css.map": "39a03b016583b0827ec8bc92a2a1677dcb4d151324dcdd9c77d303af29b1dddf",
  "app.manifest.json": "3a89e04856d5ff8244c29e91325271d6a57e33eec99242b121308c477facd169",
  "pliego.css.findings.json": "b59823b27e3f045625bcc9a8ecc3842de039ae1d70d2c98348bd0444550abe04",
  "pliego.css.manifest.json": "51bd81698f58ea3c4ab66c6f5c111135523b62e0177f970a6ae40229ac96d247",
  "pliego.css.receipt.json": "f94939da3e2c04c8d373b2fe881e9234b5abcee8bbb81b6b0d7e1a9c3fdb4fce",
  "pliego.tokens.json": "bb525a903714688d1ada713bff7a0c831631ee1c08b10434d1d03c79d742d204",
};
const expectedWatchControlHashes = {
  "app.css": "d208d2960bd28fb29354b1a89c74c106686c8a8a2d71a9cc7cc9971eb2e97070",
  "app.css.map": "39a03b016583b0827ec8bc92a2a1677dcb4d151324dcdd9c77d303af29b1dddf",
  "app.manifest.json": "3a89e04856d5ff8244c29e91325271d6a57e33eec99242b121308c477facd169",
  "pliego.css.findings.json": "3df2586a1041d39a0e3e66db4e6674df39479157b3ac413c0fea8b7f85ad23bd",
  "pliego.css.manifest.json": "b0713b0a3957f267e960ec026088441f4a4e3d476e52becd30c1cfe08af0aa64",
  "pliego.css.receipt.json": "fe5633bfa90a7edb953e390e5185597fd909d5f9b696059f36dd1e2c0ed6d2e9",
  "pliego.tokens.json": "bb525a903714688d1ada713bff7a0c831631ee1c08b10434d1d03c79d742d204",
};
const expectedDtcgControlHashes = {
  "app.css": "cec35891bc4d167bb359b57cf5fcda10a06e2b0665398424ff63b8e01c8b4850",
  "app.css.map": "ce30a183a990c8dca861721b4c5ea953884fd4fb4898220023192e20abc62c3c",
  "app.manifest.json": "88597038d0aab327fc11d11a4371b03e1aa53b44199cc137eb84729f3eddd00a",
  "pliego.css.findings.json": "2135fdb346d08d7e25516006a9d7829780d155b8e1e2374f19ddd7a08a7916c9",
  "pliego.css.manifest.json": "9fb936c7c415af5c78a3edd298e31e6a4d750a84374e830961b2472117ac3e97",
  "pliego.css.receipt.json": "b24d23b286df8c35711684c585962a2b25d84bf1c6de84b7c618c08101738183",
  "pliego.tokens.json": "7a0862dcd2cd8152cface3ea82fbccf1877e75c6f505efa8a3032f8652f354f0",
};
const expectedDtcgResolverSha256 =
  "5965f868707ad92b0164788b5a7b8b8a2234ce03d7e6c71e1df66d612801b876";
const expectedDtcgConfigHashes = {
  darkLight: "sha256:e06966ae5f001ba6b1b29bfa4d52a3c3d26f9c4ba588d8ff558bd820c23767a9",
  darkDark: "sha256:d0ab9d7cdc46d564adc14142be10d15b14dee36066966a29f4857e22a0f3616c",
};

function fail(message) {
  throw new Error(message);
}

function runCli(cliArguments, cwd, expectedStatus = 0) {
  const result = spawnSync(executable, cliArguments, {
    cwd,
    encoding: "utf8",
    timeout: 30_000,
    windowsHide: true,
  });
  if (result.error) throw result.error;
  if (result.status !== expectedStatus) {
    fail(
      `${executable} ${cliArguments.join(" ")} exited ${result.status}, expected ${expectedStatus}\n` +
        `stdout:\n${result.stdout}\nstderr:\n${result.stderr}`,
    );
  }
  if (expectedStatus === 0 && result.stderr !== "") {
    fail(`successful command emitted stderr:\n${result.stderr}`);
  }
  return result;
}

async function runWatchUntilPublished(cliArguments, cwd, receipt) {
  const child = spawn(executable, cliArguments, {
    cwd,
    windowsHide: true,
    stdio: ["ignore", "pipe", "pipe"],
  });
  let stdout = "";
  let stderr = "";
  let exited = false;
  child.stdout.setEncoding("utf8");
  child.stderr.setEncoding("utf8");
  child.stdout.on("data", (chunk) => (stdout += chunk));
  child.stderr.on("data", (chunk) => (stderr += chunk));
  child.once("exit", () => (exited = true));
  try {
    const deadline = Date.now() + 30_000;
    while (!existsSync(receipt)) {
      if (exited) fail(`watch exited before publication\nstdout:\n${stdout}\nstderr:\n${stderr}`);
      if (Date.now() >= deadline) fail(`watch publication timed out\nstderr:\n${stderr}`);
      await new Promise((resolveDelay) => setTimeout(resolveDelay, 25));
    }
  } finally {
    if (!exited) child.kill();
    await Promise.race([
      new Promise((resolveClose) => child.once("close", resolveClose)),
      new Promise((resolveDelay) => setTimeout(resolveDelay, 5_000)),
    ]);
  }
  if (stdout !== "") fail(`watch emitted unexpected stdout:\n${stdout}`);
  if (!stderr.includes("watching `src`")) fail(`watch lost its startup event:\n${stderr}`);
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function outputSnapshot(directory) {
  return readdirSync(directory)
    .sort()
    .map((name) => {
      const path = join(directory, name);
      const metadata = statSync(path);
      return [name, metadata.size, metadata.mtimeMs, metadata.ino, readFileSync(path).toString("base64")];
    });
}

function publishedFiles(directory) {
  return readdirSync(directory)
    .filter((name) => !name.endsWith(".pliego.lock"))
    .sort();
}

function treeSnapshot(directory, prefix = "") {
  const snapshot = [];
  for (const entry of readdirSync(directory, { withFileTypes: true }).sort((left, right) =>
    left.name < right.name ? -1 : left.name > right.name ? 1 : 0,
  )) {
    const relative = prefix === "" ? entry.name : `${prefix}/${entry.name}`;
    const path = join(directory, entry.name);
    if (entry.isSymbolicLink()) {
      snapshot.push(["link", relative, readlinkSync(path)]);
    } else if (entry.isDirectory()) {
      snapshot.push(["directory", relative]);
      snapshot.push(...treeSnapshot(path, relative));
    } else {
      snapshot.push(["file", relative, readFileSync(path).toString("base64")]);
    }
  }
  return snapshot;
}

function assertEqual(actual, expected, message) {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    fail(`${message}\nactual: ${JSON.stringify(actual)}\nexpected: ${JSON.stringify(expected)}`);
  }
}

function assertFrozenHashes(actual, expected, label) {
  const mismatches = Object.keys(expected).filter((name) => actual[name] !== expected[name]);
  if (mismatches.length > 0) {
    fail(
      `${label} frozen hashes drifted for ${mismatches.join(", ")}:\n${JSON.stringify(actual, null, 2)}`,
    );
  }
}

function assertTokenGraphProjection(directory, cssFile, manifestFile, label) {
  const graphBytes = readFileSync(join(directory, "pliego.tokens.json"));
  const graph = JSON.parse(graphBytes);
  const manifest = JSON.parse(readFileSync(join(directory, "pliego.css.manifest.json"), "utf8"));
  const receipt = JSON.parse(readFileSync(join(directory, "pliego.css.receipt.json"), "utf8"));
  const tokens = manifest.tokens;
  if (
    tokens.observation !== "measured" ||
    tokens.graphVersion !== "pliegocss-token-graph/1" ||
    !/^sha256:[0-9a-f]{64}$/.test(tokens.graphHash) ||
    !Number.isInteger(tokens.tokens) ||
    tokens.tokens <= 0 ||
    tokens.themes !== 1 ||
    !Number.isInteger(tokens.coverageBasisPoints) ||
    tokens.coverageBasisPoints <= 0 ||
    tokens.coverageBasisPoints > 10_000 ||
    tokens.aliases !== 0 ||
    tokens.derivedValues !== 0 ||
    tokens.cycles !== 0 ||
    tokens.contrastPairs !== 0 ||
    tokens.deprecations !== 0
  ) {
    fail(`${label} lost its bounded token projection: ${JSON.stringify(tokens)}`);
  }
  if (
    graph.schemaVersion !== 1 ||
    graph.graphVersion !== "pliegocss-token-graph/1" ||
    !Array.isArray(graph.sources) ||
    graph.sources.length !== tokens.tokens ||
    !Array.isArray(graph.themes) ||
    graph.themes.length !== tokens.themes ||
    graph.themes[0]?.name !== "default" ||
    Object.keys(graph.themes[0]?.selections ?? {}).length !== 0 ||
    !Array.isArray(graph.themes[0]?.tokens) ||
    graph.themes[0].tokens.length !== tokens.tokens ||
    `${JSON.stringify(graph)}\n` !== graphBytes.toString("utf8") ||
    tokens.graphHash !== `sha256:${sha256(graphBytes)}`
  ) {
    fail(`${label} emitted an invalid or non-canonical token graph`);
  }
  const graphOutput = manifest.outputs.find((output) => output.role === "token-graph");
  const cssOutput = manifest.outputs.find((output) => output.role === "generated-css");
  const manifestOutput = manifest.outputs.find((output) => output.role === "style-manifest");
  const graphCheck = receipt.checks.find((check) => check.id === "token-graph-integrity");
  if (
    graphOutput?.artifact?.file !== "pliego.tokens.json" ||
    graphOutput.artifact.bytes !== graphBytes.length ||
    graphOutput.artifact.sha256 !== tokens.graphHash ||
    graphOutput.mediaType !== "application/json" ||
    graphOutput.sourceMap != null ||
    JSON.stringify(graphOutput.relationships) !== JSON.stringify([cssFile, manifestFile]) ||
    !cssOutput?.relationships.includes("pliego.tokens.json") ||
    !manifestOutput?.relationships.includes("pliego.tokens.json") ||
    !manifest.receipt.requiredChecks.includes("token-graph-integrity") ||
    graphCheck?.status !== "passed" ||
    graphCheck.evidenceKind !== "integrity" ||
    graphCheck.required !== true ||
    JSON.stringify(graphCheck.evidenceArtifact) !== JSON.stringify(graphOutput.artifact)
  ) {
    fail(`${label} did not integrity-bind its canonical token graph`);
  }
}

function assertDtcgTokenGraphProjection(directory, cssFile, manifestFile, label) {
  const graphBytes = readFileSync(join(directory, "pliego.tokens.json"));
  const graph = JSON.parse(graphBytes);
  const manifestBytes = readFileSync(join(directory, "pliego.css.manifest.json"));
  const manifest = JSON.parse(manifestBytes);
  const receipt = JSON.parse(readFileSync(join(directory, "pliego.css.receipt.json"), "utf8"));
  const tokens = manifest.tokens;
  const expectedSelections = [
    { appearance: "dark", channel: "dark" },
    { appearance: "dark", channel: "light" },
    { appearance: "light", channel: "dark" },
    { appearance: "light", channel: "light" },
  ];
  if (
    graph.schemaVersion !== 1 ||
    graph.graphVersion !== "pliegocss-token-graph/1" ||
    graph.adapter?.name !== "dtcg-resolver" ||
    graph.adapter?.version !== "2025.10/same-document-1" ||
    !/^sha256:[0-9a-f]{64}$/.test(graph.adapter?.sourceHash ?? "") ||
    !Array.isArray(graph.sources) ||
    graph.sources.length !== 12 ||
    !Array.isArray(graph.themes) ||
    graph.themes.length !== 4 ||
    JSON.stringify(graph.themes.map((theme) => theme.selections)) !==
      JSON.stringify(expectedSelections) ||
    !graph.themes.every(
      (theme) =>
        typeof theme.themeId === "string" &&
        theme.themeId.length === 32 &&
        Array.isArray(theme.tokens) &&
        theme.tokens.length === 53,
    ) ||
    JSON.stringify(graph.dtcgInventory) !== JSON.stringify({ color: 3 }) ||
    `${JSON.stringify(graph)}\n` !== graphBytes.toString("utf8") ||
    sha256(graphBytes) !== expectedDtcgControlHashes["pliego.tokens.json"]
  ) {
    fail(`${label} did not publish the complete canonical Resolver 2025.10 graph`);
  }
  const resolverInput = manifest.inputs.files.find((input) => input.role === "token-resolver");
  const adapter = manifest.inputs.adapters.find((identity) => identity.name === "dtcg-resolver");
  if (
    resolverInput?.file !== "examples/product.resolver.json" ||
    resolverInput.sha256 !== `sha256:${expectedDtcgResolverSha256}` ||
    adapter?.version !== "2025.10/same-document-1" ||
    tokens.observation !== "measured" ||
    tokens.graphVersion !== graph.graphVersion ||
    tokens.graphHash !== `sha256:${expectedDtcgControlHashes["pliego.tokens.json"]}` ||
    tokens.tokens !== 53 ||
    tokens.aliases !== 1 ||
    tokens.themes !== 4 ||
    tokens.deprecations !== 1 ||
    JSON.stringify(tokens.dtcgAdapter) !== JSON.stringify(adapter) ||
    JSON.stringify(tokens.dtcgInventory) !== JSON.stringify({ color: 3 })
  ) {
    fail(`${label} lost its exact resolver input or DTCG measurement identity`);
  }
  const graphOutput = manifest.outputs.find((output) => output.role === "token-graph");
  const cssOutput = manifest.outputs.find((output) => output.role === "generated-css");
  const manifestOutput = manifest.outputs.find((output) => output.role === "style-manifest");
  const graphCheck = receipt.checks.find((check) => check.id === "token-graph-integrity");
  if (
    graphOutput?.artifact?.file !== "pliego.tokens.json" ||
    graphOutput.artifact.bytes !== graphBytes.length ||
    graphOutput.artifact.sha256 !== tokens.graphHash ||
    graphOutput.mediaType !== "application/json" ||
    graphOutput.sourceMap != null ||
    JSON.stringify(graphOutput.relationships) !== JSON.stringify([cssFile, manifestFile]) ||
    !cssOutput?.relationships.includes("pliego.tokens.json") ||
    !manifestOutput?.relationships.includes("pliego.tokens.json") ||
    !manifest.receipt.requiredChecks.includes("token-graph-integrity") ||
    graphCheck?.status !== "passed" ||
    graphCheck.evidenceKind !== "integrity" ||
    graphCheck.required !== true ||
    JSON.stringify(graphCheck.evidenceArtifact) !== JSON.stringify(graphOutput.artifact)
  ) {
    fail(`${label} did not integrity-bind its complete DTCG token graph`);
  }
}

function assertControlArtifactDriftReadOnly(arguments_, cwd, directory, artifact, label) {
  const path = join(directory, artifact);
  const original = readFileSync(path);
  writeFileSync(path, Buffer.concat([original, Buffer.from("drift\n")]));
  const drifted = outputSnapshot(directory);
  const result = runCli([...arguments_, "--check"], cwd, 1);
  if (!result.stderr.includes("drift")) {
    fail(`${label} drift failure lost its diagnostic: ${result.stderr}`);
  }
  assertEqual(outputSnapshot(directory), drifted, `${label} drift check mutated outputs`);
  runCli(arguments_, cwd);
}

function assertSourceMapProjection(directory, cssFile, expectedSource, label) {
  const mapFile = `${cssFile}.map`;
  const css = readFileSync(join(directory, cssFile), "utf8");
  const mapBytes = readFileSync(join(directory, mapFile));
  const map = JSON.parse(mapBytes);
  if (
    map.version !== 3 ||
    JSON.stringify(Object.keys(map)) !== JSON.stringify(["version", "sources", "names", "mappings"]) ||
    !Array.isArray(map.sources) ||
    !map.sources.includes(expectedSource) ||
    !Array.isArray(map.names) ||
    map.names.length !== 0 ||
    typeof map.mappings !== "string" ||
    map.mappings.length === 0 ||
    css.includes("sourceMappingURL")
  ) {
    fail(`${label} emitted an invalid or unstable Source Map v3: ${JSON.stringify(map)}`);
  }
  const manifest = JSON.parse(readFileSync(join(directory, "pliego.css.manifest.json"), "utf8"));
  const cssOutput = manifest.outputs.find((output) => output.artifact.file === cssFile);
  const mapOutput = manifest.outputs.find((output) => output.artifact.file === mapFile);
  if (
    cssOutput?.sourceMap?.file !== mapFile ||
    cssOutput.sourceMap.bytes !== mapBytes.length ||
    cssOutput.sourceMap.sha256 !== `sha256:${sha256(mapBytes)}` ||
    JSON.stringify(cssOutput.sourceMap) !== JSON.stringify(mapOutput?.artifact) ||
    mapOutput?.role !== "css-source-map" ||
    mapOutput.mediaType !== "application/json" ||
    !mapOutput.relationships.includes(cssFile)
  ) {
    fail(`${label} did not integrity-bind its CSS source map`);
  }
}

function bundleArguments(plan, outputDir, check = false) {
  return ["bundle", "--plan", plan, "--output-dir", outputDir, ...(check ? ["--check"] : [])];
}

function directDtcgArguments(outputDir, inputs) {
  const arguments_ = [
    "compile",
    "--style",
    "flex bg-brand",
    "--tokens",
    "examples/product.resolver.json",
  ];
  for (const [modifier, context] of inputs) {
    arguments_.push("--token-input", `${modifier}=${context}`);
  }
  arguments_.push(
    "--theme",
    "--targets",
    "modern",
    "--output",
    join(outputDir, "app.css"),
    "--manifest",
    join(outputDir, "app.manifest.json"),
    "--control-dir",
    outputDir,
  );
  return arguments_;
}

process.once("exit", () => rmSync(runtime, { recursive: true, force: true }));

rmSync(runtime, { recursive: true, force: true });
mkdirSync(join(project, "src"), { recursive: true });
mkdirSync(foreignCwd, { recursive: true });

const portableSource =
  "fn portable_view() {\r\n    let _marker = \"🦀\";\r\n    let _ = pc!(\"flex gap-4\");\r\n}\r\n";
writeFileSync(join(project, "src", sourceName), portableSource);
const plan = join(project, "pliego.bundles.toml");
writeFileSync(
  plan,
  `schema = 1\ntargets = "modern"\nformat = "minified"\n\n[theme]\nkind = "seed"\n\n[bundles.app]\nsources = [${JSON.stringify(logicalSource)}]\nemit-theme = true\n`,
);
const macroText = 'pc!("flex gap-4")';
const macroStart = Buffer.byteLength(portableSource.slice(0, portableSource.indexOf(macroText)));
const reachability = join(project, "pliego.reachability.json");
writeFileSync(
  reachability,
  `${JSON.stringify(
    {
      schema: 1,
      applicationCoverage: "complete",
      components: [
        {
          id: "app::portable-view",
          sites: [
            {
              file: logicalSource,
              byteStart: macroStart,
              byteEnd: macroStart + Buffer.byteLength(macroText),
            },
          ],
        },
      ],
      routes: [{ id: "home", path: "/", components: ["app::portable-view"] }],
      islands: [],
    },
    null,
    2,
  )}\n`,
);

const firstOutput = join(runtime, "output-root-cwd");
const secondOutput = join(runtime, "output-foreign-cwd");
mkdirSync(firstOutput);
mkdirSync(secondOutput);
runCli(bundleArguments(plan, firstOutput), root);
runCli(bundleArguments(plan, secondOutput), foreignCwd);

const cssName = "app.css";
const manifestName = "app.manifest.json";
const firstCss = readFileSync(join(firstOutput, cssName));
const secondCss = readFileSync(join(secondOutput, cssName));
const firstManifestBytes = readFileSync(join(firstOutput, manifestName));
const secondManifestBytes = readFileSync(join(secondOutput, manifestName));
assertEqual(firstCss, secondCss, "CSS bytes changed with process CWD");
assertEqual(firstManifestBytes, secondManifestBytes, "manifest bytes changed with process CWD");

const manifest = JSON.parse(firstManifestBytes);
if (
  manifest.schemaVersion !== 3 ||
  manifest.styleIdFormatVersion !== 2 ||
  manifest.classNameFormatVersion !== 1 ||
  manifest.themeIdFormatVersion !== 1 ||
  manifest.cssBytes !== firstCss.byteLength ||
  manifest.cssSha256 !== sha256(firstCss) ||
  manifest.styles.length !== 1
) {
  fail("bundle manifest integrity contract drifted");
}
if (manifest.cssSha256 !== expectedCssSha256 || sha256(firstManifestBytes) !== expectedManifestSha256) {
  fail(
    `frozen cross-platform hashes drifted: CSS ${manifest.cssSha256}, manifest ${sha256(firstManifestBytes)}`,
  );
}
if (
  !manifest.styles[0].origins.every(
    (origin) => origin.file === logicalSource && !origin.file.includes("\\"),
  )
) {
  fail("bundle provenance is not stable plan-relative UTF-8 with forward slashes");
}

const controlOutput = join(runtime, "audit-control");
mkdirSync(controlOutput);
const controlArguments = [
  "audit",
  "--input",
  "examples/audit/app.css",
  "--targets",
  "none",
  "--budget-policy",
  "examples/audit/pliego.budgets.json",
  "--budget-subject",
  "package=example-app",
  "--budget-subject",
  "route=/demo",
  "--control-dir",
  controlOutput,
  "--format",
  "json",
];
runCli(controlArguments, root);
for (const [name, expected] of Object.entries(expectedControlHashes)) {
  const actual = sha256(readFileSync(join(controlOutput, name)));
  if (actual !== expected) {
    fail(`frozen cross-platform control hash drifted for ${name}: ${actual}`);
  }
}
const beforeControlCheck = outputSnapshot(controlOutput);
runCli([...controlArguments, "--check"], root);
assertEqual(
  outputSnapshot(controlOutput),
  beforeControlCheck,
  "control --check changed the frozen output group",
);
const bundleControlOutput = join(runtime, "bundle-control");
mkdirSync(bundleControlOutput);
const bundleControlArguments = [
  "bundle",
  "--plan",
  plan,
  "--output-dir",
  bundleControlOutput,
  "--manifest-version",
  "5",
  "--reachability",
  reachability,
  "--asset-plan",
  "--project-index",
  "--control",
];
runCli(bundleControlArguments, root);
const bundleControlFiles = [
  "app.css",
  "app.css.map",
  "app.manifest.json",
  "pliego.assets.json",
  "pliego.css.findings.json",
  "pliego.css.manifest.json",
  "pliego.css.receipt.json",
  "pliego.index.json",
  "pliego.tokens.json",
];
const bundleControlHashes = Object.fromEntries(
  bundleControlFiles.map((name) => [name, sha256(readFileSync(join(bundleControlOutput, name)))]),
);
assertFrozenHashes(bundleControlHashes, expectedBundleControlHashes, "bundle control");
assertEqual(
  publishedFiles(bundleControlOutput),
  bundleControlFiles,
  "bundle control must publish exactly nine artifacts",
);
assertTokenGraphProjection(
  bundleControlOutput,
  "app.css",
  "app.manifest.json",
  "bundle control",
);
assertSourceMapProjection(bundleControlOutput, "app.css", logicalSource, "bundle control");
const beforeBundleControlCheck = outputSnapshot(bundleControlOutput);
runCli([...bundleControlArguments, "--check"], root);
assertEqual(
  outputSnapshot(bundleControlOutput),
  beforeBundleControlCheck,
  "bundle control --check changed the frozen output group",
);
assertControlArtifactDriftReadOnly(
  bundleControlArguments,
  root,
  bundleControlOutput,
  "pliego.tokens.json",
  "bundle token graph",
);
const assetPlanControlOutput = join(runtime, "asset-plan-control");
mkdirSync(assetPlanControlOutput);
const assetPlanControlArguments = [
  "audit",
  "--asset-plan",
  "bundle-control/pliego.assets.json",
  "--targets",
  "modern",
  "--control-dir",
  "asset-plan-control",
  "--format",
  "json",
];
runCli(assetPlanControlArguments, runtime);
const assetPlanControlFiles = [
  "pliego.css.findings.json",
  "pliego.css.manifest.json",
  "pliego.css.receipt.json",
];
const assetPlanControlHashes = Object.fromEntries(
  assetPlanControlFiles.map((name) => [name, sha256(readFileSync(join(assetPlanControlOutput, name)))]),
);
for (const [name, expected] of Object.entries(expectedAssetPlanControlHashes)) {
  if (assetPlanControlHashes[name] !== expected) {
    fail(`frozen Asset Plan control hash drifted for ${name}: ${assetPlanControlHashes[name]}`);
  }
}
const reopenedAssetPlanTokens = JSON.parse(
  readFileSync(join(assetPlanControlOutput, "pliego.css.manifest.json"), "utf8"),
).tokens;
if (reopenedAssetPlanTokens.observation !== "unavailable" || !reopenedAssetPlanTokens.unavailableReason) {
  fail(`reopened Asset Plan invented token evidence: ${JSON.stringify(reopenedAssetPlanTokens)}`);
}
const beforeAssetPlanControlCheck = outputSnapshot(assetPlanControlOutput);
runCli([...assetPlanControlArguments, "--check"], runtime);
assertEqual(
  outputSnapshot(assetPlanControlOutput),
  beforeAssetPlanControlCheck,
  "Asset Plan control --check changed the frozen output group",
);
const compileControlOutput = join(project, "compile-control");
mkdirSync(compileControlOutput);
const compileControlArguments = [
  "compile",
  "--source",
  "src",
  "--seed",
  "--theme",
  "--targets",
  "modern",
  "--output",
  "compile-control/app.css",
  "--manifest",
  "compile-control/app.manifest.json",
  "--control-dir",
  "compile-control",
];
runCli(compileControlArguments, project);
const compileControlFiles = [
  "app.css",
  "app.css.map",
  "app.manifest.json",
  "pliego.css.findings.json",
  "pliego.css.manifest.json",
  "pliego.css.receipt.json",
  "pliego.tokens.json",
];
const compileControlHashes = Object.fromEntries(
  compileControlFiles.map((name) => [name, sha256(readFileSync(join(compileControlOutput, name)))]),
);
assertFrozenHashes(compileControlHashes, expectedCompileControlHashes, "compile control");
assertEqual(
  publishedFiles(compileControlOutput),
  compileControlFiles,
  "compile control must publish exactly seven artifacts",
);
assertTokenGraphProjection(
  compileControlOutput,
  "app.css",
  "app.manifest.json",
  "compile control",
);
assertSourceMapProjection(compileControlOutput, "app.css", logicalSource, "compile control");
const beforeCompileControlCheck = outputSnapshot(compileControlOutput);
runCli([...compileControlArguments, "--check"], project);
assertEqual(
  outputSnapshot(compileControlOutput),
  beforeCompileControlCheck,
  "compile control --check changed the frozen output group",
);
assertControlArtifactDriftReadOnly(
  compileControlArguments,
  project,
  compileControlOutput,
  "pliego.tokens.json",
  "compile token graph",
);
const watchControlOutput = join(project, "watch-control");
mkdirSync(watchControlOutput);
const watchControlArguments = [
  "watch",
  "--source",
  "src",
  "--seed",
  "--theme",
  "--targets",
  "modern",
  "--output",
  "watch-control/app.css",
  "--manifest",
  "watch-control/app.manifest.json",
  "--control-dir",
  "watch-control",
];
await runWatchUntilPublished(
  watchControlArguments,
  project,
  join(watchControlOutput, "pliego.css.receipt.json"),
);
const watchControlFiles = [
  "app.css",
  "app.css.map",
  "app.manifest.json",
  "pliego.css.findings.json",
  "pliego.css.manifest.json",
  "pliego.css.receipt.json",
  "pliego.tokens.json",
];
const watchControlHashes = Object.fromEntries(
  watchControlFiles.map((name) => [name, sha256(readFileSync(join(watchControlOutput, name)))]),
);
assertFrozenHashes(watchControlHashes, expectedWatchControlHashes, "watch control");
assertEqual(
  publishedFiles(watchControlOutput),
  watchControlFiles,
  "watch control must publish exactly seven artifacts",
);
assertTokenGraphProjection(
  watchControlOutput,
  "app.css",
  "app.manifest.json",
  "watch control",
);
assertSourceMapProjection(watchControlOutput, "app.css", logicalSource, "watch control");
const dtcgControlOutput = join(runtime, "dtcg-control");
mkdirSync(dtcgControlOutput);
const dtcgControlArguments = directDtcgArguments(dtcgControlOutput, [
  ["appearance", "dark"],
  ["channel", "light"],
]);
runCli(dtcgControlArguments, root);
const dtcgControlFiles = [
  "app.css",
  "app.css.map",
  "app.manifest.json",
  "pliego.css.findings.json",
  "pliego.css.manifest.json",
  "pliego.css.receipt.json",
  "pliego.tokens.json",
];
const dtcgControlHashes = Object.fromEntries(
  dtcgControlFiles.map((name) => [name, sha256(readFileSync(join(dtcgControlOutput, name)))]),
);
assertFrozenHashes(dtcgControlHashes, expectedDtcgControlHashes, "direct DTCG control");
assertEqual(
  publishedFiles(dtcgControlOutput),
  dtcgControlFiles,
  "direct DTCG control must publish exactly seven artifacts",
);
assertDtcgTokenGraphProjection(
  dtcgControlOutput,
  "app.css",
  "app.manifest.json",
  "direct DTCG control",
);
assertSourceMapProjection(
  dtcgControlOutput,
  "app.css",
  "pliego.cli-input.json",
  "direct DTCG control",
);
const dtcgControlManifest = JSON.parse(
  readFileSync(join(dtcgControlOutput, "pliego.css.manifest.json"), "utf8"),
);
const dtcgStyleManifest = JSON.parse(
  readFileSync(join(dtcgControlOutput, "app.manifest.json"), "utf8"),
);
const dtcgCss = readFileSync(join(dtcgControlOutput, "app.css"));
if (
  dtcgControlManifest.inputs.configHash !== expectedDtcgConfigHashes.darkLight ||
  dtcgStyleManifest.themeId !== "8be8798dad4dc86f1a8e58a4f4b493eb" ||
  !dtcgCss.toString("utf8").includes("--color-brand:color(srgb .1 .1 .1)")
) {
  fail("direct DTCG control lost its exact canonical dark/light selection identity");
}
const canonicalDtcgSnapshot = treeSnapshot(dtcgControlOutput);
const canonicalDtcgArguments = directDtcgArguments(dtcgControlOutput, [
  ["CHANNEL", "LiGhT"],
  ["APPEARANCE", "DaRk"],
]);
runCli(canonicalDtcgArguments, root);
assertEqual(
  treeSnapshot(dtcgControlOutput),
  canonicalDtcgSnapshot,
  "case- and order-equivalent DTCG selections did not converge byte for byte",
);
const beforeDtcgControlCheck = outputSnapshot(dtcgControlOutput);
runCli([...dtcgControlArguments, "--check"], root);
assertEqual(
  outputSnapshot(dtcgControlOutput),
  beforeDtcgControlCheck,
  "direct DTCG control --check changed the frozen output group",
);

const dtcgIdentityOutput = join(runtime, "dtcg-identity-control");
mkdirSync(dtcgIdentityOutput);
const dtcgIdentityArguments = directDtcgArguments(dtcgIdentityOutput, [
  ["appearance", "dark"],
  ["channel", "dark"],
]);
runCli(dtcgIdentityArguments, root);
assertDtcgTokenGraphProjection(
  dtcgIdentityOutput,
  "app.css",
  "app.manifest.json",
  "DTCG config identity witness",
);
const dtcgIdentityManifest = JSON.parse(
  readFileSync(join(dtcgIdentityOutput, "pliego.css.manifest.json"), "utf8"),
);
if (
  dtcgIdentityManifest.inputs.configHash !== expectedDtcgConfigHashes.darkDark ||
  dtcgIdentityManifest.inputs.configHash === dtcgControlManifest.inputs.configHash ||
  !readFileSync(join(dtcgIdentityOutput, "app.css")).equals(dtcgCss) ||
  !readFileSync(join(dtcgIdentityOutput, "app.manifest.json")).equals(
    readFileSync(join(dtcgControlOutput, "app.manifest.json")),
  ) ||
  !readFileSync(join(dtcgIdentityOutput, "pliego.tokens.json")).equals(
    readFileSync(join(dtcgControlOutput, "pliego.tokens.json")),
  )
) {
  fail("canonical DTCG selections stopped participating in configHash independently of ThemeId");
}
const sourceBytes = readFileSync(join(project, "src", sourceName));
for (const origin of manifest.styles[0].origins) {
  if (
    !Number.isInteger(origin.byteStart) ||
    !Number.isInteger(origin.byteEnd) ||
    sourceBytes.subarray(origin.byteStart, origin.byteEnd).toString("utf8") !== 'pc!("flex gap-4")'
  ) {
    fail(`manifest origin does not select the exact macro bytes: ${JSON.stringify(origin)}`);
  }
}

const checkOutput = join(runtime, "output-check");
mkdirSync(checkOutput);
copyFileSync(join(firstOutput, cssName), join(checkOutput, cssName));
copyFileSync(join(firstOutput, manifestName), join(checkOutput, manifestName));
const beforeCheck = outputSnapshot(checkOutput);
runCli(bundleArguments(plan, checkOutput, true), foreignCwd);
assertEqual(outputSnapshot(checkOutput), beforeCheck, "successful --check wrote to the output directory");

const emptyCheckOutput = join(runtime, "output-empty-check");
mkdirSync(emptyCheckOutput);
const emptyCheck = runCli(bundleArguments(plan, emptyCheckOutput, true), foreignCwd, 1);
if (!emptyCheck.stderr.includes("bundle output drift detected")) {
  fail(`empty --check failure lost its diagnostic: ${emptyCheck.stderr}`);
}
assertEqual(readdirSync(emptyCheckOutput), [], "failed --check created output or lock files");

writeFileSync(join(checkOutput, cssName), Buffer.concat([firstCss, Buffer.from("/* drift */\n")]));
const beforeDriftCheck = outputSnapshot(checkOutput);
const drift = runCli(bundleArguments(plan, checkOutput, true), foreignCwd, 1);
if (!drift.stderr.includes("bundle output drift detected")) {
  fail(`drift failure lost its diagnostic: ${drift.stderr}`);
}
assertEqual(outputSnapshot(checkOutput), beforeDriftCheck, "failed --check mutated drifted outputs");

const outside = join(runtime, "outside-plan");
const escape = join(project, "escape");
const escapeOutput = join(runtime, "output-escape");
mkdirSync(outside);
mkdirSync(escapeOutput);
writeFileSync(join(outside, "leak.rs"), "fn leak() { let _ = pc!(\"grid\"); }\n");
symlinkSync(outside, escape, process.platform === "win32" ? "junction" : "dir");
const escapePlan = join(project, "escape.bundles.toml");
writeFileSync(
  escapePlan,
  'schema=1\ntargets="modern"\nformat="minified"\n[theme]\nkind="seed"\n[bundles.escape]\nsources=["escape/leak.rs"]\n',
);
const escaped = runCli(bundleArguments(escapePlan, escapeOutput), root, 1);
if (!/symbolic link|reparse|outside bundle plan directory/.test(escaped.stderr)) {
  fail(`link escape did not fail closed: ${escaped.stderr}`);
}
assertEqual(readdirSync(escapeOutput), [], "link escape published output or lock files");

const linkedOutputTarget = join(runtime, "linked-output-target");
const linkedOutput = join(runtime, "linked-output");
mkdirSync(linkedOutputTarget);
symlinkSync(linkedOutputTarget, linkedOutput, process.platform === "win32" ? "junction" : "dir");
const linkedDestination = runCli(bundleArguments(plan, linkedOutput), root, 1);
if (!/symbolic link|reparse/.test(linkedDestination.stderr)) {
  fail(`linked output directory did not fail closed: ${linkedDestination.stderr}`);
}
assertEqual(readdirSync(linkedOutputTarget), [], "linked output directory received artifacts");

const outsideTheme = join(runtime, "outside-theme");
const linkedTheme = join(project, "linked-theme");
const linkedThemeOutput = join(runtime, "output-linked-theme");
mkdirSync(outsideTheme);
mkdirSync(linkedThemeOutput);
writeFileSync(join(outsideTheme, "theme.toml"), 'schema=1\nextends="seed"\n');
symlinkSync(outsideTheme, linkedTheme, process.platform === "win32" ? "junction" : "dir");
const linkedThemePlan = join(project, "linked-theme.bundles.toml");
writeFileSync(
  linkedThemePlan,
  `schema=1\ntargets="modern"\nformat="minified"\n[theme]\nkind="config"\npath="linked-theme/theme.toml"\n[bundles.app]\nsources=[${JSON.stringify(logicalSource)}]\n`,
);
const linkedConfiguration = runCli(
  bundleArguments(linkedThemePlan, linkedThemeOutput),
  root,
  1,
);
if (!/symbolic link|reparse|outside bundle plan directory/.test(linkedConfiguration.stderr)) {
  fail(`linked theme configuration did not fail closed: ${linkedConfiguration.stderr}`);
}
assertEqual(readdirSync(linkedThemeOutput), [], "linked theme path published output or lock files");

const dotOutput = join(runtime, "output-dot-segment");
mkdirSync(dotOutput);
const dotPlan = join(project, "dot.bundles.toml");
writeFileSync(
  dotPlan,
  `schema=1\ntargets="modern"\nformat="minified"\n[theme]\nkind="seed"\n[bundles.dot]\nsources=["src/./${sourceName}"]\n`,
);
const dotted = runCli(bundleArguments(dotPlan, dotOutput), root, 1);
if (!dotted.stderr.includes("without `.` or `..` components")) {
  fail(`dot-segment path did not fail closed: ${dotted.stderr}`);
}
assertEqual(readdirSync(dotOutput), [], "invalid dot path published output or lock files");

const configAlias = join(project, "APP.CSS");
const configBytes = 'schema=1\nextends="seed"\n';
writeFileSync(configAlias, configBytes);
const casePlan = join(project, "case.bundles.toml");
writeFileSync(
  casePlan,
  `schema=1\ntargets="modern"\nformat="minified"\n[theme]\nkind="config"\npath="APP.CSS"\n[bundles.app]\nsources=[${JSON.stringify(logicalSource)}]\n`,
);
const beforeAlias = treeSnapshot(project);
const aliased = runCli(bundleArguments(casePlan, project), root, 1);
if (!aliased.stderr.includes("aliases input")) {
  fail(`case-insensitive input/output alias was not rejected: ${aliased.stderr}`);
}
if (readFileSync(configAlias, "utf8") !== configBytes) {
  fail("case-insensitive alias check modified the theme configuration");
}
assertEqual(treeSnapshot(project), beforeAlias, "case-insensitive alias failure mutated project files");
const caseAlias = "rejected";

const report = {
  schemaVersion: 1,
  platform: process.platform,
  architecture: process.arch,
  node: process.version,
  cssSha256: manifest.cssSha256,
  cssBytes: manifest.cssBytes,
  manifestSha256: sha256(firstManifestBytes),
  controlHashes: expectedControlHashes,
  controlCheckReadOnly: true,
  bundleControlHashes,
  bundleControlCheckReadOnly: true,
  assetPlanControlHashes,
  assetPlanControlCheckReadOnly: true,
  compileControlHashes,
  compileControlCheckReadOnly: true,
  watchControlHashes,
  dtcgControlHashes,
  dtcgControlCheckReadOnly: true,
  dtcgResolverSha256: expectedDtcgResolverSha256,
  dtcgConfigHashes: expectedDtcgConfigHashes,
  dtcgCanonicalSelection: { appearance: "dark", channel: "light" },
  dtcgSelectionIdentityIndependentOfThemeId: true,
  provenance: logicalSource,
  cwdInvariant: true,
  unicodeAndCrlf: true,
  checkReadOnly: true,
  linkEscape: "rejected",
  linkedOutput: "rejected",
  linkedTheme: "rejected",
  dotSegment: "rejected",
  caseAlias,
};
process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
