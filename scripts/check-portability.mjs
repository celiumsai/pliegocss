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
    join(
      root,
      "target",
      "debug",
      process.platform === "win32" ? "pliego-cssc.exe" : "pliego-cssc",
    ),
);
const runtime = resolve(
  process.env.PLIEGO_PORTABILITY_RUNTIME ??
    join(
      root,
      "target",
      "portability contract",
      `run-${process.pid}-${Date.now()}`,
    ),
);
const project = join(runtime, "project");
const foreignCwd = join(runtime, "foreign-cwd");
const sourceName = "café.rs";
const logicalSource = `src/${sourceName}`;
const expectedCssSha256 =
  "65dbbcff74b6886fdd749b1c7c0587eaff06b4f4720405e10716c2b0dfee9bbc";
const expectedManifestSha256 =
  "453f708c9c4edbd388eda1ee9c883212cd5d31421077d123cb1d32bc0b28d4bb";
const expectedControlHashes = {
  "pliego.css.findings.json":
    "a353b52f386f0b896db3c354f45bc0b304dffab1477904a5ee37cd8768be468a",
  "pliego.css.manifest.json":
    "b8513ecd80e9fe566778aa0bfb0b142ecfb06a014bb34a1caa02c1ad28567fd3",
  "pliego.css.receipt.json":
    "81777f9533bb6cd203f68e0f052e5974d09a2d773d7adf8f062f9361100a1c28",
};
const expectedBundleControlHashes = {
  "app.css": "65dbbcff74b6886fdd749b1c7c0587eaff06b4f4720405e10716c2b0dfee9bbc",
  "app.css.map":
    "39a03b016583b0827ec8bc92a2a1677dcb4d151324dcdd9c77d303af29b1dddf",
  "app.manifest.json":
    "0b17997d04739e7698300d9d13278ecd537b2821e7e123db00f83e4a80d3d609",
  "pliego.assets.json":
    "9664a30c61b226d6f81fc771c0042206e9a98a64a77429d7e05498b78f194d89",
  "pliego.css.findings.json":
    "99a2101ad94c358f743b3df1a1214efe8a7716a9006226e43b74141039804e59",
  "pliego.css.manifest.json":
    "e75b481bd2755a70d5c2ac1030729633cb96d28d8107e562b55ae0951913ba12",
  "pliego.css.receipt.json":
    "6fe898d54ad8c745d63d1e6cdd8d31ae6c157130ad4d047542ba6f3cb1f4b5cf",
  "pliego.index.json":
    "b685bbaaa08bfac33f9024f96a8a6c8338771ed4d019fe7c01db602ea40a1864",
  "pliego.tokens.json":
    "3298873e66ade310459593530c86419c05a973d2404b71882c2d0ae430030a28",
};
const expectedAssetPlanControlHashes = {
  "pliego.css.findings.json":
    "bd9fa74558e57f08cc9bdff0bf6f6fa6502c483487451652795353a14dab579d",
  "pliego.css.manifest.json":
    "f254eebbfc716351308f56f34d6494e4317866965cbea6d4bf6b27922f0b09ea",
  "pliego.css.receipt.json":
    "d79e5cca007ee6af62cbafaa36532503860e6243f7d534398d27a1bde4e40dce",
};
const expectedCompileControlHashes = {
  "app.css": "65dbbcff74b6886fdd749b1c7c0587eaff06b4f4720405e10716c2b0dfee9bbc",
  "app.css.map":
    "39a03b016583b0827ec8bc92a2a1677dcb4d151324dcdd9c77d303af29b1dddf",
  "app.manifest.json":
    "453f708c9c4edbd388eda1ee9c883212cd5d31421077d123cb1d32bc0b28d4bb",
  "pliego.css.findings.json":
    "f0bef2cd675d1bf1f4a759c851d9a8be6e4617fdaaa443c00fd3696278691437",
  "pliego.css.manifest.json":
    "2ab00fa0f7b8a06e7bff3780fdc06d61b26b7136e163f8e6437907ddb013343e",
  "pliego.css.receipt.json":
    "3dfb3de854d95b39c7f69325f9eae71b37e62607f9c4e367e8188a63f848da70",
  "pliego.tokens.json":
    "3298873e66ade310459593530c86419c05a973d2404b71882c2d0ae430030a28",
};
const expectedWatchControlHashes = {
  "app.css": "65dbbcff74b6886fdd749b1c7c0587eaff06b4f4720405e10716c2b0dfee9bbc",
  "app.css.map":
    "39a03b016583b0827ec8bc92a2a1677dcb4d151324dcdd9c77d303af29b1dddf",
  "app.manifest.json":
    "453f708c9c4edbd388eda1ee9c883212cd5d31421077d123cb1d32bc0b28d4bb",
  "pliego.css.findings.json":
    "52ccd05b7c6f39520c474a72a5b3e9a6eaa34b7e79fa8f05a759dfacf1ce2852",
  "pliego.css.manifest.json":
    "537de8064740008b879872d77abfd767352c3a75474b72f79a7d95aab3982c85",
  "pliego.css.receipt.json":
    "602c062c391c030112356f996eb5bec251757cdf5ffb59b0f299b575dc869a49",
  "pliego.tokens.json":
    "3298873e66ade310459593530c86419c05a973d2404b71882c2d0ae430030a28",
};
const expectedDtcgControlHashes = {
  "app.css": "9a349f5059ffb3ace6c985dfe1fadcd9cb9ac45668b5cd0c00e26625e5a6c6c3",
  "app.css.map":
    "ce30a183a990c8dca861721b4c5ea953884fd4fb4898220023192e20abc62c3c",
  "app.manifest.json":
    "9ed48b83a35dc176305c7761938435e9a0aa5e3c15907e3c3d80de1b248bad76",
  "pliego.css.findings.json":
    "1915a6ca985813c148b7c8ad4724b3061d26790b2b3bc301a9cf9e029422cb2c",
  "pliego.css.manifest.json":
    "96a9fb6769b6ba1260e4581528c834d248a7dcb52e9a9be616ed1edfdb686c84",
  "pliego.css.receipt.json":
    "62968ba59c63c76ef01c73a8091978c30855ba8d39cf10f6717332786086546d",
  "pliego.tokens.json":
    "8659f9c443cb5c77c399fd741a27d476ec18887742c5459002fa6b82c22e864a",
};
const expectedDtcgResolverSha256 =
  "5965f868707ad92b0164788b5a7b8b8a2234ce03d7e6c71e1df66d612801b876";
