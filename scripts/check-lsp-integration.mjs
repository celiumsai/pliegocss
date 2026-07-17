import { spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { resolve, sep } from "node:path";
import { pathToFileURL } from "node:url";

const ROOT = resolve(import.meta.dirname, "..");
const executable = process.platform === "win32" ? ".exe" : "";
const target = process.env.CARGO_TARGET_DIR
  ? resolve(ROOT, process.env.CARGO_TARGET_DIR)
  : resolve(ROOT, "target");
const lsp = resolve(target, "debug", `pliego-css-lsp${executable}`);
const compiler = resolve(target, "debug", `pliego-cssc${executable}`);
const cancellationProxy = resolve(target, `lsp-cancellation-proxy${executable}`);
const diagnosticCorpus = JSON.parse(
  readFileSync(resolve(ROOT, "integration-tests", "lsp-diagnostics", "corpus.json"), "utf8"),
);

function fail(message) {
  throw new Error(message);
}

const build = spawnSync(
  "cargo",
  ["+1.85", "build", "--locked", "-p", "pliego-css-lsp", "-p", "pliego-cssc"],
  { cwd: ROOT, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] },
);
if (build.error) fail(`cannot build LSP gate: ${build.error.message}`);
if (build.status !== 0) fail(`${build.stdout}${build.stderr}`.trim());
if (!existsSync(lsp) || !existsSync(compiler)) fail("LSP gate binaries are missing");
const proxyBuild = spawnSync(
  "rustc",
  [
    "+1.85",
    resolve(ROOT, "integration-tests", "lsp-diagnostics", "cancellation-proxy.rs"),
    "-o",
    cancellationProxy,
  ],
  { cwd: ROOT, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] },
);
if (proxyBuild.error) fail(`cannot build cancellation proxy: ${proxyBuild.error.message}`);
if (proxyBuild.status !== 0) fail(`${proxyBuild.stdout}${proxyBuild.stderr}`.trim());
if (!existsSync(cancellationProxy)) fail("LSP cancellation proxy is missing");
if (
  diagnosticCorpus.kind !== "pliegocss-lsp-diagnostic-corpus" ||
  diagnosticCorpus.schemaVersion !== 2 ||
  !Array.isArray(diagnosticCorpus.cases)
) {
  fail("unsupported LSP diagnostic corpus schema");
}
const expectedCorpusCodes = [
  ...Array.from({ length: 12 }, (_, index) => `PCS${String(index + 1).padStart(3, "0")}`),
  ...Array.from({ length: 6 }, (_, index) => `PSC${String(index + 1).padStart(3, "0")}`),
  "PCR001",
  "FMT001",
];
const corpusIds = diagnosticCorpus.cases.map((item) => item.id);
const corpusCodes = [...new Set(diagnosticCorpus.cases.map((item) => item.code))].sort();
if (new Set(corpusIds).size !== corpusIds.length) fail("diagnostic corpus ids must be unique");
if (JSON.stringify(corpusCodes) !== JSON.stringify([...expectedCorpusCodes].sort())) {
  fail(`diagnostic corpus code coverage drifted: ${corpusCodes.join(", ")}`);
}

