import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { repositoryRoot } from "./benchmark-authority-v2.mjs";

const authority = JSON.parse(
  readFileSync(
    resolve(repositoryRoot, "benchmarks", "browser-output-certification-v1", "authority.json"),
    "utf8",
  ),
);
const outputPath = resolve(
  repositoryRoot,
  "docs",
  "benchmarks",
  "browser-output-certification-v1.md",
);

function tableRows() {
  return authority.hosts
    .map(
      (host) =>
        `| \`${host.id}\` | \`${host.runner}\` | ${host.os} | ${host.arch} | ${host.browser} |`,
    )
    .join("\n");
}

const document = `# Browser/output certification v1

Status: **implemented local harness; hosted 3×3 evidence pending**

This page is generated from
[\`authority.json\`](../../benchmarks/browser-output-certification-v1/authority.json). Edit the
JSON contract and run \`node scripts/render-browser-output-certification.mjs --write\`; do not
maintain the matrix by hand.

## Scope

The certification compiles the frozen medium fixture once with PliegoCSS and once with the fresh
\`${authority.competitorLane}\` lane from Benchmark Authority v2. Within each host it compares
${authority.computedProperties.length} computed properties on every styled node and captures both
original screenshots plus a pixel-diff image for every scenario.

Passing proves only the frozen shared utility intent. It is not general Tailwind compatibility,
does not compare pixels across different browser engines, and does not broaden PliegoCSS's utility
catalog.

## Reset contract

| Mode | Contract |
|---|---|
${authority.resetModes.map((mode) => `| \`${mode.id}\` | ${mode.contract} |`).join("\n")}

The shared reset is isolated in \`@layer reset\`. Tailwind Preflight is not silently enabled because
PliegoCSS does not own a Preflight implementation. This keeps the reset/no-reset comparison
explicit and prevents an unlayered reset from outranking Tailwind's layered utilities.

## Hosted matrix

| Host | GitHub runner | OS | Architecture | Browser engine |
|---|---|---|---|---|
${tableRows()}

The matrix is the full browser/OS cross-product for Chromium, Firefox, and WebKit across Windows,
Linux, and macOS. Windows and Linux provide x64 evidence; the standard \`macos-15\` runner provides
ARM64 evidence.

## Scenarios

| Scenario | Viewport | State |
|---|---:|---|
${authority.scenarios.map((scenario) => `| \`${scenario.id}\` | ${scenario.width}×${scenario.height} | ${scenario.action} |`).join("\n")}

Every scenario runs under both reset modes. Computed colors normalize to 8-bit sRGB, equivalent
flex-end keywords normalize together, and layout lengths quantize to
${authority.comparison.computedLengthQuantumCssPx} CSS px to account for browser-internal
subpixel serialization. The same host must also match every styled border box, client/scroll extent,
and direct text-node rectangle captured at ${authority.comparison.layoutGeometryResolutionCssPx} CSS
px resolution with at most ${authority.comparison.maximumLayoutGeometryDeltaCssPx} CSS px delta.
Each scenario and host summary retain the maximum observed geometry delta; aggregation recomputes
that maximum and rejects missing, non-finite, negative, over-budget, or summary-drifted values.
Screenshots use pixelmatch threshold
${authority.comparison.screenshotPixelThreshold} and may differ on at most
${authority.comparison.maximumScreenshotMismatchRatio * 100}% of CSS pixels; original PNG hashes
remain retained so the tolerance cannot hide or rewrite evidence.

## Commands

\`\`\`console
pnpm check:browser-output-authority
pnpm exec playwright install chromium firefox webkit
pnpm check:browser-output -- --browser=chromium
pnpm check:browser-output -- --browser=firefox
pnpm check:browser-output -- --browser=webkit
\`\`\`

CI executes each of the nine hosts with \`--require-clean\`, uploads the JSON and PNG artifacts,
then aggregates them only when every host is unexpired, clean-tree, bound to one commit/tree, and
complete. That source identity must equal the aggregator checkout, so nine mutually consistent but
stale documents cannot pass. Local dirty-tree runs are diagnostic and cannot satisfy the hosted matrix.
`;

const mode = process.argv[2];
if (mode === "--write") {
  writeFileSync(outputPath, document);
  process.stdout.write(`wrote ${outputPath}\n`);
} else if (mode === "--check") {
  if (readFileSync(outputPath, "utf8") !== document) {
    throw new Error("browser/output certification documentation is stale; run --write");
  }
  process.stdout.write("browser/output certification documentation: current\n");
} else {
  throw new Error("usage: node scripts/render-browser-output-certification.mjs --write|--check");
}