const expectedDtcgConfigHashes = {
  darkLight:
    "sha256:e6ce8e2578b2bbb68c0b533495dab2a781e5c7a32665d4c4a4845146f014ba3e",
  darkDark:
    "sha256:5bab41c2b1a8d06d05fbaae1517d425a35b0bb8143fcdaeaee3c5faf48d9469e",
};
const updateGoldens = process.env.PLIEGOCSS_UPDATE_GOLDENS === "1";

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
      if (exited)
        fail(
          `watch exited before publication\nstdout:\n${stdout}\nstderr:\n${stderr}`,
        );
      if (Date.now() >= deadline)
        fail(`watch publication timed out\nstderr:\n${stderr}`);
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
  if (!stderr.includes("watching `src`"))
    fail(`watch lost its startup event:\n${stderr}`);
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
      return [
        name,
        metadata.size,
        metadata.mtimeMs,
        metadata.ino,
        readFileSync(path).toString("base64"),
      ];
    });
}

function publishedFiles(directory) {
  return readdirSync(directory)
    .filter((name) => !name.endsWith(".pliego.lock"))
    .sort();
}

function treeSnapshot(directory, prefix = "") {
  const snapshot = [];
  for (const entry of readdirSync(directory, { withFileTypes: true }).sort(
    (left, right) =>
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
    fail(
      `${message}\nactual: ${JSON.stringify(actual)}\nexpected: ${JSON.stringify(expected)}`,
    );
  }
}

function assertFrozenHashes(actual, expected, label) {
  const mismatches = Object.keys(expected).filter(
    (name) => actual[name] !== expected[name],
  );
  if (mismatches.length > 0) {
    if (updateGoldens) {
      process.stdout.write(`${label}: ${JSON.stringify(actual)}\n`);
      return;
    }
    fail(
      `${label} frozen hashes drifted for ${mismatches.join(", ")}:\n${JSON.stringify(actual, null, 2)}`,
    );
  }
}

function assertTokenGraphProjection(directory, cssFile, manifestFile, label) {
  const graphBytes = readFileSync(join(directory, "pliego.tokens.json"));
  const graph = JSON.parse(graphBytes);
  const manifest = JSON.parse(
    readFileSync(join(directory, "pliego.css.manifest.json"), "utf8"),
  );
  const receipt = JSON.parse(
    readFileSync(join(directory, "pliego.css.receipt.json"), "utf8"),
  );
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
    fail(
      `${label} lost its bounded token projection: ${JSON.stringify(tokens)}`,
    );
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
  const graphOutput = manifest.outputs.find(
    (output) => output.role === "token-graph",
  );
  const cssOutput = manifest.outputs.find(
    (output) => output.role === "generated-css",
  );
  const manifestOutput = manifest.outputs.find(
    (output) => output.role === "style-manifest",
  );
  const graphCheck = receipt.checks.find(
    (check) => check.id === "token-graph-integrity",
  );
  if (
    graphOutput?.artifact?.file !== "pliego.tokens.json" ||
    graphOutput.artifact.bytes !== graphBytes.length ||
    graphOutput.artifact.sha256 !== tokens.graphHash ||
    graphOutput.mediaType !== "application/json" ||
    graphOutput.sourceMap != null ||
    JSON.stringify(graphOutput.relationships) !==
      JSON.stringify([cssFile, manifestFile]) ||
    !cssOutput?.relationships.includes("pliego.tokens.json") ||
    !manifestOutput?.relationships.includes("pliego.tokens.json") ||
    !manifest.receipt.requiredChecks.includes("token-graph-integrity") ||
    graphCheck?.status !== "passed" ||
    graphCheck.evidenceKind !== "integrity" ||
    graphCheck.required !== true ||
    JSON.stringify(graphCheck.evidenceArtifact) !==
      JSON.stringify(graphOutput.artifact)
  ) {
    fail(`${label} did not integrity-bind its canonical token graph`);
  }
}

function assertDtcgTokenGraphProjection(
  directory,
  cssFile,
  manifestFile,
  label,
) {
  const graphBytes = readFileSync(join(directory, "pliego.tokens.json"));
  const graph = JSON.parse(graphBytes);
  const manifestBytes = readFileSync(
    join(directory, "pliego.css.manifest.json"),
  );
  const manifest = JSON.parse(manifestBytes);
  const receipt = JSON.parse(
    readFileSync(join(directory, "pliego.css.receipt.json"), "utf8"),
  );
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
    fail(
      `${label} did not publish the complete canonical Resolver 2025.10 graph`,
    );
  }
  const resolverInput = manifest.inputs.files.find(
    (input) => input.role === "token-resolver",
  );
  const adapter = manifest.inputs.adapters.find(
    (identity) => identity.name === "dtcg-resolver",
  );
  if (
    resolverInput?.file !== "examples/product.resolver.json" ||
    resolverInput.sha256 !== `sha256:${expectedDtcgResolverSha256}` ||
    adapter?.version !== "2025.10/same-document-1" ||
    tokens.observation !== "measured" ||
    tokens.graphVersion !== graph.graphVersion ||
    tokens.graphHash !==
      `sha256:${expectedDtcgControlHashes["pliego.tokens.json"]}` ||
    tokens.tokens !== 53 ||
    tokens.aliases !== 1 ||
    tokens.themes !== 4 ||
    tokens.deprecations !== 1 ||
    JSON.stringify(tokens.dtcgAdapter) !== JSON.stringify(adapter) ||
    JSON.stringify(tokens.dtcgInventory) !== JSON.stringify({ color: 3 })
  ) {
    fail(`${label} lost its exact resolver input or DTCG measurement identity`);
  }
  const graphOutput = manifest.outputs.find(
    (output) => output.role === "token-graph",
  );
  const cssOutput = manifest.outputs.find(
    (output) => output.role === "generated-css",
  );
  const manifestOutput = manifest.outputs.find(
    (output) => output.role === "style-manifest",
  );
  const graphCheck = receipt.checks.find(
    (check) => check.id === "token-graph-integrity",
  );
  if (
    graphOutput?.artifact?.file !== "pliego.tokens.json" ||
    graphOutput.artifact.bytes !== graphBytes.length ||
    graphOutput.artifact.sha256 !== tokens.graphHash ||
    graphOutput.mediaType !== "application/json" ||
    graphOutput.sourceMap != null ||
    JSON.stringify(graphOutput.relationships) !==
      JSON.stringify([cssFile, manifestFile]) ||
    !cssOutput?.relationships.includes("pliego.tokens.json") ||
    !manifestOutput?.relationships.includes("pliego.tokens.json") ||
    !manifest.receipt.requiredChecks.includes("token-graph-integrity") ||
    graphCheck?.status !== "passed" ||
    graphCheck.evidenceKind !== "integrity" ||
    graphCheck.required !== true ||
    JSON.stringify(graphCheck.evidenceArtifact) !==
      JSON.stringify(graphOutput.artifact)
  ) {
    fail(`${label} did not integrity-bind its complete DTCG token graph`);
  }
}