const workspace = resolve(target, "lsp-integration-workspace");
if (!workspace.startsWith(`${target}${sep}`)) fail("unsafe LSP fixture path");
rmSync(workspace, { recursive: true, force: true });
mkdirSync(resolve(workspace, "src"), { recursive: true });
mkdirSync(resolve(workspace, "out"), { recursive: true });
const text = 'fn view(){let _=pc!(" flex   gap-4 ");}';
const literalStart = text.indexOf('" flex');
const literalEnd = text.indexOf('"', literalStart + 1) + 1;
const css = Buffer.from(".pc{display:flex}\n");
const hash = (bytes) => createHash("sha256").update(bytes).digest("hex");
const manifest = Buffer.from(
  JSON.stringify({
    schemaVersion: 5,
    cssSha256: hash(css),
    cssBytes: css.length,
    themeId: "seed",
    graph: {
      schemaVersion: 2,
      physicalDeclarationIdFormatVersion: 1,
      physicalCoverage: "compiler-verified-complete",
      declarations: [],
      physicalDeclarations: [
        {
          id: "css-decl:00000000:00000000",
          ordinal: 0,
          property: "display",
          important: false,
          generated: false,
          byteStart: 4,
          byteEnd: 16,
          propertyByteStart: 4,
          propertyByteEnd: 11,
          valueByteStart: 12,
          valueByteEnd: 16,
        },
      ],
    },
  }),
);
const index = {
  schemaVersion: 1,
  sourceSiteIdFormatVersion: 1,
  manifestSchemaVersion: 5,
  graphSchemaVersion: 2,
  declarationIdFormatVersion: 1,
  physicalRuleIdFormatVersion: 1,
  physicalDeclarationIdFormatVersion: 1,
  originCoverage: "compiler-verified-complete",
  applicationCoverage: "adapter-attested-complete",
  physicalCoverage: "compiler-verified-complete",
  styleIdFormatVersion: 2,
  classNameFormatVersion: 1,
  themeIdFormatVersion: 1,
  themeId: "seed",
  targets: "modern",
  format: "minified",
  ruleSelection: "all-compiled",
  assetPlanFile: "pliego.assets.json",
  assetPlanBytes: 1,
  assetPlanSha256: hash("x"),
  documents: [
    {
      id: "document:one",
      path: "src/view.rs",
      bytes: Buffer.byteLength(text),
      sha256: hash(text),
      siteIds: ["site:one"],
    },
  ],
  bundles: [
    {
      id: "app",
      cssFile: "app.css",
      manifestFile: "app.manifest.json",
      emitsTheme: false,
      cssBytes: css.length,
      cssSha256: hash(css),
      manifestBytes: manifest.length,
      manifestSha256: hash(manifest),
      siteIds: ["site:one"],
    },
  ],
  sites: [
    {
      id: "site:one",
      documentId: "document:one",
      path: "src/view.rs",
      byteStart: literalStart,
      byteEnd: literalEnd,
      macroKind: "pc",
      reason: "visible-literal",
      source: "flex gap-4",
      styleId: "style",
      className: "pc_one",
      bundleIds: ["app"],
      declarationIds: ["decl:one"],
      tokenIds: [],
      componentIds: ["component:one"],
      physicalDeclarations: [
        { bundleId: "app", id: "css-decl:00000000:00000000" },
      ],
    },
  ],
};
writeFileSync(resolve(workspace, "src", "view.rs"), text);
writeFileSync(resolve(workspace, "out", "app.css"), css);
writeFileSync(resolve(workspace, "out", "app.manifest.json"), manifest);
writeFileSync(resolve(workspace, "out", "pliego.index.json"), JSON.stringify(index));
writeFileSync(resolve(workspace, "out", "pliego.assets.json"), "x");

