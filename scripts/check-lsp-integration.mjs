import { spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { resolve, sep } from "node:path";
import { pathToFileURL } from "node:url";

const ROOT = resolve(import.meta.dirname, "..");
const executable = process.platform === "win32" ? ".exe" : "";
const target = process.env.CARGO_TARGET_DIR
  ? resolve(ROOT, process.env.CARGO_TARGET_DIR)
  : resolve(ROOT, "target");
const lsp = resolve(target, "debug", `pliego-css-lsp${executable}`);
const compiler = resolve(target, "debug", `pliego-cssc${executable}`);

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

const child = spawn(
  lsp,
  ["--pliego-cssc", compiler, "--seed", "--project-index", "out/pliego.index.json"],
  {
    cwd: workspace,
    stdio: ["pipe", "pipe", "pipe"],
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
await new Promise((resolveDelay) => setTimeout(resolveDelay, 500));
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
const semanticMessages = published.filter((message) =>
  message.params?.diagnostics?.some((diagnostic) => diagnostic.code === "PCS001"),
);
if (semanticMessages.length !== 1 || semanticMessages[0].params.version !== finalVersion) {
  fail("debounce published a stale or duplicate semantic diagnostic result");
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
    formattingEdits: byId.get(2).result.length,
    completion: item.label,
    completionRange: item.textEdit.range,
    hover: "compiler-css",
    definition: "project-index-to-physical-css",
    exitCode,
  }, null, 2)}\n`,
);