function assertControlArtifactDriftReadOnly(
  arguments_,
  cwd,
  directory,
  artifact,
  label,
) {
  const path = join(directory, artifact);
  const original = readFileSync(path);
  writeFileSync(path, Buffer.concat([original, Buffer.from("drift\n")]));
  const drifted = outputSnapshot(directory);
  const result = runCli([...arguments_, "--check"], cwd, 1);
  if (!result.stderr.includes("drift")) {
    fail(`${label} drift failure lost its diagnostic: ${result.stderr}`);
  }
  assertEqual(
    outputSnapshot(directory),
    drifted,
    `${label} drift check mutated outputs`,
  );
  runCli(arguments_, cwd);
}

function assertSourceMapProjection(directory, cssFile, expectedSource, label) {
  const mapFile = `${cssFile}.map`;
  const css = readFileSync(join(directory, cssFile), "utf8");
  const mapBytes = readFileSync(join(directory, mapFile));
  const map = JSON.parse(mapBytes);
  if (
    map.version !== 3 ||
    JSON.stringify(Object.keys(map)) !==
      JSON.stringify(["version", "sources", "names", "mappings"]) ||
    !Array.isArray(map.sources) ||
    !map.sources.includes(expectedSource) ||
    !Array.isArray(map.names) ||
    map.names.length !== 0 ||
    typeof map.mappings !== "string" ||
    map.mappings.length === 0 ||
    css.includes("sourceMappingURL")
  ) {
    fail(
      `${label} emitted an invalid or unstable Source Map v3: ${JSON.stringify(map)}`,
    );
  }
  const manifest = JSON.parse(
    readFileSync(join(directory, "pliego.css.manifest.json"), "utf8"),
  );
  const cssOutput = manifest.outputs.find(
    (output) => output.artifact.file === cssFile,
  );
  const mapOutput = manifest.outputs.find(
    (output) => output.artifact.file === mapFile,
  );
  if (
    cssOutput?.sourceMap?.file !== mapFile ||
    cssOutput.sourceMap.bytes !== mapBytes.length ||
    cssOutput.sourceMap.sha256 !== `sha256:${sha256(mapBytes)}` ||
    JSON.stringify(cssOutput.sourceMap) !==
      JSON.stringify(mapOutput?.artifact) ||
    mapOutput?.role !== "css-source-map" ||
    mapOutput.mediaType !== "application/json" ||
    !mapOutput.relationships.includes(cssFile)
  ) {
    fail(`${label} did not integrity-bind its CSS source map`);
  }
}

