import { existsSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const guidePath = resolve(root, "docs/getting-started/pliegors.md");
const guide = readFileSync(guidePath, "utf8");
const normalizedGuide = guide.replace(/\s+/g, " ");

const requiredSections = [
  "## 1. Scaffold the PliegoRS application",
  "## 3. Author a typed utility and bind it to a component",
  "## 4. Register routes, components, and islands once",
  "## 5. Derive reachability and CSS partitions",
  "## 6. Publish route and island CSS through normal assets",
  "## 7. Add a resumable island",
  "## 8. Keep external CSS when it is the right tool",
  "## 9. Run the development loop",
  "## Current boundary",
];
for (const section of requiredSections) {
  if (!guide.includes(section)) {
    throw new Error(`PliegoRS guide is missing required section: ${section}`);
  }
}

const requiredClaims = [
  "product_component!",
  "ProductRegistry",
  "ApplicationTopology",
  "pliego build",
  "pliego.build.json",
  "preload_stylesheet",
  "pliego_resume",
  "external stylesheets",
  "full-page reload",
  "not yet a one-command PliegoRS starter feature",
];
for (const claim of requiredClaims) {
  if (!normalizedGuide.includes(claim)) {
    throw new Error(`PliegoRS guide lost required contract text: ${claim}`);
  }
}

const fixtureContracts = new Map([
  ["integration-tests/pliegors-smoke/src/product.rs", ["ProductRegistry", "application_registry"]],
  ["integration-tests/pliegors-smoke/src/styles/home.rs", ["product_component!", "pc!"]],
  ["integration-tests/pliegors-smoke/src/css_adapter.rs", ["ApplicationTopology", "generate_css_inputs"]],
  ["integration-tests/pliegors-smoke/src/bin/collector.rs", ["application_registry"]],
  ["integration-tests/pliegors-smoke/src/bin/ssg.rs", ["preload_stylesheet", "Site::new"]],
]);
for (const [relativePath, symbols] of fixtureContracts) {
  const source = readFileSync(resolve(root, relativePath), "utf8");
  for (const symbol of symbols) {
    if (!source.includes(symbol)) {
      throw new Error(`${relativePath} no longer carries documented symbol ${symbol}`);
    }
  }
}

const localLinks = [...guide.matchAll(/\[[^\]]+\]\(([^)]+)\)/g)]
  .map((match) => match[1])
  .filter((target) => !target.startsWith("http") && !target.startsWith("#"))
  .map((target) => target.split("#", 1)[0]);
for (const target of localLinks) {
  if (!existsSync(resolve(dirname(guidePath), target))) {
    throw new Error(`PliegoRS guide has a broken local link: ${target}`);
  }
}

console.log(JSON.stringify({
  status: "ok",
  guide: "docs/getting-started/pliegors.md",
  sections: requiredSections.length,
  contractClaims: requiredClaims.length,
  fixtureFiles: fixtureContracts.size,
  localLinks: localLinks.length,
}, null, 2));