const cliCorpusFindings = new Map();
for (const item of diagnosticCorpus.cases) {
  if (!/^[a-z0-9-]+$/.test(item.id) || !["style", "source", "format"].includes(item.kind)) {
    fail("diagnostic corpus contains an invalid id or kind");
  }
  const argumentsList = ["--diagnostic-format", "json"];
  if (item.kind === "style") {
    argumentsList.push("check");
    argumentsList.push("--style", item.style);
  } else if (item.kind === "source") {
    argumentsList.push("check");
    const sourcePath = resolve(workspace, `${item.id}.rs`);
    writeFileSync(sourcePath, item.source);
    argumentsList.push("--source", sourcePath);
  } else {
    argumentsList.push("fmt");
    const sourcePath = resolve(workspace, `${item.id}.rs`);
    writeFileSync(sourcePath, item.source);
    argumentsList.push("--source", sourcePath, "--check");
  }
  if (item.kind !== "format") argumentsList.push("--seed");
  const checked = spawnSync(compiler, argumentsList, {
    cwd: workspace,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
  if (checked.error || checked.status === 0) {
    fail(`CLI diagnostic corpus case ${item.id} did not fail as required`);
  }
  const document = JSON.parse(checked.stderr);
  if (document.schemaVersion !== 1 || document.diagnostics?.length !== 1) {
    fail(`CLI diagnostic corpus case ${item.id} returned an unsupported envelope`);
  }
  const finding = document.diagnostics[0];
  cliCorpusFindings.set(item.id, finding);
  if (finding.code !== item.code || finding.message !== item.message) {
    fail(`CLI diagnostic corpus case ${item.id} drifted`);
  }
  const range = item.kind === "style" ? finding.styleRange : finding.range;
  const expectedStart = item.kind === "style" ? item.styleStart : item.sourceStart;
  const expectedEnd = item.kind === "style" ? item.styleEnd : item.sourceEnd;
  if (range?.byteStart !== expectedStart || range?.byteEnd !== expectedEnd) {
    fail(`CLI diagnostic corpus range ${item.id} drifted`);
  }
}

const child = spawn(
  lsp,
  ["--pliego-cssc", cancellationProxy, "--seed", "--project-index", "out/pliego.index.json"],
  {
    cwd: workspace,
    stdio: ["pipe", "pipe", "pipe"],
    env: {
      ...process.env,
      PLIEGOCSS_REAL_COMPILER: compiler,
      PLIEGOCSS_CANCELLATION_MARKER: resolve(workspace, "cancellation.pid"),
    },
  },
);
const output = [];
const errors = [];
child.stdout.on("data", (chunk) => output.push(chunk));
child.stderr.on("data", (chunk) => errors.push(chunk));

function send(message) {
  const body = Buffer.from(JSON.stringify(message));
  child.stdin.write(`Content-Length: ${body.length}\r\n\r\n`);
  child.stdin.write(body);
}

async function waitForOutput(fragment, offset = 0, timeoutMs = 5000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (Buffer.concat(output).subarray(offset).includes(Buffer.from(fragment))) return;
    await new Promise((resolveDelay) => setTimeout(resolveDelay, 25));
  }
  fail(`LSP did not emit ${JSON.stringify(fragment)} within ${timeoutMs} ms`);
}

async function waitForFile(path, timeoutMs = 5000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (existsSync(path)) return;
    await new Promise((resolveDelay) => setTimeout(resolveDelay, 10));
  }
  fail(`LSP cancellation child did not create ${path} within ${timeoutMs} ms`);
}

function processIsAlive(pid) {
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    if (error?.code === "ESRCH") return false;
    throw error;
  }
}

const uri = pathToFileURL(resolve(workspace, "src", "view.rs")).href;
const rootUri = pathToFileURL(workspace).href;
const cursor = text.indexOf("gap") + 3;

