import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, isAbsolute, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";

const scriptPath = fileURLToPath(import.meta.url);
const root = resolve(dirname(scriptPath), "..");
const runtimeBase = join(root, "target", "benchmarks", "reachability-pruning");
const runtime = join(runtimeBase, `run-${process.pid}`);
const check = process.argv.length === 3 && process.argv[2] === "--check";

const expected = {
  sourceSha256: "159233dbb9258f93bbc0561e795780aa3b803a2cab8bfa3b19d37284d40359c0",
  reachabilitySha256: "d457dd60004d9d989d42928c5665a5ed84561bf4955f61af3a23ecf21b876a76",
  baseline: {
    cssSha256: "83940c253ce21b33283b8424c88d42e41c6b2b9cab1b2193934887f93f9e9504",
    rawBytes: 7_283,
    gzipBytes: 1_214,
    manifestSha256: "f4b0d70440860dfb79bbdaeff74d47ec8e71d4bf577a06d83e4165d390ddae93",
    styles: 14,
    declarations: 147,
    tokens: 30,
    physicalRules: 60,
    physicalDeclarations: 207,
  },
  pruned: {
    cssSha256: "fcd12e252a983989e1006701539aadf375311327ef2aa294afc1e79e44f6edb3",
    rawBytes: 366,
    gzipBytes: 224,
    manifestSha256: "2a7f5766f577e487334ad014600602cb315ee846455912572e2192b2e7fa2197",
    styles: 2,
    declarations: 13,
    tokens: 9,
    physicalRules: 2,
    physicalDeclarations: 15,
  },
  themedBaseline: {
    cssSha256: "c44f73c55e746cc012b35aaa4519df2933382c4abdf00016eb117718a948033f",
    rawBytes: 7_654,
    gzipBytes: 1_370,
    manifestSha256: "5ca365733d1ba568e758150a0ba4f497e1c0704ba36c820165c1d4e6f957c8bc",
    styles: 14,
    declarations: 147,
    tokens: 30,
    physicalRules: 61,
    physicalDeclarations: 217,
    customProperties: [
      "--color-accent",
      "--color-accent-strong",
      "--color-canvas",
      "--color-ink",
      "--color-line",
      "--color-muted",
      "--color-surface",
      "--color-surface-raised",
      "--font-mono",
      "--font-sans",
    ],
  },
  themedPruned: {
    cssSha256: "97f7110faa860b545b0d9a432ed68fb094088b0aebdd79db76435dbbc1c28ce5",
    rawBytes: 457,
    gzipBytes: 264,
    manifestSha256: "4db44f42fba2570580f191da0d2177207e95ce8ded7bf7a1f9cb4c5cdb378ed2",
    styles: 2,
    declarations: 13,
    tokens: 9,
    physicalRules: 3,
    physicalDeclarations: 18,
    customProperties: ["--color-accent", "--color-ink", "--color-surface"],
  },
};

const expectedGzipSha256ByZlib = {
  "1.3.0.1-motley-82a5fec": {
    baseline: "f29588fd0dada2e22227f827a752785e2c4bf04f894379b41b10f42e0cc717f2",
    pruned: "cf24e99f35cd7cdb95bcab9be2fc8e7508f380d8f9a20c9a2f7690ed1eda0d60",
    themedBaseline: "6fdb9eb9fdbbed3e05b6cdb75fc7c7279ee2d6c3757664c4bb5e02c3fb10bfdd",
    themedPruned: "393437116dab5d6803a5842d899546598f5ca2f64c3990267321e76f069b27f1",
  },
  "1.3.1-e00f703": {
    baseline: "f29588fd0dada2e22227f827a752785e2c4bf04f894379b41b10f42e0cc717f2",
    pruned: "cf24e99f35cd7cdb95bcab9be2fc8e7508f380d8f9a20c9a2f7690ed1eda0d60",
    themedBaseline: "6fdb9eb9fdbbed3e05b6cdb75fc7c7279ee2d6c3757664c4bb5e02c3fb10bfdd",
    themedPruned: "393437116dab5d6803a5842d899546598f5ca2f64c3990267321e76f069b27f1",
  },
};

