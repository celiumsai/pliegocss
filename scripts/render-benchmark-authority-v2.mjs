import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import {
  corpusMetrics,
  loadAuthority,
  materializeCorpus,
  repositoryRoot,
} from "./benchmark-authority-v2.mjs";

const arguments_ = process.argv.slice(2);
if (arguments_.length !== 1 || !new Set(["--check", "--write"]).has(arguments_[0])) {
  throw new Error("usage: node scripts/render-benchmark-authority-v2.mjs --check|--write");
}

const authority = loadAuthority();
const destination = resolve(repositoryRoot, authority.artifacts.generatedDocumentation);
const tag = (lane) => lane.distTag ?? "pinned version";
const laneRows = authority.lanes
  .map(
    (lane) =>
      `| \`${lane.id}\` | ${lane.role} | \`${tag(lane)}\` | \`${lane.version}\` | \`${lane.cliVersion}\` |`,
  )
  .join("\n");
const corpusRows = authority.corpora
  .map((corpus) => {
    const metrics = corpusMetrics(materializeCorpus(corpus));
    return `| \`${corpus.id}\` | ${corpus.scope} | ${metrics.htmlBytes.toLocaleString("en-US")} | ${metrics.classAttributeCount.toLocaleString("en-US")} | ${metrics.utilityOccurrences.toLocaleString("en-US")} | ${metrics.uniqueUtilityTokens.toLocaleString("en-US")} |`;
  })
  .join("\n");

const markdown = `# Tailwind Benchmark Authority v2

Status: **active machine authority; generated from [\`oracle.json\`](../../benchmarks/benchmark-authority-v2/oracle.json)**

This document is generated. Edit the schema-2 JSON authority and run
\`node scripts/render-benchmark-authority-v2.mjs --write\`; do not maintain the tables by hand.

The primary competitor is \`${authority.primaryLane}\`. The oracle was observed at
\`${authority.observedAtUtc}\`, expires at \`${authority.expiresAtUtc}\`, and has a maximum age of
${authority.maximumAgeHours} hours. An expired or live-registry-mismatched oracle blocks a new
competitive result. The reset contract is explicitly **${authority.resetContract}** because
PliegoCSS does not emit Preflight.

## Competitor lanes

| Lane | Role | Registry selector | Tailwind | CLI |
|---|---|---|---:|---:|
${laneRows}

\`tailwind-frozen-release\` is a regression control. It is not allowed to stand in for current
Tailwind. The v3 lane follows the upstream npm \`v3-lts\` dist-tag; “LTS” here names that registry
lane and does not expand Tailwind's support promises.

## Corpora

| Corpus | Scope | HTML bytes | Class attributes | Utility occurrences | Unique utilities |
|---|---|---:|---:|---:|---:|
${corpusRows}

The large corpus repeats the medium body ${authority.corpora.find((corpus) => corpus.id === "large").repeat}
times. It measures source-volume scaling while deliberately keeping the same utility vocabulary;
it is not presented as broader catalog coverage.

## Measurement contract

- ${authority.methodology.warmupPairs} warmup pairs and ${authority.methodology.measuredPairs} measured pairs per lane/corpus.
- Pair order alternates: ${authority.methodology.pairOrder}.
- Every observation is a fresh process. Complete per-pair latency and peak-working-set samples are retained.
- A separate fresh invocation must reproduce each measured output hash.
- CSS and rewritten/source HTML report raw, gzip level ${authority.methodology.compression.gzipLevel}, and Brotli quality ${authority.methodology.compression.brotliQuality} bytes.
- Transfer compares separate HTML and CSS streams under both compression formats.
- Candidate coverage must be 100% for the shared corpus before a pair is comparable.
- PliegoCSS and Tailwind use the same no-Preflight utility intent; browser computed-style equivalence belongs to G4.

Peak working set is measured for the direct child process by
[\`benchmark-process-metrics.rs\`](../../scripts/benchmark-process-metrics.rs), not inferred from
host free-memory deltas. Process startup, security tooling and normal OS caches remain part of the
fresh-process observation; the release build happens outside the timed region.

## Commands

\`\`\`console
pnpm check:benchmark-authority
pnpm check:benchmark-oracle
node scripts/measure-benchmark-authority-v2.mjs --smoke
node scripts/measure-benchmark-authority-v2.mjs
\`\`\`

Immutable evidence additionally uses \`--evidence=benchmarks/evidence/v2/<snapshot>.json\`, the
canonical 5/30 sample counts, a clean source tree, a new destination, and an explicit security-tooling
declaration. Machine-local results remain ignored at
\`${authority.artifacts.localResult}\`.

## Claim boundary

${authority.methodology.scorePolicy}

Legacy Gate A/B snapshots remain historical integrity evidence. They cannot supply the current
competitor lane, memory, Brotli, or micro/medium/large claims and therefore cannot produce the v2
competitive score.
`;

if (arguments_[0] === "--write") {
  writeFileSync(destination, markdown);
  process.stdout.write(`${destination}\n`);
} else {
  const current = readFileSync(destination, "utf8");
  if (current !== markdown) {
    throw new Error(`${authority.artifacts.generatedDocumentation} is stale; run the renderer with --write`);
  }
  process.stdout.write(
    `${JSON.stringify({ schemaVersion: 2, status: "passed", document: authority.artifacts.generatedDocumentation, lanes: authority.lanes.length, corpora: authority.corpora.length }, null, 2)}\n`,
  );
}