send({ jsonrpc: "2.0", id: 1, method: "initialize", params: { rootUri } });
send({ jsonrpc: "2.0", method: "initialized", params: {} });
send({
  jsonrpc: "2.0",
  method: "textDocument/didOpen",
  params: { textDocument: { uri, languageId: "rust", version: 1, text } },
});
send({
  jsonrpc: "2.0",
  id: 2,
  method: "textDocument/formatting",
  params: { textDocument: { uri }, options: { tabSize: 4, insertSpaces: true } },
});
send({
  jsonrpc: "2.0",
  id: 3,
  method: "textDocument/completion",
  params: { textDocument: { uri }, position: { line: 0, character: cursor } },
});
send({
  jsonrpc: "2.0",
  id: 4,
  method: "textDocument/hover",
  params: { textDocument: { uri }, position: { line: 0, character: cursor } },
});
send({
  jsonrpc: "2.0",
  id: 5,
  method: "textDocument/definition",
  params: { textDocument: { uri }, position: { line: 0, character: cursor } },
});
for (let version = 2; version < 10; version += 1) {
  send({
    jsonrpc: "2.0",
    method: "textDocument/didChange",
    params: {
      textDocument: { uri, version },
      contentChanges: [{ text: `fn view(){let _=pc!("flex stale-${version}");}` }],
    },
  });
}
const finalVersion = 10;
const invalidText = 'fn view(){let _=pc!("flex unknown-thing");}';
send({
  jsonrpc: "2.0",
  method: "textDocument/didChange",
  params: {
    textDocument: { uri, version: finalVersion },
    contentChanges: [{ text: invalidText }],
  },
});
send({
  jsonrpc: "2.0",
  id: 6,
  method: "textDocument/formatting",
  params: { textDocument: { uri }, options: { tabSize: 4, insertSpaces: true } },
});
await waitForOutput("unknown utility `unknown-thing`");
const cancellationMarker = resolve(workspace, "cancellation.pid");
const cancellationVersion = 11;
send({
  jsonrpc: "2.0",
  method: "textDocument/didChange",
  params: {
    textDocument: { uri, version: cancellationVersion },
    contentChanges: [{ text: 'fn view(){let _=pc!("cancel-me");}' }],
  },
});
await waitForFile(cancellationMarker);
const cancelledPid = Number(readFileSync(cancellationMarker, "utf8").trim());
if (!Number.isSafeInteger(cancelledPid) || cancelledPid <= 0) {
  fail("cancellation proxy wrote an invalid process id");
}
const recoveryVersion = 12;
const recoveryText = 'fn view(){let _=pc!("unknown-after-cancel");}';
const recoveryOffset = Buffer.concat(output).length;
const cancellationStarted = Date.now();
send({
  jsonrpc: "2.0",
  method: "textDocument/didChange",
  params: {
    textDocument: { uri, version: recoveryVersion },
    contentChanges: [{ text: recoveryText }],
  },
});
await waitForOutput("unknown utility `unknown-after-cancel`", recoveryOffset, 5000);
const cancellationLatencyMs = Date.now() - cancellationStarted;
const processDeadline = Date.now() + 2000;
while (Date.now() < processDeadline && processIsAlive(cancelledPid)) {
  await new Promise((resolveDelay) => setTimeout(resolveDelay, 10));
}
if (processIsAlive(cancelledPid)) fail("stale semantic compiler child remained alive");

const pcxVersion = 13;
const pcxText =
  'fn view(){let _=pcx!("flex",if a{"opacity-50"}else{"block"},if b{"opacity-50"}else{"grid"});}';
const pcxSourcePath = resolve(workspace, "pcx-parity.rs");
writeFileSync(pcxSourcePath, pcxText);
const pcxChecked = spawnSync(
  compiler,
  ["--diagnostic-format", "json", "check", "--source", pcxSourcePath, "--seed"],
  { cwd: workspace, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] },
);
if (pcxChecked.error || pcxChecked.status === 0) {
  fail("CLI PCX003 parity fixture did not fail as required");
}
const pcxCliFinding = JSON.parse(pcxChecked.stderr).diagnostics?.[0];
if (pcxCliFinding?.code !== "PCX003") fail("CLI PCX003 parity fixture drifted");
send({
  jsonrpc: "2.0",
  method: "textDocument/didChange",
  params: {
    textDocument: { uri, version: pcxVersion },
    contentChanges: [{ text: pcxText }],
  },
});
await waitForOutput('"code":"PCX003"');
const corpusRuns = [];
let corpusVersion = 14;
for (const item of diagnosticCorpus.cases) {
  const sourceText =
    item.kind === "style" ? `fn view(){let _=pc!("${item.style}");}` : item.source;
  const offset = Buffer.concat(output).length;
  send({
    jsonrpc: "2.0",
    method: "textDocument/didChange",
    params: {
      textDocument: { uri, version: corpusVersion },
      contentChanges: [{ text: sourceText }],
    },
  });
  await waitForOutput(`"code":"${item.code}"`, offset);
  corpusRuns.push({ item, sourceText, version: corpusVersion });
  corpusVersion += 1;
}
const toolFailureVersion = corpusVersion;
const toolFailureOffset = Buffer.concat(output).length;
send({
  jsonrpc: "2.0",
  method: "textDocument/didChange",
  params: {
    textDocument: { uri, version: toolFailureVersion },
    contentChanges: [{ text: 'fn view(){let _=pc!("proxy-invalid-json");}' }],
  },
});
await waitForOutput('"code":"PCL001"', toolFailureOffset);

