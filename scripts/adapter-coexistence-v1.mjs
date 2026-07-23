import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
export const authorityPath = resolve(
  repositoryRoot,
  "benchmarks",
  "adapter-coexistence-v1",
  "authority.json",
);

export function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

export function readJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

export function loadContract() {
  const authorityBytes = readFileSync(authorityPath);
  const authority = JSON.parse(authorityBytes.toString("utf8"));
  const corpusPath = resolve(repositoryRoot, authority.corpus.path);
  const corpusBytes = readFileSync(corpusPath);
  if (sha256(corpusBytes) !== authority.corpus.sha256) {
    throw new Error("adapter coexistence corpus hash drifted");
  }
  return {
    authority,
    authorityBytes,
    authoritySha256: sha256(authorityBytes),
    corpus: JSON.parse(corpusBytes.toString("utf8")),
    corpusBytes,
    corpusPath,
    corpusSha256: sha256(corpusBytes),
  };
}

export function renderProjectDocument(project) {
  return `<!doctype html>\n<html lang="en">\n<head>\n<meta charset="utf-8">\n<meta name="viewport" content="width=device-width,initial-scale=1">\n<title>${escapeText(project.title)}</title>\n<link rel="stylesheet" href="./tailwind.css">\n<link rel="stylesheet" href="./pliego.css">\n</head>\n<body class="min-h-screen bg-canvas font-sans text-ink antialiased">${project.body}</body>\n</html>\n`;
}

function escapeText(value) {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;");
}

const classAttribute = /\bclass="([^"]+)"/gu;

export function extractStaticClassGroups(document, role = "document") {
  for (const forbidden of [
    /\bclassName\s*=/u,
    /\bclass\s*=\s*'/u,
    /(?:^|\s)(?::class|v-bind:class|\[class\]|class:list)\s*=/u,
    /\$\{|\{\{|<%|#\{/u,
    /<(?:script|style)\b/iu,
  ]) {
    if (forbidden.test(document)) {
      throw new Error(`${role} contains unsupported or dynamic class syntax ${forbidden}`);
    }
  }
  const matches = [...document.matchAll(classAttribute)];
  const rawClassMarkers = document.match(/\bclass\s*=/gu)?.length ?? 0;
  if (matches.length === 0 || matches.length !== rawClassMarkers) {
    throw new Error(`${role} must use only non-empty double-quoted class attributes`);
  }
  const groups = matches.map((match) => {
    const normalized = match[1].trim().split(/\s+/u).join(" ");
    if (normalized !== match[1]) {
      throw new Error(`${role} class groups must already use canonical single spacing`);
    }
    return normalized;
  });
  return { groups, uniqueGroups: [...new Set(groups)] };
}

export function decorateClassNodes(document, role = "document") {
  let index = 0;
  const decorated = document.replace(classAttribute, (attribute) => {
    const node = `g6-node-${String(index).padStart(3, "0")}`;
    index += 1;
    return `data-g6-node="${node}" ${attribute}`;
  });
  if (index !== extractStaticClassGroups(document, role).groups.length) {
    throw new Error(`${role} class decoration count drifted`);
  }
  return decorated;
}

export function rewriteStaticClassGroups(document, manifest, role = "document") {
  const mappings = new Map();
  for (const style of manifest.styles ?? []) {
    if (typeof style.className !== "string" || !style.className.startsWith("pc_")) {
      throw new Error(`${role} manifest contains an invalid generated class`);
    }
    for (const origin of style.origins ?? []) {
      if (typeof origin.source !== "string") continue;
      const previous = mappings.get(origin.source);
      if (previous && previous !== style.className) {
        throw new Error(`${role} maps one source class group to multiple generated classes`);
      }
      mappings.set(origin.source, style.className);
    }
  }
  let replacements = 0;
  const rewritten = document.replace(classAttribute, (_, group) => {
    const generated = mappings.get(group);
    if (!generated) {
      throw new Error(`${role} manifest has no generated class for ${JSON.stringify(group)}`);
    }
    replacements += 1;
    return `class="${generated}"`;
  });
  const expected = extractStaticClassGroups(document, role).groups.length;
  if (replacements !== expected) throw new Error(`${role} did not rewrite every class group`);
  return { document: rewritten, mappings, replacements };
}

export function exactKeys(value, expected, role) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${role} must be an object`);
  }
  const actual = Object.keys(value).sort();
  const wanted = [...expected].sort();
  if (JSON.stringify(actual) !== JSON.stringify(wanted)) {
    throw new Error(`${role} keys drifted: ${actual.join(", ")}`);
  }
}

export function profileById(authority, id) {
  const profile = authority.tailwindProfiles.find((entry) => entry.id === id);
  if (!profile) throw new Error(`unknown Tailwind profile ${id}`);
  return profile;
}

export function hostFor(authority, browser, requestedId) {
  const host = requestedId
    ? authority.hosts.find((entry) => entry.id === requestedId)
    : authority.hosts.find(
        (entry) =>
          entry.browser === browser && entry.os === process.platform && entry.arch === process.arch,
      );
  if (!host) throw new Error("current host is outside the adapter coexistence authority");
  if (host.browser !== browser || host.os !== process.platform || host.arch !== process.arch) {
    throw new Error(
      `host ${host.id} requires ${host.os}/${host.arch}/${host.browser}, found ${process.platform}/${process.arch}/${browser}`,
    );
  }
  return host;
}