function bundleArguments(plan, outputDir, check = false) {
  return [
    "bundle",
    "--plan",
    plan,
    "--output-dir",
    outputDir,
    ...(check ? ["--check"] : []),
  ];
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
  'fn portable_view() {\r\n    let _marker = "🦀";\r\n    let _ = pc!("flex gap-4");\r\n}\r\n';
writeFileSync(join(project, "src", sourceName), portableSource);
const plan = join(project, "pliego.bundles.toml");
writeFileSync(
  plan,
  `schema = 1\ntargets = "modern"\nformat = "minified"\n\n[theme]\nkind = "seed"\n\n[bundles.app]\nsources = [${JSON.stringify(logicalSource)}]\nemit-theme = true\n`,
);
const macroText = 'pc!("flex gap-4")';
const macroStart = Buffer.byteLength(
  portableSource.slice(0, portableSource.indexOf(macroText)),
);
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
assertEqual(
  firstManifestBytes,
  secondManifestBytes,
  "manifest bytes changed with process CWD",
);

const manifest = JSON.parse(firstManifestBytes);
if (
  manifest.schemaVersion !== 3 ||
  manifest.styleIdFormatVersion !== 2 ||
  manifest.classNameFormatVersion !== 1 ||
  manifest.themeIdFormatVersion !== 3 ||
  manifest.cssBytes !== firstCss.byteLength ||
  manifest.cssSha256 !== sha256(firstCss) ||
  manifest.styles.length !== 1
) {
  fail("bundle manifest integrity contract drifted");
}
if (
  manifest.cssSha256 !== expectedCssSha256 ||
  sha256(firstManifestBytes) !== expectedManifestSha256
) {
  if (updateGoldens) {
    process.stdout.write(
      `bundle: ${JSON.stringify({ css: manifest.cssSha256, manifest: sha256(firstManifestBytes) })}\n`,
    );
  } else {
    fail(
      `frozen cross-platform hashes drifted: CSS ${manifest.cssSha256}, manifest ${sha256(firstManifestBytes)}`,
    );
  }
}
if (
  !manifest.styles[0].origins.every(
    (origin) => origin.file === logicalSource && !origin.file.includes("\\"),
  )
) {
  fail(
    "bundle provenance is not stable plan-relative UTF-8 with forward slashes",
  );
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
    if (updateGoldens)
      process.stdout.write(`audit control ${name}: ${actual}\n`);
    else
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
  bundleControlFiles.map((name) => [
    name,
    sha256(readFileSync(join(bundleControlOutput, name))),
  ]),
);
assertFrozenHashes(
  bundleControlHashes,
  expectedBundleControlHashes,
  "bundle control",
);
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
assertSourceMapProjection(
  bundleControlOutput,
  "app.css",
  logicalSource,
  "bundle control",
);
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
  assetPlanControlFiles.map((name) => [
    name,
    sha256(readFileSync(join(assetPlanControlOutput, name))),
  ]),
);
for (const [name, expected] of Object.entries(expectedAssetPlanControlHashes)) {
  if (assetPlanControlHashes[name] !== expected) {
    if (updateGoldens)
      process.stdout.write(
        `asset plan control ${name}: ${assetPlanControlHashes[name]}\n`,
      );
    else
      fail(
        `frozen Asset Plan control hash drifted for ${name}: ${assetPlanControlHashes[name]}`,
      );
  }
}
const reopenedAssetPlanTokens = JSON.parse(
  readFileSync(
    join(assetPlanControlOutput, "pliego.css.manifest.json"),
    "utf8",
  ),
).tokens;
if (
  reopenedAssetPlanTokens.observation !== "unavailable" ||
  !reopenedAssetPlanTokens.unavailableReason
) {
  fail(
    `reopened Asset Plan invented token evidence: ${JSON.stringify(reopenedAssetPlanTokens)}`,
  );
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
  compileControlFiles.map((name) => [
    name,
    sha256(readFileSync(join(compileControlOutput, name))),
  ]),
);
assertFrozenHashes(
  compileControlHashes,
  expectedCompileControlHashes,
  "compile control",
);
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
assertSourceMapProjection(
  compileControlOutput,
  "app.css",
  logicalSource,
  "compile control",
);
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
  watchControlFiles.map((name) => [
    name,
    sha256(readFileSync(join(watchControlOutput, name))),
  ]),
);
assertFrozenHashes(
  watchControlHashes,
  expectedWatchControlHashes,
  "watch control",
);
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
assertSourceMapProjection(
  watchControlOutput,
  "app.css",
  logicalSource,
  "watch control",
);
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
  dtcgControlFiles.map((name) => [
    name,
    sha256(readFileSync(join(dtcgControlOutput, name))),
  ]),
);
assertFrozenHashes(
  dtcgControlHashes,
  expectedDtcgControlHashes,
  "direct DTCG control",
);
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
  dtcgControlManifest.inputs.configHash !==
    expectedDtcgConfigHashes.darkLight ||
  dtcgStyleManifest.themeId !== "28acd9cd56861d1aea635ce531e240f5" ||
  !dtcgCss.toString("utf8").includes("--color-brand:color(srgb .1 .1 .1)")
) {
  if (updateGoldens) {
    process.stdout.write(
      `dtcg darkLight: ${JSON.stringify({ configHash: dtcgControlManifest.inputs.configHash, themeId: dtcgStyleManifest.themeId })}\n`,
    );
  } else {
    fail(
      "direct DTCG control lost its exact canonical dark/light selection identity",
    );
  }
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
  dtcgIdentityManifest.inputs.configHash !==
    expectedDtcgConfigHashes.darkDark ||
  dtcgIdentityManifest.inputs.configHash ===
    dtcgControlManifest.inputs.configHash ||
  !readFileSync(join(dtcgIdentityOutput, "app.css")).equals(dtcgCss) ||
  !readFileSync(join(dtcgIdentityOutput, "app.manifest.json")).equals(
    readFileSync(join(dtcgControlOutput, "app.manifest.json")),
  ) ||
  !readFileSync(join(dtcgIdentityOutput, "pliego.tokens.json")).equals(
    readFileSync(join(dtcgControlOutput, "pliego.tokens.json")),
  )
) {
  if (updateGoldens) {
    process.stdout.write(
      `dtcg darkDark: ${JSON.stringify({ configHash: dtcgIdentityManifest.inputs.configHash })}\n`,
    );
  } else {
    fail(
      "canonical DTCG selections stopped participating in configHash independently of ThemeId",
    );
  }
}
const sourceBytes = readFileSync(join(project, "src", sourceName));
for (const origin of manifest.styles[0].origins) {
  if (
    !Number.isInteger(origin.byteStart) ||
    !Number.isInteger(origin.byteEnd) ||
    sourceBytes.subarray(origin.byteStart, origin.byteEnd).toString("utf8") !==
      'pc!("flex gap-4")'
  ) {
    fail(
      `manifest origin does not select the exact macro bytes: ${JSON.stringify(origin)}`,
    );
  }
}

