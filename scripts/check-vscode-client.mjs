import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync, statSync } from "node:fs";
import { resolve } from "node:path";

const ROOT = resolve(import.meta.dirname, "..");
const EXTENSION = resolve(ROOT, "editors", "vscode");
const VSIX = resolve(ROOT, "target", "pliegocss-vscode-0.0.0.vsix");
const pnpmScript = process.env.npm_execpath;
const fallbackPnpm = [
  "C:/Program Files/nodejs/node_modules/npm/bin/npm-cli.js",
  "exec", "--yes", "--package=pnpm@10.14.0", "--", "pnpm",
];

function fail(message) {
  throw new Error(message);
}

function run(args, cwd = ROOT) {
  const command = pnpmScript ? process.execPath : process.execPath;
  const commandArgs = pnpmScript ? [pnpmScript, ...args] : [...fallbackPnpm, ...args];
  const result = spawnSync(command, commandArgs, {
    cwd,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
    shell: false,
  });
  if (result.error) fail(`cannot run pnpm: ${result.error.message}`);
  if (result.status !== 0) fail(`${result.stdout}${result.stderr}`.trim());
  return result.stdout;
}

run(["--filter", "pliegocss-vscode", "check"]);
const files = run(["exec", "vsce", "ls", "--no-dependencies"], EXTENSION)
  .trim()
  .split(/\r?\n/)
  .filter(Boolean)
  .sort();
const expected = ["LICENSE", "README.md", "dist/extension.js", "package.json"].sort();
if (JSON.stringify(files) !== JSON.stringify(expected)) {
  fail(`unexpected VSIX file set: ${JSON.stringify(files)}`);
}
run(["--filter", "pliegocss-vscode", "package"]);
const bytes = readFileSync(VSIX);
if (bytes.length > 256 * 1024) fail(`VSIX exceeds 256 KiB: ${bytes.length}`);
if (bytes[0] !== 0x50 || bytes[1] !== 0x4b) fail("VSIX is not a ZIP archive");

const manifest = JSON.parse(readFileSync(resolve(EXTENSION, "package.json"), "utf8"));
if (manifest.main !== "./dist/extension.js") fail("extension main is not the bundled client");
if (manifest.capabilities?.virtualWorkspaces?.supported !== false) {
  fail("virtual workspaces must fail closed");
}
if (manifest.capabilities?.untrustedWorkspaces?.supported !== false) {
  fail("untrusted workspaces must fail closed");
}
const bundle = readFileSync(resolve(EXTENSION, "dist", "extension.js"), "utf8");
for (const contract of ["pliego-css-lsp", "pliego-cssc", "onDidChangeConfiguration"]) {
  if (!bundle.includes(contract)) fail(`bundle is missing ${contract}`);
}

process.stdout.write(
  `${JSON.stringify({
    schemaVersion: 1,
    extension: `${manifest.publisher}.${manifest.name}`,
    version: manifest.version,
    files,
    bundleBytes: statSync(resolve(EXTENSION, "dist", "extension.js")).size,
    vsixBytes: bytes.length,
    vsixSha256: createHash("sha256").update(bytes).digest("hex"),
    serverDownload: false,
    virtualWorkspaces: false,
    untrustedWorkspaces: false,
  }, null, 2)}\n`,
);
