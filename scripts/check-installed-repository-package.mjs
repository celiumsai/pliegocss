import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, isAbsolute, relative, resolve } from "node:path";
import { ROOT, fail, repositoryVersion } from "./repository-distribution.mjs";

function parse(args) {
  const parsed = {};
  for (const arg of args) {
    const match = /^--([a-z-]+)=(.+)$/u.exec(arg);
    if (!match || Object.hasOwn(parsed, match[1])) fail(`invalid option: ${arg}`);
    parsed[match[1]] = match[2];
  }
  return parsed;
}

function run(command, args, cwd, { input } = {}) {
  const executable = process.platform === "win32" && command === "pnpm"
    ? process.env.ComSpec ?? "cmd.exe"
    : command;
  const commandArgs = process.platform === "win32" && command === "pnpm"
    ? ["/d", "/s", "/c", command, ...args]
    : args;
  const result = spawnSync(executable, commandArgs, {
    cwd,
    encoding: "utf8",
    input,
    timeout: 30_000,
    windowsHide: true,
  });
  if (result.error || result.status !== 0) {
    fail(
      `${command} ${args.join(" ")} failed: ${result.error?.message ?? `${result.stdout}${result.stderr}`.trim()}`,
    );
  }
  return `${result.stdout}${result.stderr}`.trim();
}

const options = parse(process.argv.slice(2));
if (!options.tarball) fail("--tarball is required");
const tarball = resolve(ROOT, options.tarball);
const version = repositoryVersion();
const temporaryRoot = mkdtempSync(resolve(tmpdir(), "pliegocss-pnpm-consumer-"));
try {
  writeFileSync(
    resolve(temporaryRoot, "package.json"),
    `${JSON.stringify(
      {
        name: "pliegocss-repository-distribution-consumer",
        version: "0.0.0",
        private: true,
        packageManager: "pnpm@11.7.0",
      },
      null,
      2,
    )}\n`,
  );
  writeFileSync(
    resolve(temporaryRoot, "pnpm-workspace.yaml"),
    "packages: []\nblockExoticSubdeps: true\nonlyBuiltDependencies: []\n",
  );
  run(
    "pnpm",
    ["add", "--save-dev", "--save-exact", "--offline", "--ignore-scripts", tarball],
    temporaryRoot,
  );
  const packageManifest = JSON.parse(readFileSync(resolve(temporaryRoot, "package.json"), "utf8"));
  const installed = packageManifest.devDependencies?.["@pliegocss/cli"];
  if (typeof installed !== "string" || !installed.startsWith("file:")) {
    fail("consumer did not record the verified local release asset");
  }
  const cliVersion = run("pnpm", ["exec", "pliego-cssc", "--version"], temporaryRoot);
  const lspVersion = run("pnpm", ["exec", "pliego-css-lsp", "--version"], temporaryRoot);
  if (!cliVersion.includes(version) || !lspVersion.includes(version)) {
    fail(`installed binaries do not report ${version}`);
  }
  process.stdout.write(
    `${JSON.stringify(
      {
        schemaVersion: 1,
        result: "passed",
        host: `${process.platform}:${process.arch}`,
        tarball: basename(tarball),
        version,
        install: "pnpm-local-verified-asset",
        scripts: "ignored-and-absent",
      },
      null,
      2,
    )}\n`,
  );
} finally {
  const local = relative(resolve(tmpdir()), temporaryRoot);
  if (!local || local.startsWith("..") || isAbsolute(local) || !basename(temporaryRoot).startsWith("pliegocss-pnpm-consumer-")) {
    fail(`refusing to remove unsafe consumer path: ${temporaryRoot}`);
  }
  rmSync(temporaryRoot, { recursive: true, force: true, maxRetries: 3, retryDelay: 100 });
}
