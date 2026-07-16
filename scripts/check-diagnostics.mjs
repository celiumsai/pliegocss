import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const root = resolve(import.meta.dirname, "..");
const executable = join(
  root,
  "target",
  "release",
  process.platform === "win32" ? "pliego-cssc.exe" : "pliego-cssc",
);
const contract = JSON.parse(
  readFileSync(join(root, "benchmarks", "tailwind-v4", "mutations", "cases.json"), "utf8"),
);

const build = spawnSync("cargo", ["build", "--release", "--locked", "-p", "pliego-cssc"], {
  cwd: root,
  encoding: "utf8",
  windowsHide: true,
});
if (build.status !== 0) {
  throw new Error(build.stderr || "release build failed");
}

const results = contract.cases.map((testCase) => {
  const result = spawnSync(executable, ["compile", "--style", testCase.input], {
    cwd: root,
    encoding: "utf8",
    windowsHide: true,
  });
  const diagnostic = `${result.stdout}${result.stderr}`;
  if (result.status === 0) {
    throw new Error(`${testCase.id}: invalid input compiled successfully`);
  }
  if (!diagnostic.includes(testCase.pliegoCode)) {
    throw new Error(`${testCase.id}: expected ${testCase.pliegoCode}\n${diagnostic}`);
  }
  return {
    id: testCase.id,
    code: testCase.pliegoCode,
    failedClosed: true,
  };
});

process.stdout.write(`${JSON.stringify({ checked: results.length, results }, null, 2)}\n`);
