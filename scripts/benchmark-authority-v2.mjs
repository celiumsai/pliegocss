import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
export const authorityPath = join(
  repositoryRoot,
  "benchmarks",
  "benchmark-authority-v2",
  "oracle.json",
);

export function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

export function loadAuthority() {
  return JSON.parse(readFileSync(authorityPath, "utf8"));
}

export function extractClassValues(html) {
  return [...html.matchAll(/class="([^"]*)"/gu)].map((match) => match[1]);
}

export function materializeCorpus(corpus) {
  const source = readFileSync(resolve(repositoryRoot, corpus.source), "utf8").replaceAll(
    "\r\n",
    "\n",
  );
  if (corpus.materialization === "tracked") {
    return source;
  }
  if (corpus.materialization !== "repeat-body") {
    throw new Error(`unknown corpus materialization ${JSON.stringify(corpus.materialization)}`);
  }
  const body = /<body(?:\s+class="([^"]*)")?[^>]*>([\s\S]*?)<\/body>/u.exec(source);
  if (!body) {
    throw new Error(`${corpus.source} has no materializable body`);
  }
  const bodyClass = body[1]?.trim() ?? "";
  const contents = body[2].trim();
  const copies = Array.from({ length: corpus.repeat }, (_, index) => {
    const classAttribute = bodyClass ? ` class="${bodyClass}"` : "";
    return `    <section data-benchmark-copy="${index}"${classAttribute}>\n${contents}\n    </section>`;
  }).join("\n");
  return [
    "<!doctype html>",
    '<html lang="en">',
    "  <head>",
    '    <meta charset="UTF-8" />',
    '    <meta name="viewport" content="width=device-width, initial-scale=1.0" />',
    `    <title>PliegoCSS benchmark authority ${corpus.id} corpus</title>`,
    "  </head>",
    "  <body>",
    copies,
    "  </body>",
    "</html>",
    "",
  ].join("\n");
}

export function corpusMetrics(html) {
  const classValues = extractClassValues(html);
  const utilityTokens = classValues.flatMap((value) => value.trim().split(/\s+/u).filter(Boolean));
  return {
    sha256: sha256(Buffer.from(html, "utf8")),
    htmlBytes: Buffer.byteLength(html),
    classAttributeCount: classValues.length,
    utilityOccurrences: utilityTokens.length,
    uniqueUtilityTokens: new Set(utilityTokens).size,
    uniqueStyleInputs: new Set(classValues).size,
  };
}

export function round(value) {
  return Number(value.toFixed(3));
}

function median(sorted) {
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 === 0
    ? (sorted[middle - 1] + sorted[middle]) / 2
    : sorted[middle];
}

export function summarize(values, unit) {
  if (!Array.isArray(values) || values.length === 0 || values.some((value) => !Number.isFinite(value))) {
    throw new Error("cannot summarize an empty or non-finite sample series");
  }
  const sorted = [...values].sort((left, right) => left - right);
  const center = median(sorted);
  const deviations = sorted.map((value) => Math.abs(value - center)).sort((left, right) => left - right);
  return {
    unit,
    samples: values.map(round),
    count: values.length,
    min: round(sorted[0]),
    median: round(center),
    p95: round(sorted[Math.ceil(sorted.length * 0.95) - 1]),
    max: round(sorted.at(-1)),
    medianAbsoluteDeviation: round(median(deviations)),
  };
}

export function packageJsonPath(alias) {
  return join(repositoryRoot, "node_modules", ...alias.split("/"), "package.json");
}

export function readInstalledPackage(alias) {
  const path = packageJsonPath(alias);
  return { path, value: JSON.parse(readFileSync(path, "utf8")) };
}

export function resolveLaneCli(lane) {
  const installed = readInstalledPackage(lane.cliPackageAlias);
  const bin = installed.value.bin;
  const relative = typeof bin === "string" ? bin : (bin?.tailwindcss ?? bin?.tailwind);
  if (!relative) {
    throw new Error(`${lane.cliPackageAlias} exposes no Tailwind CLI binary`);
  }
  return resolve(dirname(installed.path), relative);
}

export function canonicalLaneMap(authority) {
  return new Map(authority.lanes.map((lane) => [lane.id, lane]));
}