const checkOutput = join(runtime, "output-check");
mkdirSync(checkOutput);
copyFileSync(join(firstOutput, cssName), join(checkOutput, cssName));
copyFileSync(join(firstOutput, manifestName), join(checkOutput, manifestName));
const beforeCheck = outputSnapshot(checkOutput);
runCli(bundleArguments(plan, checkOutput, true), foreignCwd);
assertEqual(
  outputSnapshot(checkOutput),
  beforeCheck,
  "successful --check wrote to the output directory",
);

const emptyCheckOutput = join(runtime, "output-empty-check");
mkdirSync(emptyCheckOutput);
const emptyCheck = runCli(
  bundleArguments(plan, emptyCheckOutput, true),
  foreignCwd,
  1,
);
if (!emptyCheck.stderr.includes("bundle output drift detected")) {
  fail(`empty --check failure lost its diagnostic: ${emptyCheck.stderr}`);
}
assertEqual(
  readdirSync(emptyCheckOutput),
  [],
  "failed --check created output or lock files",
);

writeFileSync(
  join(checkOutput, cssName),
  Buffer.concat([firstCss, Buffer.from("/* drift */\n")]),
);
const beforeDriftCheck = outputSnapshot(checkOutput);
const drift = runCli(bundleArguments(plan, checkOutput, true), foreignCwd, 1);
if (!drift.stderr.includes("bundle output drift detected")) {
  fail(`drift failure lost its diagnostic: ${drift.stderr}`);
}
assertEqual(
  outputSnapshot(checkOutput),
  beforeDriftCheck,
  "failed --check mutated drifted outputs",
);