const liveStyles = [
  {
    id: "route-shell",
    root: "route",
    style: "flex items-center gap-4 rounded-lg bg-surface p-4 text-ink",
  },
  {
    id: "counter-island",
    root: "island",
    style: "inline-flex rounded-md bg-accent px-3 py-2 text-white",
  },
];

const deadStyles = [
  "inline-flex cursor-pointer items-center justify-center gap-2 rounded-lg bg-accent px-4 py-2 text-sm font-semibold text-white shadow-sm hover:bg-accent-strong focus:ring-2 focus:ring-accent disabled:cursor-not-allowed disabled:opacity-40 sm:px-3 md:px-4 lg:px-5",
  "inline-flex cursor-pointer items-center justify-center gap-2 rounded-lg border border-line bg-surface px-4 py-2 text-sm font-semibold text-ink shadow-sm hover:bg-surface-raised focus:ring-2 focus:ring-accent disabled:cursor-not-allowed disabled:opacity-40",
  "flex flex-col gap-6 rounded-2xl border border-line bg-surface p-6 shadow-sm hover:bg-surface-raised md:flex-row md:gap-8 lg:p-8",
  "w-full h-40 rounded-xl bg-surface-raised md:w-40 lg:w-40",
  "max-w-prose text-sm leading-6 text-muted",
  "flex flex-col gap-4 border-b border-line bg-canvas pb-4 hover:bg-surface sm:flex-row sm:items-center sm:justify-between md:px-4 lg:px-6",
  "rounded-md px-3 py-2 text-sm font-medium text-muted hover:bg-surface-raised hover:text-ink focus:ring-2 focus:ring-accent",
  "grid gap-6 rounded-2xl border border-line bg-surface p-6 focus:ring-2 focus:ring-accent sm:grid-cols-1 md:grid-cols-2 lg:gap-8",
  "w-full rounded-lg border border-line bg-white px-3 py-2 text-sm text-ink focus:border-accent focus:ring-2 focus:ring-accent",
  "grid grid-cols-1 gap-4 sm:grid-cols-1 md:grid-cols-2 lg:grid-cols-3",
  "rounded-xl border border-line bg-surface p-5 shadow-sm hover:bg-surface-raised focus:ring-2 focus:ring-accent",
  "inline-block rounded-full bg-accent/20 px-2 py-1 text-xs font-semibold text-accent-strong",
].map((style, index) => ({
  id: `dead-${String(index + 1).padStart(2, "0")}`,
  root: "dead",
  style,
}));

const fixtureStyles = [...liveStyles, ...deadStyles];

if (!check && process.argv.length !== 2) {
  throw new Error("usage: node scripts/measure-reachability-pruning.mjs [--check]");
}

function fail(message) {
  throw new Error(message);
}

function assert(condition, message) {
  if (!condition) fail(message);
}

function assertEqual(actual, expectedValue, message) {
  if (JSON.stringify(actual) !== JSON.stringify(expectedValue)) {
    fail(
      `${message}\nactual: ${JSON.stringify(actual, null, 2)}\n` +
        `expected: ${JSON.stringify(expectedValue, null, 2)}`,
    );
  }
}

function assertNodeVersion() {
  const [major, minor] = process.versions.node.split(".").map(Number);
  assert(
    major > 22 || (major === 22 && minor >= 13),
    `Node ${process.versions.node} is unsupported; expected >=22.13`,
  );
}

function assertSafeRuntime() {
  const child = relative(runtimeBase, runtime);
  assert(child !== "" && !child.startsWith(`..${sep}`) && child !== ".." && !isAbsolute(child),
    `unsafe benchmark runtime path: ${runtime}`,
  );
  assert(child === `run-${process.pid}`, `unexpected benchmark runtime leaf: ${child}`);
}

