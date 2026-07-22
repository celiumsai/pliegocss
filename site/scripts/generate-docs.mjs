// SPDX-License-Identifier: Apache-2.0

import { createHash } from "node:crypto";
import {
  existsSync,
  readFileSync,
  readdirSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const scriptPath = fileURLToPath(import.meta.url);
const siteRoot = resolve(dirname(scriptPath), "..");
const root = resolve(siteRoot, "..");
const docsRoot = resolve(root, "docs/site");
const outputPath = resolve(siteRoot, "src/docs.generated.json");
const metadataFields = [
  "schemaVersion",
  "route",
  "category",
  "eyebrow",
  "order",
];

function fail(sourcePath, message) {
  throw new Error(`site docs: ${sourcePath}: ${message}`);
}

function exactFields(value, expected, sourcePath, role) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    fail(sourcePath, `${role} must be an object`);
  }
  const actual = Object.keys(value).sort();
  if (JSON.stringify(actual) !== JSON.stringify([...expected].sort())) {
    fail(sourcePath, `${role} fields drifted: ${actual.join(", ")}`);
  }
}

function portablePath(path) {
  return path.split(sep).join("/");
}

function markdownFiles(directory) {
  const files = [];
  for (const entry of readdirSync(directory).sort()) {
    const path = resolve(directory, entry);
    if (statSync(path).isDirectory()) files.push(...markdownFiles(path));
    else if (entry.endsWith(".md")) files.push(path);
  }
  return files;
}

function oneLine(lines, sourcePath, role) {
  if (lines.length !== 1 || lines[0].trim() !== lines[0] || !lines[0]) {
    fail(sourcePath, `${role} must be one non-empty paragraph line`);
  }
  return lines[0];
}

export function parseDocument(bytes, sourcePath = "document.md") {
  const source = Buffer.isBuffer(bytes) ? bytes : Buffer.from(bytes, "utf8");
  const text = source.toString("utf8").replaceAll("\r\n", "\n");
  const lines = text.endsWith("\n") ? text.slice(0, -1).split("\n") : text.split("\n");
  if (
    lines[0] !== "<!-- pliegocss-site" ||
    lines[2] !== "-->" ||
    lines[3] !== ""
  ) {
    fail(sourcePath, "expected the strict three-line pliegocss-site header");
  }
  let metadata;
  try {
    metadata = JSON.parse(lines[1]);
  } catch (error) {
    fail(sourcePath, `invalid metadata JSON: ${error.message}`);
  }
  exactFields(metadata, metadataFields, sourcePath, "metadata");
  if (metadata.schemaVersion !== 1) fail(sourcePath, "unsupported metadata schema");
  if (!/^\/docs\/[a-z0-9][a-z0-9/-]*\/$/u.test(metadata.route)) {
    fail(sourcePath, "route must be a canonical /docs/.../ path");
  }
  if (
    typeof metadata.category !== "string" ||
    !metadata.category ||
    typeof metadata.eyebrow !== "string" ||
    !metadata.eyebrow ||
    !Number.isSafeInteger(metadata.order) ||
    metadata.order < 1
  ) {
    fail(sourcePath, "category, eyebrow, or order is invalid");
  }
  const titleMatch = /^# (.+)$/u.exec(lines[4] ?? "");
  if (!titleMatch || lines[5] !== "") fail(sourcePath, "expected one H1 followed by a blank line");
  const summary = oneLine([lines[6] ?? ""], sourcePath, "summary");
  if (lines[7] !== "") fail(sourcePath, "summary must be followed by a blank line");

  const sections = [];
  const sectionIds = new Set();
  let index = 8;
  while (index < lines.length) {
    const heading = /^## (.+) \{#([a-z0-9][a-z0-9-]*)\}$/u.exec(lines[index] ?? "");
    if (!heading) fail(sourcePath, `unsupported structure at line ${index + 1}`);
    if (sectionIds.has(heading[2])) fail(sourcePath, `duplicate section id ${heading[2]}`);
    sectionIds.add(heading[2]);
    if (lines[index + 1] !== "") fail(sourcePath, `section ${heading[2]} needs a blank line`);
    const body = oneLine([lines[index + 2] ?? ""], sourcePath, `section ${heading[2]} body`);
    index += 3;
    let code = null;
    if (index < lines.length) {
      if (lines[index] !== "") fail(sourcePath, `section ${heading[2]} body must end with a blank line`);
      index += 1;
      if ((lines[index] ?? "").startsWith("```")) {
        if (!/^```[a-z0-9-]*$/u.test(lines[index])) {
          fail(sourcePath, `section ${heading[2]} has an invalid code fence`);
        }
        index += 1;
        const codeLines = [];
        while (index < lines.length && lines[index] !== "```") {
          codeLines.push(lines[index]);
          index += 1;
        }
        if (lines[index] !== "```") fail(sourcePath, `section ${heading[2]} code fence is open`);
        if (codeLines.length === 0) fail(sourcePath, `section ${heading[2]} code is empty`);
        code = codeLines.join("\n");
        index += 1;
        if (index < lines.length) {
          if (lines[index] !== "") fail(sourcePath, `section ${heading[2]} code must end with a blank line`);
          index += 1;
        }
      }
    }
    sections.push({ id: heading[2], title: heading[1], body, code });
  }
  if (sections.length === 0) fail(sourcePath, "at least one section is required");
  return {
    sourcePath,
    sourceSha256: `sha256:${createHash("sha256").update(source).digest("hex")}`,
    route: metadata.route,
    title: titleMatch[1],
    category: metadata.category,
    eyebrow: metadata.eyebrow,
    summary,
    order: metadata.order,
    sections,
  };
}

export function generateManifest() {
  if (!existsSync(docsRoot)) throw new Error("site docs: docs/site does not exist");
  const documents = markdownFiles(docsRoot).map((path) => {
    const sourcePath = portablePath(relative(root, path));
    if (!sourcePath.startsWith("docs/site/") || sourcePath.includes("../")) {
      fail(sourcePath, "source escaped docs/site");
    }
    return parseDocument(readFileSync(path), sourcePath);
  });
  documents.sort((left, right) => left.order - right.order || left.route.localeCompare(right.route));
  const routes = new Set();
  for (const [index, document] of documents.entries()) {
    if (document.order !== index + 1) {
      fail(document.sourcePath, `order must be contiguous; expected ${index + 1}`);
    }
    if (routes.has(document.route)) fail(document.sourcePath, `duplicate route ${document.route}`);
    routes.add(document.route);
  }
  return { schemaVersion: 1, kind: "pliegocss-site-markdown", documents };
}

export function renderManifest() {
  return `${JSON.stringify(generateManifest(), null, 2)}\n`;
}

function main() {
  const args = process.argv.slice(2);
  if (args.length > 1 || (args[0] && !["--check", "--write"].includes(args[0]))) {
    throw new Error("usage: node site/scripts/generate-docs.mjs [--check|--write]");
  }
  const mode = args[0] ?? "--check";
  const rendered = renderManifest();
  if (mode === "--write") {
    writeFileSync(outputPath, rendered, "utf8");
    process.stdout.write(`wrote ${outputPath}\n`);
    return;
  }
  if (!existsSync(outputPath) || readFileSync(outputPath, "utf8").replaceAll("\r\n", "\n") !== rendered) {
    throw new Error("site docs: generated manifest drifted; run with --write");
  }
  const manifest = JSON.parse(rendered);
  process.stdout.write(`site Markdown manifest: current (${manifest.documents.length} documents)\n`);
}

if (process.argv[1] && resolve(process.argv[1]) === scriptPath) main();