const outside = join(runtime, "outside-plan");
const escape = join(project, "escape");
const escapeOutput = join(runtime, "output-escape");
mkdirSync(outside);
mkdirSync(escapeOutput);
writeFileSync(join(outside, "leak.rs"), 'fn leak() { let _ = pc!("grid"); }\n');
symlinkSync(outside, escape, process.platform === "win32" ? "junction" : "dir");
const escapePlan = join(project, "escape.bundles.toml");
writeFileSync(
  escapePlan,
  'schema=1\ntargets="modern"\nformat="minified"\n[theme]\nkind="seed"\n[bundles.escape]\nsources=["escape/leak.rs"]\n',
);
const escaped = runCli(bundleArguments(escapePlan, escapeOutput), root, 1);
if (
  !/symbolic link|reparse|outside bundle plan directory/.test(escaped.stderr)
) {
  fail(`link escape did not fail closed: ${escaped.stderr}`);
}
assertEqual(
  readdirSync(escapeOutput),
  [],
  "link escape published output or lock files",
);

const linkedOutputTarget = join(runtime, "linked-output-target");
const linkedOutput = join(runtime, "linked-output");
mkdirSync(linkedOutputTarget);
symlinkSync(
  linkedOutputTarget,
  linkedOutput,
  process.platform === "win32" ? "junction" : "dir",
);
const linkedDestination = runCli(bundleArguments(plan, linkedOutput), root, 1);
if (!/symbolic link|reparse/.test(linkedDestination.stderr)) {
  fail(
    `linked output directory did not fail closed: ${linkedDestination.stderr}`,
  );
}
assertEqual(
  readdirSync(linkedOutputTarget),
  [],
  "linked output directory received artifacts",
);