const literalLimitVersion = toolFailureVersion + 1;
const literalLimitText = `fn view(){${'let _=pc!("flex");'.repeat(257)}}`;
const literalLimitOffset = Buffer.concat(output).length;
send({
  jsonrpc: "2.0",
  method: "textDocument/didChange",
  params: {
    textDocument: { uri, version: literalLimitVersion },
    contentChanges: [{ text: literalLimitText }],
  },
});
await waitForOutput('"code":"PCL002"', literalLimitOffset);
send({ jsonrpc: "2.0", id: 7, method: "shutdown", params: null });
send({ jsonrpc: "2.0", method: "exit", params: null });
child.stdin.end();

const exitCode = await new Promise((resolveExit, reject) => {
  child.once("error", reject);
  child.once("close", resolveExit);
});
if (exitCode !== 0) fail(`LSP exited ${exitCode}: ${Buffer.concat(errors).toString()}`);
if (errors.length > 0) fail(`LSP wrote stderr: ${Buffer.concat(errors).toString()}`);

function decodeFrames(bytes) {
  const messages = [];
  let offset = 0;
  while (offset < bytes.length) {
    const boundary = bytes.indexOf("\r\n\r\n", offset, "ascii");
    if (boundary < 0) fail("truncated LSP response headers");
    const header = bytes.subarray(offset, boundary).toString("ascii");
    const match = /^Content-Length:\s*(\d+)$/im.exec(header);
    if (!match) fail("LSP response is missing Content-Length");
    const length = Number(match[1]);
    const start = boundary + 4;
    const end = start + length;
    if (end > bytes.length) fail("truncated LSP response body");
    messages.push(JSON.parse(bytes.subarray(start, end).toString("utf8")));
    offset = end;
  }
  return messages;
}

