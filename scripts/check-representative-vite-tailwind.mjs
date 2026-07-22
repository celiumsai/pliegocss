import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative, resolve } from "node:path";
import { cargoTargetRoot } from "./rust-target.mjs";

const root = resolve(import.meta.dirname, "..");
const fixture = join(root, "integration-tests", "representative", "vite-tailwind-inventory");
const executable = join(cargoTargetRoot(root), "debug", process.platform === "win32" ? "pliego-cssc.exe" : "pliego-cssc");

function fail(message, detail) {
  throw new Error(detail === undefined ? message : `${message}\n${JSON.stringify(detail, null, 2)}`);
}
function run(command, args, cwd = root) {
  const result = spawnSync(command, args, { cwd, encoding: "utf8", windowsHide: true });
  if (result.error) throw result.error;
  if (result.status !== 0) fail(`command failed: ${command} ${args.join(" ")}`, result);
  return result;
}
function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}
function files(directory) {
  return readdirSync(directory, { withFileTypes: true })
    .sort((a, b) => a.name.localeCompare(b.name, "en"))
    .flatMap((entry) => {
      const path = join(directory, entry.name);
      return entry.isDirectory() ? files(path) : [path];
    });
}
function snapshot() {
  return Object.fromEntries(
    files(fixture).map((path) => [relative(fixture, path).replaceAll("\\", "/"), sha256(readFileSync(path))]),
  );
}

for (const name of ["index.html", "package.json", "src/app.css", "src/main.ts"]) {
  const path = join(fixture, name);
  if (!existsSync(path) || !statSync(path).isFile()) fail(`missing representative fixture file ${name}`);
}
run("cargo", ["build", "--quiet", "--locked", "-p", "pliego-cssc"]);
const before = snapshot();
const first = run(executable, ["migration-project-inventory", "."], fixture);
const second = run(executable, ["migration-project-inventory", "."], fixture);
if (first.stderr !== "" || second.stderr !== "" || first.stdout !== second.stdout || !first.stdout.endsWith("\n")) {
  fail("inventory output is not deterministic and clean", { first, second });
}
if (JSON.stringify(snapshot()) !== JSON.stringify(before)) fail("inventory mutated the representative application");
const inventory = JSON.parse(first.stdout);
const summary = inventory.summary;
for (const [field, expected] of Object.entries({
  sources: 1,
  tailwindSources: 1,
  auxiliaries: 1,
  tailwindTemplates: 1,
  staticTemplateCandidates: 46,
  dynamicTemplateCandidates: 0,
})) {
  if (summary?.[field] !== expected) fail(`summary.${field} drifted`, { actual: summary?.[field], expected });
}
const tailwind = inventory.sources?.find((source) => source.sourceKind === "tailwind");
if (tailwind?.file !== "src/app.css" || tailwind.preflightReliance !== "implicit") {
  fail("Tailwind source or Preflight classification drifted", tailwind);
}
const template = inventory.auxiliaries?.find((entry) => entry.auxiliaryKind === "tailwind-template");
if (template?.file !== "index.html") fail("Tailwind template ownership drifted", template);
const dynamic = template.observations?.filter((observation) => observation.dynamic === true) ?? [];
if (dynamic.length !== 0) fail("unexpected dynamic template seam", dynamic);
process.stdout.write(
  `${JSON.stringify(
    {
      schema: "pliegocss/representative-application/1",
      id: "vite-tailwind-inventory-first",
      passed: true,
      application: { files: Object.keys(before).length, snapshot: before },
      inventory: {
        bytes: Buffer.byteLength(first.stdout),
        sha256: sha256(Buffer.from(first.stdout)),
        summary,
        preflightReliance: tailwind.preflightReliance,
        dynamicTemplateCandidates: dynamic.length,
      },
      evidenceLimits: [
        "This gate inventories a bounded Vite-shaped Tailwind project without executing Vite or Tailwind.",
        "It does not prove semantic migration accuracy, generated CSS parity, or reversible codemods.",
      ],
    },
    null,
    2,
  )}\n`,
);