const outsideTheme = join(runtime, "outside-theme");
const linkedTheme = join(project, "linked-theme");
const linkedThemeOutput = join(runtime, "output-linked-theme");
mkdirSync(outsideTheme);
mkdirSync(linkedThemeOutput);
writeFileSync(join(outsideTheme, "theme.toml"), 'schema=1\nextends="seed"\n');
symlinkSync(
  outsideTheme,
  linkedTheme,
  process.platform === "win32" ? "junction" : "dir",
);
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
if (
  !/symbolic link|reparse|outside bundle plan directory/.test(
    linkedConfiguration.stderr,
  )
) {
  fail(
    `linked theme configuration did not fail closed: ${linkedConfiguration.stderr}`,
  );
}
assertEqual(
  readdirSync(linkedThemeOutput),
  [],
  "linked theme path published output or lock files",
);

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
assertEqual(
  readdirSync(dotOutput),
  [],
  "invalid dot path published output or lock files",
);

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
  fail(
    `case-insensitive input/output alias was not rejected: ${aliased.stderr}`,
  );
}
if (readFileSync(configAlias, "utf8") !== configBytes) {
  fail("case-insensitive alias check modified the theme configuration");
}
assertEqual(
  treeSnapshot(project),
  beforeAlias,
  "case-insensitive alias failure mutated project files",
);
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
