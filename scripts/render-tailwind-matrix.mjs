import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { loadAuthority, repositoryRoot } from "./benchmark-authority-v2.mjs";

const matrixPath = resolve(repositoryRoot, "docs/benchmarks/tailwind-v4-competitive-matrix.json");
const documentPath = resolve(repositoryRoot, "docs/benchmarks/tailwind-v4-competitive-matrix.md");

function cell(value) {
  return `${value}`.replaceAll("|", "\\|").replaceAll(/\s+/gu, " ").trim();
}

export function renderTailwindMatrix() {
  const matrix = JSON.parse(readFileSync(matrixPath, "utf8"));
  const authority = loadAuthority();
  const lanes = authority.lanes
    .map(
      (lane) =>
        `| \`${lane.id}\` | ${cell(lane.role)} | \`${lane.distTag ?? "pinned"}\` | \`${lane.version}\` |`,
    )
    .join("\n");
  const dimensions = matrix.dimensions
    .map(
      (dimension) =>
        `| \`${dimension.id}\` | ${cell(dimension.area)} | **${cell(dimension.status)}** | ${cell(dimension.tailwind)} | ${cell(dimension.pliego)} | ${cell(dimension.releaseImpact)} |`,
    )
    .join("\n");
  const priorities = (impact) =>
    matrix.dimensions
      .filter((dimension) => dimension.releaseImpact === impact)
      .map((dimension) => `- \`${dimension.id}\`: ${dimension.nextAction}`)
      .join("\n") || "- None.";
  const counts = Object.fromEntries(Object.keys(matrix.statuses).map((status) => [status, 0]));
  for (const dimension of matrix.dimensions) counts[dimension.status] += 1;
  const countText = Object.entries(counts)
    .map(([status, count]) => `\`${status}\` ${count}`)
    .join(", ");

  return `# PliegoCSS and Tailwind CSS ${matrix.baseline.version}

Status: **generated competitive matrix for release planning; not a universal performance or parity claim**

This document is generated from [\`tailwind-v4-competitive-matrix.json\`](./tailwind-v4-competitive-matrix.json).
The competitor and corpus authority is [Benchmark Authority v2](./tailwind-benchmark-authority-v2.md).
Edit those JSON contracts and regenerate; do not update this table by hand.

PliegoCSS is not positioned as “Tailwind in Rust.” Tailwind is the production utility-framework
baseline; PliegoCSS competes through typed semantics, policy, provenance, ownership, diagnostics and
receipts. Catalog breadth and a historical single-fixture timing are not a complete score.

## Active oracle

Primary lane: \`${authority.primaryLane}\`. Oracle observation: \`${authority.observedAtUtc}\`.
Expiration: \`${authority.expiresAtUtc}\`. Reset contract: **${authority.resetContract}**.

| Lane | Role | Registry selector | Version |
|---|---|---|---:|
${lanes}

The \`4.3.2\` lane is historical regression control only. Current performance and payload claims
remain open until immutable schema-2 evidence is recorded from a clean tree with complete paired
samples, peak memory, gzip, Brotli and all three corpus sizes.

## Executive matrix

| Dimension | Area | Assessment | Tailwind | PliegoCSS | Release impact |
|---|---|---|---|---|---|
${dimensions}

Status totals: ${countText}.

## Blocking

${priorities("blocking")}

## Should

${priorities("should")}

## Explicitly post-0.1

${priorities("post-0.1")}

## Historical evidence boundary

[Gate B](./pliego-gate-b.md) remains immutable historical evidence for Tailwind \`4.3.2\` on its
recorded host. It cannot represent \`tailwind-latest\`, v3-LTS, current paired latency, memory,
Brotli, or micro/medium/large behavior. No current competitive score is emitted from that snapshot.

## Verification

\`\`\`console
pnpm check:benchmark-authority
pnpm check:benchmark-oracle
pnpm check:tailwind-matrix
node scripts/measure-benchmark-authority-v2.mjs --smoke
\`\`\`
`;
}

const arguments_ = process.argv.slice(2);
if (arguments_.length !== 1 || !new Set(["--check", "--write"]).has(arguments_[0])) {
  throw new Error("usage: node scripts/render-tailwind-matrix.mjs --check|--write");
}
const rendered = renderTailwindMatrix();
if (arguments_[0] === "--write") {
  writeFileSync(documentPath, rendered);
  process.stdout.write(`${documentPath}\n`);
} else {
  if (readFileSync(documentPath, "utf8") !== rendered) {
    throw new Error("Tailwind competitive matrix Markdown is stale; run the renderer with --write");
  }
  process.stdout.write(`${JSON.stringify({ schemaVersion: 2, status: "passed", document: "docs/benchmarks/tailwind-v4-competitive-matrix.md" }, null, 2)}\n`);
}
