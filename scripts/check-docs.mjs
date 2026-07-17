import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, extname, relative, resolve, sep } from "node:path";

const ROOT = resolve(import.meta.dirname, "..");

function fail(message) {
  throw new Error(message);
}

function markdownFiles(directory) {
  return readdirSync(directory, { withFileTypes: true })
    .flatMap((entry) => {
      const path = resolve(directory, entry.name);
      return entry.isDirectory()
        ? markdownFiles(path)
        : entry.isFile() && extname(entry.name) === ".md"
          ? [path]
          : [];
    })
    .sort();
}

const files = [
  resolve(ROOT, "README.md"),
  resolve(ROOT, "DOCUMENTATION_PLAN.md"),
  ...markdownFiles(resolve(ROOT, "docs")),
  resolve(ROOT, "editors", "vscode", "README.md"),
  resolve(ROOT, "editors", "neovim", "README.md"),
];
const failures = [];
let localLinks = 0;
for (const file of files) {
  const source = readFileSync(file, "utf8");
  for (const match of source.matchAll(/\[[^\]]*\]\(([^)]+)\)/g)) {
    let target = match[1].trim();
    if (
      target.startsWith("#") ||
      /^(?:https?:|mailto:)/.test(target) ||
      target.includes("${")
    ) {
      continue;
    }
    if (target.startsWith("<") && target.endsWith(">")) target = target.slice(1, -1);
    target = target.split("#", 1)[0];
    try {
      target = decodeURIComponent(target);
    } catch {
      failures.push(`${relative(ROOT, file)} has invalid URL encoding in ${match[1]}`);
      continue;
    }
    const resolved = resolve(dirname(file), target);
    if (!resolved.startsWith(`${ROOT}${sep}`) && resolved !== ROOT) {
      failures.push(`${relative(ROOT, file)} links outside the repository: ${match[1]}`);
      continue;
    }
    if (!existsSync(resolved)) {
      failures.push(`${relative(ROOT, file)} links to missing ${match[1]}`);
      continue;
    }
    if (!statSync(resolved).isFile() && !statSync(resolved).isDirectory()) {
      failures.push(`${relative(ROOT, file)} links to unsupported ${match[1]}`);
      continue;
    }
    localLinks += 1;
  }
}

for (const required of [
  "docs/getting-started/editor-setup.md",
  "docs/troubleshooting/lsp.md",
  "docs/reference/lsp-diagnostic-corpus.md",
  "editors/vscode/README.md",
  "editors/neovim/README.md",
]) {
  if (!files.includes(resolve(ROOT, required))) failures.push(`missing required document ${required}`);
}

if (failures.length > 0) fail(failures.join("\n"));
process.stdout.write(
  `${JSON.stringify({ schemaVersion: 1, markdownFiles: files.length, localLinks }, null, 2)}\n`,
);