const messages = decodeFrames(Buffer.concat(output));
const byId = new Map(messages.filter((message) => "id" in message).map((message) => [message.id, message]));
const published = messages.filter(
  (message) => message.method === "textDocument/publishDiagnostics",
);
if (byId.get(1)?.result?.capabilities?.positionEncoding !== "utf-16") {
  fail("initialize did not negotiate UTF-16");
}
if (byId.get(1)?.result?.capabilities?.definitionProvider !== true) {
  fail("initialize did not advertise configured Project Index navigation");
}
if (published[0]?.params?.diagnostics?.[0]?.code !== "FMT001") {
  fail("didOpen did not publish FMT001");
}
const semanticMessages = published.filter(
  (message) =>
    message.params?.version === finalVersion &&
    message.params?.diagnostics?.some((diagnostic) => diagnostic.code === "PCS001"),
);
if (semanticMessages.length !== 1 || semanticMessages[0].params.version !== finalVersion) {
  fail(
    `debounce published a stale or duplicate semantic diagnostic result: ${JSON.stringify(
      semanticMessages.map((message) => ({
        version: message.params?.version,
        diagnostics: message.params?.diagnostics,
      })),
    )}`,
  );
}
const semantic = semanticMessages[0].params.diagnostics.find(
  (diagnostic) => diagnostic.code === "PCS001",
);
if (semantic?.message !== "unknown utility `unknown-thing`") {
  fail("didChange did not preserve the compiler semantic diagnostic");
}
if (
  semantic.range?.start?.character !== invalidText.indexOf("unknown-thing") ||
  semantic.range?.end?.character !== invalidText.indexOf("unknown-thing") + "unknown-thing".length
) {
  fail("compiler semantic diagnostic did not map to the exact Rust source range");
}
const pcxMessages = published.filter(
  (message) =>
    message.params?.version === pcxVersion &&
    message.params?.diagnostics?.some((diagnostic) => diagnostic.code === "PCX003"),
);
if (pcxMessages.length !== 1 || pcxMessages[0].params.version !== pcxVersion) {
  fail("pcx cross-clause diagnostics were stale, missing, or duplicated");
}
const pcxDiagnostic = pcxMessages[0].params.diagnostics.find(
  (diagnostic) => diagnostic.code === "PCX003",
);
if (!pcxDiagnostic?.message?.includes("independent clauses 1 and 2")) {
  fail("pcx diagnostic did not preserve the compiler message");
}
if (pcxDiagnostic.message !== pcxCliFinding.message || pcxDiagnostic.severity !== 1) {
  fail("pcx diagnostic message or severity diverged from CLI schema 1");
}
for (const field of ["category", "suggestion", "replacement"]) {
  if (
    JSON.stringify(pcxDiagnostic.data?.[field] ?? null) !==
    JSON.stringify(pcxCliFinding[field] ?? null)
  ) {
    fail(`pcx diagnostic ${field} diverged from CLI schema 1`);
  }
}
if (
  pcxDiagnostic.range?.start?.character !== pcxText.lastIndexOf('"opacity-50"') ||
  pcxDiagnostic.range?.end?.character !==
    pcxText.lastIndexOf('"opacity-50"') + '"opacity-50"'.length
) {
  fail(
    `pcx diagnostic did not map to the exact conflicting branch literal: ${JSON.stringify(
      pcxDiagnostic.range,
    )}`,
  );
}
const cancelledVersionMessages = published.filter(
  (message) =>
    message.params?.version === cancellationVersion &&
    message.params?.diagnostics?.some((diagnostic) => diagnostic.code === "PCL001"),
);
if (cancelledVersionMessages.length !== 0) {
  fail("cancelled semantic process leaked a tooling diagnostic");
}
const recoveryMessages = published.filter(
  (message) =>
    message.params?.version === recoveryVersion &&
    message.params?.diagnostics?.some((diagnostic) => diagnostic.code === "PCS001"),
);
if (recoveryMessages.length !== 1) {
  fail("semantic worker did not recover after forceful cancellation");
}
for (const run of corpusRuns) {
  const diagnostics = published
    .filter((message) => message.params?.version === run.version)
    .flatMap((message) => message.params?.diagnostics ?? []);
  const codes = [...new Set(diagnostics.map((diagnostic) => diagnostic.code))];
  if (codes.length !== 1 || codes[0] !== run.item.code) {
    fail(`LSP diagnostic corpus case ${run.item.id} emitted ${codes.join(", ")}`);
  }
  const finding = diagnostics.find((diagnostic) => diagnostic.code === run.item.code);
  const cliFinding = cliCorpusFindings.get(run.item.id);
  if (finding?.message !== run.item.message) {
    fail(`LSP diagnostic corpus message ${run.item.id} drifted`);
  }
  const severity = { error: 1, warning: 2, information: 3, hint: 4 }[cliFinding.severity];
  if (finding.severity !== severity) {
    fail(`LSP diagnostic corpus severity ${run.item.id} drifted`);
  }
  for (const field of ["category", "suggestion", "replacement"]) {
    if (JSON.stringify(finding.data?.[field] ?? null) !== JSON.stringify(cliFinding[field] ?? null)) {
      fail(`LSP diagnostic corpus ${field} ${run.item.id} drifted`);
    }
  }
  const relativeStart =
    run.item.kind === "style" ? run.item.styleStart : run.item.sourceStart;
  const relativeEnd = run.item.kind === "style" ? run.item.styleEnd : run.item.sourceEnd;
  const contentStart =
    run.item.kind === "style" ? run.sourceText.indexOf(`"${run.item.style}"`) + 1 : 0;
  if (
    finding.range?.start?.line !== 0 ||
    finding.range?.end?.line !== 0 ||
    finding.range?.start?.character !== contentStart + relativeStart ||
    finding.range?.end?.character !== contentStart + relativeEnd
  ) {
    fail(`LSP diagnostic corpus range ${run.item.id} drifted`);
  }
}
const toolFailureDiagnostics = published
  .filter((message) => message.params?.version === toolFailureVersion)
  .flatMap((message) => message.params?.diagnostics ?? []);
