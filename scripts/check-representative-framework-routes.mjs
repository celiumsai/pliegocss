import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { cargoTargetRoot } from "./rust-target.mjs";

const root = resolve(import.meta.dirname, "..");
const fixture = join(root, "integration-tests", "representative", "framework-neutral-routes");
const reachabilityPath = join(fixture, ".reachability.generated.json");
const cli = join(cargoTargetRoot(root), "debug", process.platform === "win32" ? "pliego-cssc.exe" : "pliego-cssc");
function fail(message, detail) { throw new Error(detail === undefined ? message : `${message}\n${JSON.stringify(detail, null, 2)}`); }
function run(command, args, cwd = root) {
  const result = spawnSync(command, args, { cwd, encoding: "utf8", windowsHide: true });
  if (result.error) throw result.error;
  if (result.status !== 0) fail(`command failed: ${command} ${args.join(" ")}`, { status: result.status, stdout: result.stdout, stderr: result.stderr });
  return result;
}
function sha256(bytes) { return createHash("sha256").update(bytes).digest("hex"); }
function site(file) {
  const source = readFileSync(join(fixture, file), "utf8");
  const start = source.indexOf('pc!("');
  if (start < 0) fail(`missing pc! site in ${file}`);
  const end = source.indexOf('")', start);
  if (end < 0) fail(`unterminated pc! site in ${file}`);
  return { file, byteStart: Buffer.byteLength(source.slice(0, start)), byteEnd: Buffer.byteLength(source.slice(0, end + 2)) };
}
for (const name of ["pliego.bundles.toml", "src/global.rs", "src/home.rs", "src/visit.rs", "src/counter.rs"]) if (!existsSync(join(fixture, name))) fail(`missing ${name}`);
run("cargo", ["build", "--quiet", "--locked", "-p", "pliego-cssc"]);
const reachability = {
  schema: 1,
  applicationCoverage: "complete",
  components: [
    { id: "global", sites: [site("src/global.rs")] },
    { id: "home", sites: [site("src/home.rs")] },
    { id: "visit", sites: [site("src/visit.rs")] },
    { id: "counter", sites: [site("src/counter.rs")] },
  ],
  routes: [
    { id: "home", path: "/", components: ["global", "home"] },
    { id: "visit", path: "/visit", components: ["global", "visit"] },
  ],
  islands: [{ id: "counter", name: "visit-counter", components: ["counter"] }],
};
writeFileSync(reachabilityPath, `${JSON.stringify(reachability, null, 2)}\n`);
let output;
try {
  output = mkdtempSync(join(tmpdir(), "pliegocss-framework-neutral-routes-"));
  const args = [
    "bundle", "--plan", "pliego.bundles.toml", "--output-dir", output,
    "--manifest-version", "5", "--reachability", reachabilityPath,
    "--prune-unreachable", "--asset-plan", "--project-index", "--control",
  ];
  run(cli, args, fixture);
  const first = {};
  for (const name of ["global", "home", "visit", "counter"]) {
    for (const suffix of [".css", ".manifest.json", ".css.map"]) {
      const file = `${name}${suffix}`; const bytes = readFileSync(join(output, file)); first[file] = { bytes: bytes.length, sha256: sha256(bytes) };
    }
  }
  for (const file of ["pliego.assets.json", "pliego.index.json", "pliego.tokens.json", "pliego.css.findings.json", "pliego.css.manifest.json", "pliego.css.receipt.json"]) {
    const bytes = readFileSync(join(output, file)); first[file] = { bytes: bytes.length, sha256: sha256(bytes) };
  }
  run(cli, [...args, "--check"], fixture);
  for (const [file, identity] of Object.entries(first)) {
    const bytes = readFileSync(join(output, file));
    if (bytes.length !== identity.bytes || sha256(bytes) !== identity.sha256) fail(`artifact ${file} drifted under --check`);
  }
  const plan = JSON.parse(readFileSync(join(output, "pliego.assets.json"), "utf8"));
  const index = JSON.parse(readFileSync(join(output, "pliego.index.json"), "utf8"));

  if (plan.routes?.length !== 2 || plan.islands?.length !== 1 || plan.bundles?.length !== 4) fail("asset topology drifted", plan);
  if (index.schemaVersion !== 1 || index.documents?.length !== 4) fail("Project Index topology drifted", index);
  const home = plan.routes.find((route) => route.id === "route:home");
  const visit = plan.routes.find((route) => route.id === "route:visit");
  const island = plan.islands[0];
  if (JSON.stringify(home.bundles) !== JSON.stringify(["global", "home"])) fail("home selection drifted", home);
  if (JSON.stringify(visit.bundles) !== JSON.stringify(["global", "visit"])) fail("visit selection drifted", visit);
  if (JSON.stringify(island.bundles) !== JSON.stringify(["global", "counter"])) fail("island selection drifted", island);
  process.stdout.write(`${JSON.stringify({
    schema: "pliegocss/representative-application/1", id: "framework-neutral-multiroute", passed: true,
    topology: { routes: plan.routes, islands: plan.islands, bundles: plan.bundles.map((bundle) => bundle.id) },
    projectIndex: { schemaVersion: index.schemaVersion, sources: index.documents.length }, artifacts: first,
    externalEvidence: { pliegorsCheckout: "not-configured", browserReplay: "not-run" },
    evidenceLimits: [
      "This gate proves framework-neutral multiroute/island selection and deterministic controlled artifacts.",
      "PliegoRS was unavailable on this host, so no PliegoRS or browser claim is inferred."
    ]
  }, null, 2)}\n`);
} finally {
  rmSync(reachabilityPath, { force: true });
  if (output) rmSync(output, { recursive: true, force: true, maxRetries: 10, retryDelay: 50 });
}
