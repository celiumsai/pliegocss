import { spawnSync } from "node:child_process";
import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { resolve, sep } from "node:path";
import { runTests } from "@vscode/test-electron";

const EXTENSION = resolve(import.meta.dirname, "..");
const ROOT = resolve(EXTENSION, "..", "..");
const target = process.env.CARGO_TARGET_DIR
  ? resolve(ROOT, process.env.CARGO_TARGET_DIR)
  : resolve(ROOT, "target");
const workspace = resolve(target, "lsp-integration-workspace");
const executable = process.platform === "win32" ? ".exe" : "";
const lsp = resolve(target, "debug", `pliego-css-lsp${executable}`);
const compiler = resolve(target, "debug", `pliego-cssc${executable}`);

/** @param {string} message */
function fail(message) {
  throw new Error(message);
}

if (!workspace.startsWith(`${target}${sep}`)) fail("unsafe VS Code host fixture path");
const fixture = spawnSync(process.execPath, [resolve(ROOT, "scripts", "check-lsp-integration.mjs")], {
  cwd: ROOT,
  encoding: "utf8",
  env: { ...process.env, PLIEGOCSS_KEEP_LSP_FIXTURE: "1" },
  stdio: ["ignore", "pipe", "pipe"],
});
if (fixture.error) fail(`cannot prepare VS Code host fixture: ${fixture.error.message}`);
if (fixture.status !== 0) fail(`${fixture.stdout}${fixture.stderr}`.trim());

mkdirSync(resolve(workspace, ".vscode"), { recursive: true });
writeFileSync(
  resolve(workspace, ".vscode", "settings.json"),
  `${JSON.stringify(
    {
      "pliegocss.server.path": lsp,
      "pliegocss.compiler.path": compiler,
      "pliegocss.theme.mode": "seed",
      "pliegocss.projectIndex.path": "out/pliego.index.json",
    },
    null,
    2,
  )}\n`,
);

try {
  await runTests({
    version: "1.105.1",
    extensionDevelopmentPath: EXTENSION,
    extensionTestsPath: resolve(EXTENSION, "tests", "host", "index.js"),
    launchArgs: [
      workspace,
      "--disable-extensions",
      "--disable-workspace-trust",
      "--skip-welcome",
      "--skip-release-notes",
    ],
    extensionTestsEnv: {
      PLIEGOCSS_HOST_WORKSPACE: workspace,
    },
  });
} finally {
  rmSync(workspace, { recursive: true, force: true });
}

process.stdout.write(
  `${JSON.stringify({
    schemaVersion: 1,
    vscode: "1.105.1",
    extension: "celiums.pliegocss-vscode",
    diagnostics: "compiler-backed",
    definition: "project-index-to-physical-css",
    serverDownload: false,
  }, null, 2)}\n`,
);
