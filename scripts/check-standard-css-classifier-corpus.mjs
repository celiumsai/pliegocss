import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const corpus = JSON.parse(readFileSync(resolve(root, "docs/reference/standard-css-classifier-corpus.json"), "utf8"));
function fail(message) { throw new Error(`standard CSS classifier corpus: ${message}`); }
if (corpus.schemaVersion !== 1 || corpus.classifierVersion !== "css-rule-features-1" || corpus.coverage !== "rule-level-bounded") fail("header drifted");
if (corpus.declarationInventory?.version !== "declaration-shapes-1" || JSON.stringify(corpus.declarationInventory.classes) !== JSON.stringify(["typed", "unparsed", "custom"])) fail("declaration inventory drifted");
if (corpus.declarationFeatures?.length !== 5 || corpus.declarationFeatures[0].id !== "user-select" || corpus.declarationFeatures[0].modernDecision !== "rejected" || corpus.declarationFeatures[1].id !== "aspect-ratio" || corpus.declarationFeatures[1].modernDecision !== "preserved" || corpus.declarationFeatures[2].id !== "color-function" || corpus.declarationFeatures[3].id !== "oklab" || corpus.declarationFeatures[4].id !== "color-mix") fail("declaration feature corpus drifted");
if (corpus.selectorInventory?.version !== "selector-shapes-1" || corpus.selectorInventory.metrics?.length !== 5) fail("selector inventory drifted");
if (corpus.selectorFeatures?.length !== 5 || corpus.selectorFeatures.some((feature) => feature.baseline !== "widely") || JSON.stringify(corpus.selectorFeatures.map((feature) => feature.id)) !== JSON.stringify(["focus-visible","has","is","where","not"])) fail("selector feature corpus drifted");
const expected = ["cascade-layers","container-queries","container-scroll-state-queries","container-style-queries","nesting","registered-custom-properties","scope","starting-style"];
const ids = corpus.features.map((feature) => feature.id).sort();
if (JSON.stringify(ids) !== JSON.stringify(expected)) fail(`feature inventory drifted: ${JSON.stringify(ids)}`);
if (new Set(ids).size !== ids.length || corpus.features.some((feature) => !feature.css)) fail("feature entries are invalid");
if (corpus.knownUnclassified?.length !== 2 || corpus.limitations?.length !== 3) fail("claim boundary drifted");
process.stdout.write(`${JSON.stringify({ schemaVersion: 1, classifierVersion: corpus.classifierVersion, declarationInventoryVersion: corpus.declarationInventory.version, selectorInventoryVersion: corpus.selectorInventory.version, classifiedFamilies: ids.length, knownUnclassified: corpus.knownUnclassified.length, coverage: corpus.coverage }, null, 2)}\n`);
