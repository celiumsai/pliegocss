import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { repositoryRoot, sha256 } from "./benchmark-authority-v2.mjs";

const authorityPath = resolve(
  repositoryRoot,
  "benchmarks",
  "browser-output-certification-v1",
  "authority.json",
);
const authority = JSON.parse(readFileSync(authorityPath, "utf8"));

function fail(message) {
  throw new Error(`Browser/output authority: ${message}`);
}

function equal(actual, expected, label) {
  if (JSON.stringify(actual) !== JSON.stringify(expected)) fail(`${label} drifted`);
}

function nonEmpty(value, label) {
  if (typeof value !== "string" || value.trim() === "") fail(`${label} must be non-empty`);
}

if (authority.schemaVersion !== 1 || authority.status !== "active-contract") {
  fail("header drifted");
}
if (authority.kind !== "pliegocss-browser-output-certification") fail("kind drifted");
if (authority.competitorAuthority !== "benchmarks/benchmark-authority-v2/oracle.json") {
  fail("competitor authority path drifted");
}
if (authority.competitorLane !== "tailwind-latest") fail("competitor lane drifted");
if (authority.maximumEvidenceAgeHours !== 168) fail("evidence validity drifted");

for (const input of [authority.fixture, authority.reset]) {
  if (!/^[0-9a-f]{64}$/u.test(input.sha256)) fail(`${input.path} hash is invalid`);
  const bytes = readFileSync(resolve(repositoryRoot, input.path));
  if (sha256(bytes) !== input.sha256) fail(`${input.path} hash drifted`);
}
if (!readFileSync(resolve(repositoryRoot, authority.reset.path), "utf8").startsWith("@layer reset")) {
  fail("shared reset must be isolated in the lowest cascade layer");
}

equal(authority.resetModes.map((mode) => mode.id), ["no-reset", "shared-reset"], "reset modes");
authority.resetModes.forEach((mode) => nonEmpty(mode.contract, `${mode.id}.contract`));
equal(authority.browsers, ["chromium", "firefox", "webkit"], "browser engines");

const expectedHosts = [
  ["windows", "windows-latest", "win32", "x64"],
  ["linux", "ubuntu-24.04", "linux", "x64"],
  ["macos", "macos-15", "darwin", "arm64"],
].flatMap(([prefix, runner, os, arch]) =>
  authority.browsers.map((browser) => ({
    id: `${prefix}-${arch === "x64" ? "x64" : "arm64"}-${browser}`,
    runner,
    os,
    arch,
    browser,
  })),
);
equal(authority.hosts, expectedHosts, "host matrix");

if (
  !Array.isArray(authority.computedProperties) ||
  authority.computedProperties.length < 40 ||
  new Set(authority.computedProperties).size !== authority.computedProperties.length
) {
  fail("computed property set is incomplete or contains duplicates");
}
for (const required of [
  "display",
  "width",
  "padding-top",
  "grid-template-columns",
  "color",
  "background-color",
  "border-top-width",
  "outline-width",
  "font-size",
  "opacity",
]) {
  if (!authority.computedProperties.includes(required)) fail(`computed property ${required} is missing`);
}

equal(
  authority.scenarios.map(({ id, width, height, action }) => ({ id, width, height, action })),
  [
    { id: "base-mobile", width: 375, height: 812, action: "none" },
    { id: "base-tablet", width: 768, height: 1024, action: "none" },
    { id: "base-desktop", width: 1440, height: 900, action: "none" },
    { id: "hover-primary", width: 1440, height: 900, action: "hover" },
    { id: "focus-input", width: 1440, height: 900, action: "focus" },
  ],
  "scenario contract",
);
for (const scenario of authority.scenarios.filter((entry) => entry.action !== "none")) {
  nonEmpty(scenario.selector, `${scenario.id}.selector`);
}

equal(
  authority.requiredCoverage,
  [
    "computed-style",
    "layout-geometry",
    "responsive",
    "interactive-state",
    "reset-explicit",
    "screenshot-bounded",
    "tailwind-latest",
  ],
  "coverage contract",
);
if (authority.comparison.computedLengthQuantumCssPx !== 0.25) fail("length quantum drifted");
if (
  authority.comparison.layoutGeometryResolutionCssPx !== 0.015625 ||
  authority.comparison.maximumLayoutGeometryDeltaCssPx !== 0.1
) {
  fail("layout geometry precision or tolerance drifted");
}
if (authority.comparison.screenshotPixelThreshold !== 0.2) fail("pixel threshold drifted");
if (authority.comparison.maximumScreenshotMismatchRatio !== 0.005) {
  fail("screenshot mismatch budget drifted");
}
for (const field of ["computedStyle", "layoutGeometry", "screenshots", "claimBoundary"]) {
  nonEmpty(authority.comparison[field], `comparison.${field}`);
}

const packageJson = JSON.parse(readFileSync(resolve(repositoryRoot, "package.json"), "utf8"));
const expectedPackages = {
  playwright: "1.61.1",
  pixelmatch: "7.2.0",
  pngjs: "7.0.0",
};
for (const [name, version] of Object.entries(expectedPackages)) {
  if (packageJson.devDependencies?.[name] !== version) fail(`${name} must be pinned to ${version}`);
  const installed = JSON.parse(
    readFileSync(resolve(repositoryRoot, "node_modules", name, "package.json"), "utf8"),
  );
  if (installed.version !== version) fail(`installed ${name} version drifted`);
}

const runnerSource = readFileSync(
  resolve(repositoryRoot, "scripts", "browser-output-certification.mjs"),
  "utf8",
);
for (const marker of [
  "getComputedStyle",
  "getBoundingClientRect",
  "page.screenshot",
  "pixelmatch",
  "--require-clean",
  "authority.competitorLane",
]) {
  if (!runnerSource.includes(marker)) fail(`runner is missing ${marker}`);
}
const workflow = readFileSync(
  resolve(repositoryRoot, ".github", "workflows", "browser-output-certification.yml"),
  "utf8",
);
for (const host of authority.hosts) {
  if (!workflow.includes(`host: ${host.id}`)) fail(`workflow is missing ${host.id}`);
}
for (const marker of [
  "playwright install",
  "--require-clean",
  "check:browser-output-matrix",
  "actions/upload-artifact@",
  "actions/download-artifact@",
]) {
  if (!workflow.includes(marker)) fail(`workflow is missing ${marker}`);
}

process.stdout.write(
  `${JSON.stringify(
    {
      schemaVersion: 1,
      status: "passed",
      authoritySha256: sha256(readFileSync(authorityPath)),
      hosts: authority.hosts.length,
      browsers: authority.browsers,
      resetModes: authority.resetModes.map((mode) => mode.id),
      scenarios: authority.scenarios.length,
      computedProperties: authority.computedProperties.length,
    },
    null,
    2,
  )}\n`,
);
