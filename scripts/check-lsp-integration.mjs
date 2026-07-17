import { spawn, spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { resolve } from "node:path";
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

const child = spawn(lsp, ["--pliego-cssc", compiler, "--seed"], {
  cwd: ROOT,
  stdio: ["pipe", "pipe", "pipe"],
});
const output = [];
const errors = [];
child.stdout.on("data", (chunk) => output.push(chunk));
child.stderr.on("data", (chunk) => errors.push(chunk));

function send(message) {
  const body = Buffer.from(JSON.stringify(message));
  child.stdin.write(`Content-Length: ${body.length}\r\n\r\n`);
  child.stdin.write(body);
}

const uri = pathToFileURL(resolve(ROOT, "lsp-smoke.rs")).href;
const rootUri = pathToFileURL(ROOT).href;
const text = 'fn view(){let _=pc!(" flex   gap-4 ");}';
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
send({ jsonrpc: "2.0", id: 5, method: "shutdown", params: null });
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
const published = messages.find((message) => message.method === "textDocument/publishDiagnostics");
if (byId.get(1)?.result?.capabilities?.positionEncoding !== "utf-16") {
  fail("initialize did not negotiate UTF-16");
}
if (published?.params?.diagnostics?.[0]?.code !== "FMT001") {
  fail("didOpen did not publish FMT001");
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
if (byId.get(5)?.result !== null) fail("shutdown did not return null");

process.stdout.write(
  `${JSON.stringify({
    schemaVersion: 1,
    positionEncoding: "utf-16",
    diagnostics: published.params.diagnostics.length,
    formattingEdits: byId.get(2).result.length,
    completion: item.label,
    completionRange: item.textEdit.range,
    hover: "compiler-css",
    exitCode,
  }, null, 2)}\n`,
);