function run(program, args, options = {}) {
  const result = spawnSync(program, args, {
    cwd: root,
    encoding: "utf8",
    maxBuffer: 32 * 1024 * 1024,
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

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function deterministicGzip(bytes) {
  const compressed = gzipSync(bytes, { level: 9 });
  assert(
    compressed[0] === 0x1f && compressed[1] === 0x8b && compressed.length >= 10,
    "zlib did not produce a gzip stream",
  );
  compressed[9] = 0xff;
  return compressed;
}

function resolveExecutable() {
  if (process.env.PLIEGO_CSSC) {
    const configured = resolve(root, process.env.PLIEGO_CSSC);
    assert(existsSync(configured), `PLIEGO_CSSC does not exist: ${configured}`);
    return configured;
  }
  const executable = join(
    root,
    "target",
    "debug",
    process.platform === "win32" ? "pliego-cssc.exe" : "pliego-cssc",
  );
  if (!existsSync(executable)) {
    run("cargo", ["+1.96.0", "build", "--locked", "-p", "pliego-cssc"], {
      env: { ...process.env, CARGO_INCREMENTAL: "0" },
    });
  }
  assert(existsSync(executable), `cargo did not produce ${executable}`);
  return executable;
}

function renderFixture() {
  return fixtureStyles
    .map(
      (item, index) =>
        `fn fixture_${String(index).padStart(2, "0")}() {\n` +
        `    let _ = pc!(${JSON.stringify(item.style)});\n}\n`,
    )
    .join("\n");
}

function buildReachability(source) {
  let cursor = 0;
  const components = fixtureStyles.map((item) => {
    const invocation = `pc!(${JSON.stringify(item.style)})`;
    const byteStart = source.indexOf(invocation, cursor);
    assert(byteStart >= cursor, `fixture invocation is missing for ${item.id}`);
    const byteEnd = byteStart + Buffer.byteLength(invocation);
    cursor = byteEnd;
    return {
      id: item.id,
      sites: [{ file: "src/styles.rs", byteStart, byteEnd }],
    };
  });
  assert(source.indexOf("pc!(", cursor) === -1, "fixture has an unindexed pc! invocation");
  return {
    schema: 1,
    applicationCoverage: "complete",
    components,
    routes: [{ id: "home", path: "/", components: ["route-shell"] }],
    islands: [
      { id: "counter", name: "Counter", components: ["counter-island"] },
    ],
  };
}

function compile(executable, name, prune, theme = false) {
  const css = `out/${name}.css`;
  const manifest = `out/${name}.manifest.json`;
  const args = [
    "compile",
    "--source",
    "src/styles.rs",
    "--seed",
    "--targets",
    "modern",
    "--format",
    "minified",
    "--output",
    css,
    "--manifest",
    manifest,
    "--manifest-version",
    "5",
    "--reachability",
    "pliego.reachability.json",
    ...(theme ? ["--theme"] : []),
    ...(prune ? ["--prune-unreachable"] : []),
  ];
  const result = run(executable, args, { cwd: runtime });
  assert(result.stdout === "", `${name} emitted stdout:\n${result.stdout}`);
  assert(result.stderr === "", `${name} emitted stderr:\n${result.stderr}`);
  return {
    css: readFileSync(join(runtime, css)),
    manifest: readFileSync(join(runtime, manifest)),
  };
}

function sortedUnique(values, description) {
  const sorted = [...values].sort();
  assertEqual(values, sorted, `${description} is not canonically sorted`);
  assert(new Set(values).size === values.length, `${description} contains duplicates`);
}

function graphIds(manifest) {
  const graph = manifest.graph;
  return new Set([
    ...manifest.styles.map((style) => `style:${style.styleId}`),
    ...graph.declarations.map((item) => item.id),
    ...graph.tokens.map((item) => item.id),
    ...graph.components.map((item) => item.id),
    ...graph.routes.map((item) => item.id),
    ...graph.islands.map((item) => item.id),
    ...graph.syntheticProducers.map((item) => item.id),
    ...graph.physicalRules.map((item) => item.id),
    ...graph.physicalDeclarations.map((item) => item.id),
  ]);
}

function assertRanges(graph, css) {
  for (const rule of graph.physicalRules) {
    assert(
      0 <= rule.byteStart &&
        rule.byteStart < rule.headerByteEnd &&
        rule.headerByteStart === rule.byteStart &&
        rule.headerByteEnd < rule.byteEnd &&
        rule.byteEnd <= css.length,
      `invalid physical rule range for ${rule.id}`,
    );
    assert(css.subarray(rule.byteStart, rule.byteEnd).at(-1) === 0x7d,
      `${rule.id} does not include its closing brace`,
    );
  }
  for (const declaration of graph.physicalDeclarations) {
    assert(
      0 <= declaration.byteStart &&
        declaration.byteStart === declaration.propertyByteStart &&
        declaration.propertyByteStart < declaration.propertyByteEnd &&
        declaration.propertyByteEnd <= declaration.valueByteStart &&
        declaration.valueByteStart < declaration.valueByteEnd &&
        declaration.valueByteEnd <= declaration.byteEnd &&
        declaration.byteEnd <= css.length,
      `invalid physical declaration range for ${declaration.id}`,
    );
    assert(
      css
        .subarray(declaration.propertyByteStart, declaration.propertyByteEnd)
        .toString("utf8") === declaration.property,
      `${declaration.id} property range drifted`,
    );
  }
}

function assertClosedGraph(manifest, css, source, expectedStyleCount) {
  assert(manifest.schemaVersion === 5, "benchmark manifest is not schema 5");
  assert(manifest.cssSha256 === sha256(css), "manifest CSS hash is stale");
  assert(manifest.cssBytes === css.length, "manifest CSS byte count is stale");
  assert(css.at(-1) === 0x0a, "CSS lost its final newline");
  assert(manifest.styles.length === expectedStyleCount, "semantic style count drifted");
  const graph = manifest.graph;
  assert(graph?.schemaVersion === 2, "benchmark graph is not schema 2");
  assert(graph.originCoverage === "compiler-verified-complete", "origin coverage is open");
  assert(graph.applicationCoverage === "adapter-attested-complete", "application coverage is open");
  assert(graph.physicalCoverage === "compiler-verified-complete", "physical coverage is open");

  for (const [field, values] of Object.entries({
    declarations: graph.declarations,
    tokens: graph.tokens,
    components: graph.components,
    routes: graph.routes,
    islands: graph.islands,
    syntheticProducers: graph.syntheticProducers,
    physicalRules: graph.physicalRules,
    physicalDeclarations: graph.physicalDeclarations,
  })) {
    sortedUnique(values.map((item) => item.id), `graph.${field}`);
  }
  sortedUnique(
    graph.edges.map((edge) => `${edge.kind}\0${edge.from}\0${edge.to}`),
    "graph.edges",
  );

  const nodes = graphIds(manifest);
  const expectedNodes =
    manifest.styles.length +
    graph.declarations.length +
    graph.tokens.length +
    graph.components.length +
    graph.routes.length +
    graph.islands.length +
    graph.syntheticProducers.length +
    graph.physicalRules.length +
    graph.physicalDeclarations.length;
  assert(nodes.size === expectedNodes, "graph node IDs are not globally unique");
  for (const edge of graph.edges) {
    assert(nodes.has(edge.from), `edge source is dangling: ${edge.from}`);
    assert(nodes.has(edge.to), `edge destination is dangling: ${edge.to}`);
  }

  const edges = (kind) => graph.edges.filter((edge) => edge.kind === kind);
  const declarations = new Set(graph.declarations.map((item) => item.id));
  const physicalDeclarations = new Set(graph.physicalDeclarations.map((item) => item.id));
  const physicalRules = new Set(graph.physicalRules.map((item) => item.id));
  for (const declaration of declarations) {
    assert(edges("styleHasDeclaration").some((edge) => edge.to === declaration),
      `${declaration} has no semantic style`,
    );
    assert(edges("componentUsesDeclaration").some((edge) => edge.to === declaration),
      `${declaration} has no component owner`,
    );
    assert(
      edges("declarationContributesToPhysicalDeclaration").some(
        (edge) => edge.from === declaration && physicalDeclarations.has(edge.to),
      ),
      `${declaration} has no physical contribution`,
    );
  }
  for (const declaration of physicalDeclarations) {
    const owners = edges("physicalDeclarationBelongsToRule").filter(
      (edge) => edge.from === declaration && physicalRules.has(edge.to),
    );
    assert(owners.length === 1, `${declaration} does not belong to exactly one rule`);
    const caused =
      edges("declarationContributesToPhysicalDeclaration").some(
        (edge) => edge.to === declaration,
      ) ||
      edges("syntheticProducerProducesPhysicalDeclaration").some(
        (edge) => edge.to === declaration,
      );
    assert(caused, `${declaration} has no semantic or synthetic producer`);
  }
  for (const token of graph.tokens) {
    assert(edges("declarationUsesToken").some((edge) => edge.to === token.id),
      `${token.id} has no declaration consumer`,
    );
  }

  for (const style of manifest.styles) {
    assert(css.toString("utf8").includes(`.${style.className}`),
      `${style.className} is absent from CSS`,
    );
    for (const origin of style.origins) {
      assert(origin.file.replaceAll("\\", "/") === "src/styles.rs", "origin path drifted");
      const invocation = source.subarray(origin.byteStart, origin.byteEnd).toString("utf8");
      assert(invocation.startsWith('pc!("') && invocation.endsWith('")'),
        "origin no longer selects an exact pc! invocation",
      );
      assert(invocation.includes(origin.source), "origin payload drifted");
    }
  }
  assertRanges(graph, css);
  return {
    styles: manifest.styles.length,
    declarations: graph.declarations.length,
    tokens: graph.tokens.length,
    physicalRules: graph.physicalRules.length,
    physicalDeclarations: graph.physicalDeclarations.length,
  };
}

function assertRootAndPruningSemantics(baseline, pruned, reachability) {
  assert(baseline.styles.length === fixtureStyles.length, "baseline lost fixture styles");
  assert(pruned.styles.length === liveStyles.length, "pruned output retained dead styles");
  const originStarts = new Set(
    pruned.styles.flatMap((style) => style.origins.map((origin) => origin.byteStart)),
  );
  const components = new Map(reachability.components.map((item) => [item.id, item]));
  for (const item of liveStyles) {
    assert(originStarts.has(components.get(item.id).sites[0].byteStart),
      `live ${item.root} style ${item.id} was pruned`,
    );
  }
  for (const item of deadStyles) {
    assert(!originStarts.has(components.get(item.id).sites[0].byteStart),
      `dead style ${item.id} survived`,
    );
  }
  const edgeKeys = new Set(
    pruned.graph.edges.map((edge) => `${edge.kind}\0${edge.from}\0${edge.to}`),
  );
  assert(
    edgeKeys.has("routeUsesComponent\0route:home\0component:route-shell"),
    "route root edge is missing",
  );
  assert(
    edgeKeys.has("islandUsesComponent\0island:counter\0component:counter-island"),
    "island root edge is missing",
  );
  assert(
    pruned.graph.declarations.length < baseline.graph.declarations.length,
    "pruning did not remove semantic declarations",
  );
  assert(
    pruned.graph.tokens.length < baseline.graph.tokens.length,
    "pruning did not remove dead semantic token nodes",
  );
}

function measurement(artifact, counts) {
  const firstGzip = deterministicGzip(artifact.css);
  const secondGzip = deterministicGzip(artifact.css);
  assert(firstGzip.equals(secondGzip), "gzip output is not byte-deterministic");
  return {
    cssSha256: sha256(artifact.css),
    rawBytes: artifact.css.length,
    gzipSha256: sha256(firstGzip),
    gzipBytes: firstGzip.length,
    manifestSha256: sha256(artifact.manifest),
    ...counts,
  };
}

function customProperties(css) {
  const rootRule = css.toString("utf8").match(/^:root\{([^}]*)\}/);
  assert(rootRule, "themed CSS does not begin with a :root rule");
  return [...rootRule[1].matchAll(/(--[a-z0-9-]+):/g)].map((match) => match[1]);
}

assertNodeVersion();
assertSafeRuntime();
process.once("exit", () => rmSync(runtime, { recursive: true, force: true }));
rmSync(runtime, { recursive: true, force: true });
mkdirSync(join(runtime, "src"), { recursive: true });
mkdirSync(join(runtime, "out"), { recursive: true });

const source = renderFixture();
const reachability = buildReachability(source);
const sourceBytes = Buffer.from(source);
const reachabilityBytes = Buffer.from(`${JSON.stringify(reachability, null, 2)}\n`);
writeFileSync(join(runtime, "src", "styles.rs"), sourceBytes);
writeFileSync(join(runtime, "pliego.reachability.json"), reachabilityBytes);

const executable = resolveExecutable();
const baseline = compile(executable, "baseline-a", false);
const baselineRepeat = compile(executable, "baseline-b", false);
const pruned = compile(executable, "pruned-a", true);
const prunedRepeat = compile(executable, "pruned-b", true);
const themedBaseline = compile(executable, "themed-baseline-a", false, true);
const themedBaselineRepeat = compile(executable, "themed-baseline-b", false, true);
const themedPruned = compile(executable, "themed-pruned-a", true, true);
const themedPrunedRepeat = compile(executable, "themed-pruned-b", true, true);
assert(baseline.css.equals(baselineRepeat.css), "baseline CSS repeat drifted");
assert(baseline.manifest.equals(baselineRepeat.manifest), "baseline manifest repeat drifted");
assert(pruned.css.equals(prunedRepeat.css), "pruned CSS repeat drifted");
assert(pruned.manifest.equals(prunedRepeat.manifest), "pruned manifest repeat drifted");
assert(themedBaseline.css.equals(themedBaselineRepeat.css), "themed baseline CSS repeat drifted");
assert(themedBaseline.manifest.equals(themedBaselineRepeat.manifest), "themed baseline manifest repeat drifted");
assert(themedPruned.css.equals(themedPrunedRepeat.css), "themed pruned CSS repeat drifted");
assert(themedPruned.manifest.equals(themedPrunedRepeat.manifest), "themed pruned manifest repeat drifted");

const baselineManifest = JSON.parse(baseline.manifest);
const prunedManifest = JSON.parse(pruned.manifest);
const themedBaselineManifest = JSON.parse(themedBaseline.manifest);
const themedPrunedManifest = JSON.parse(themedPruned.manifest);
const baselineCounts = assertClosedGraph(
  baselineManifest,
  baseline.css,
  sourceBytes,
  fixtureStyles.length,
);
const prunedCounts = assertClosedGraph(
  prunedManifest,
  pruned.css,
  sourceBytes,
  liveStyles.length,
);
const themedBaselineCounts = assertClosedGraph(
  themedBaselineManifest,
  themedBaseline.css,
  sourceBytes,
  fixtureStyles.length,
);
const themedPrunedCounts = assertClosedGraph(
  themedPrunedManifest,
  themedPruned.css,
  sourceBytes,
  liveStyles.length,
);
assertRootAndPruningSemantics(baselineManifest, prunedManifest, reachability);
assertRootAndPruningSemantics(themedBaselineManifest, themedPrunedManifest, reachability);

const baselineCustomProperties = customProperties(themedBaseline.css);
const prunedCustomProperties = customProperties(themedPruned.css);
sortedUnique(baselineCustomProperties, "themed baseline custom properties");
sortedUnique(prunedCustomProperties, "themed pruned custom properties");
const baselineCustomPropertySet = new Set(baselineCustomProperties);
assert(prunedCustomProperties.length > 0, "themed pruning removed every custom property");
assert(
  prunedCustomProperties.every((property) => baselineCustomPropertySet.has(property)),
  "themed pruning introduced a custom property absent from the baseline",
);
assert(
  prunedCustomProperties.length < baselineCustomProperties.length,
  "themed pruning did not remove unused custom properties",
);

const frozen = {
  sourceSha256: sha256(sourceBytes),
  reachabilitySha256: sha256(reachabilityBytes),
  baseline: measurement(baseline, baselineCounts),
  pruned: measurement(pruned, prunedCounts),
  themedBaseline: {
    ...measurement(themedBaseline, themedBaselineCounts),
    customProperties: baselineCustomProperties,
  },
  themedPruned: {
    ...measurement(themedPruned, themedPrunedCounts),
    customProperties: prunedCustomProperties,
  },
};
const rawSaved = frozen.baseline.rawBytes - frozen.pruned.rawBytes;
const gzipSaved = frozen.baseline.gzipBytes - frozen.pruned.gzipBytes;
const gates = {
  repeatedCompilerBytes: true,
  graphSchemaTwoClosed: true,
  exactLiveStyleCount: frozen.pruned.styles === liveStyles.length,
  deadStylesRemoved: frozen.baseline.styles - frozen.pruned.styles === deadStyles.length,
  rawReduction: rawSaved > 0,
  gzipReduction: gzipSaved > 0,
  unusedThemeVariablesRemoved:
    frozen.themedPruned.customProperties.length < frozen.themedBaseline.customProperties.length,
  themedRawReduction: frozen.themedPruned.rawBytes < frozen.themedBaseline.rawBytes,
  themedGzipReduction: frozen.themedPruned.gzipBytes < frozen.themedBaseline.gzipBytes,
};
assert(Object.values(gates).every(Boolean), `pruning gate failed: ${JSON.stringify(gates)}`);
if (check) {
  const gzipHashes = expectedGzipSha256ByZlib[process.versions.zlib];
  assert(gzipHashes, `unreviewed zlib build ${process.versions.zlib}`);
  assertEqual(
    {
      baseline: frozen.baseline.gzipSha256,
      pruned: frozen.pruned.gzipSha256,
      themedBaseline: frozen.themedBaseline.gzipSha256,
      themedPruned: frozen.themedPruned.gzipSha256,
    },
    gzipHashes,
    "reviewed deterministic gzip hashes drifted",
  );
  const comparable = structuredClone(frozen);
  delete comparable.baseline.gzipSha256;
  delete comparable.pruned.gzipSha256;
  delete comparable.themedBaseline.gzipSha256;
  delete comparable.themedPruned.gzipSha256;
  assertEqual(comparable, expected, "reviewed reachability-pruning contract drifted");
}

const report = {
  schemaVersion: 1,
  claimScope: "controlled-application-union-pruning",
  generatedAt: new Date().toISOString(),
  runtime: { node: process.version, zlib: process.versions.zlib },
  inputs: {
    sourceSha256: frozen.sourceSha256,
    reachabilitySha256: frozen.reachabilitySha256,
    styles: fixtureStyles.length,
    routeRootStyles: 1,
    islandRootStyles: 1,
    deadStyles: deadStyles.length,
    themeEmission: "measured-in-separate-baseline-and-pruned-profiles",
    compilerSha256: sha256(readFileSync(executable)),
    compilerSource: process.env.PLIEGO_CSSC ? "PLIEGO_CSSC-prebuilt" : "target-debug",
  },
  profiles: {
    baseline: frozen.baseline,
    pruned: frozen.pruned,
    themedBaseline: frozen.themedBaseline,
    themedPruned: frozen.themedPruned,
  },
  delta: {
    rawBytesSaved: rawSaved,
    rawPercentSaved: Number(((rawSaved / frozen.baseline.rawBytes) * 100).toFixed(3)),
    gzipBytesSaved: gzipSaved,
    gzipPercentSaved: Number(((gzipSaved / frozen.baseline.gzipBytes) * 100).toFixed(3)),
  },
  gates,
};
console.log(JSON.stringify(report, null, 2));