const toolFailure = toolFailureDiagnostics.find((diagnostic) => diagnostic.code === "PCL001");
if (
  toolFailure?.severity !== 1 ||
  toolFailure?.data?.category !== "tool" ||
  !toolFailure.message.includes("invalid pliego-cssc diagnostic JSON")
) {
  fail("PCL001 fault-injection contract drifted");
}
const literalLimitDiagnostics = published
  .filter((message) => message.params?.version === literalLimitVersion)
  .flatMap((message) => message.params?.diagnostics ?? []);
const literalLimit = literalLimitDiagnostics.find((diagnostic) => diagnostic.code === "PCL002");
if (
  literalLimit?.severity !== 1 ||
  literalLimit?.data?.category !== "tool" ||
  literalLimit.message !== "semantic diagnostic literal limit exceeded"
) {
  fail("PCL002 bounded-limit contract drifted");
}
if (byId.get(2)?.result?.[0]?.newText !== '"flex gap-4"') {
  fail("formatting did not return the canonical whole literal");
}
const item = byId.get(3)?.result?.items?.find((candidate) => candidate.label === "gap-4");
if (!item || item.textEdit.range.start.character !== text.indexOf("gap")) {
  fail("completion did not replace the utility from its exact start");
}
if (item.textEdit.range.end.character !== text.indexOf("gap-4") + "gap-4".length) {
  fail("completion did not replace the complete existing utility");
}
if (!byId.get(4)?.result?.contents?.value?.includes("```css")) {
  fail("hover did not include compiler-emitted CSS");
}
const definition = byId.get(5)?.result?.[0];
if (!definition?.targetUri?.endsWith("/out/app.css")) {
  fail("definition did not target the integrity-bound stylesheet");
}
if (
  definition.targetRange?.start?.character !== 4 ||
  definition.targetRange?.end?.character !== 16
) {
  fail("definition did not select the physical declaration range");
}
const responsiveIndex = messages.findIndex((message) => message.id === 6);
const semanticIndex = messages.indexOf(semanticMessages[0]);
if (responsiveIndex < 0 || semanticIndex < 0 || responsiveIndex >= semanticIndex) {
  fail("semantic diagnostics blocked the protocol request loop");
}
if (byId.get(7)?.result !== null) fail("shutdown did not return null");

if (process.env.PLIEGOCSS_KEEP_LSP_FIXTURE !== "1") {
  rmSync(workspace, { recursive: true, force: true });
}

process.stdout.write(
  `${JSON.stringify({
    schemaVersion: 1,
    positionEncoding: "utf-16",
    formatDiagnostics: published[0].params.diagnostics.length,
    semanticDiagnostic: semantic.code,
    semanticRange: semantic.range,
    semanticVersion: semanticMessages[0].params.version,
    staleSemanticResults: 0,
    protocolResponsiveDuringDebounce: true,
    forcefulCancellation: true,
    cancelledCompilerPid: cancelledPid,
    cancellationRecoveryMs: cancellationLatencyMs,
    pcxDiagnostic: pcxDiagnostic.code,
    pcxVersion: pcxMessages[0].params.version,
    pcxRange: pcxDiagnostic.range,
    diagnosticCorpusSchema: diagnosticCorpus.schemaVersion,
    diagnosticCorpusCases: corpusRuns.length,
    diagnosticCorpusEquality: "code-message-range-severity-suggestion-replacement",
    typedDiagnosticCodes: [...expectedCorpusCodes, "PCX003"],
    operationalFallbacks: [toolFailure.code, literalLimit.code],
    formattingEdits: byId.get(2).result.length,
    completion: item.label,
    completionRange: item.textEdit.range,
    hover: "compiler-css",
    definition: "project-index-to-physical-css",
    exitCode,
  }, null, 2)}\n`,
);
