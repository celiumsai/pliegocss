// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import { generateManifest, parseDocument } from "./generate-docs.mjs";

const manifest = generateManifest();
assert.equal(manifest.schemaVersion, 1);
assert.equal(manifest.kind, "pliegocss-site-markdown");
assert.equal(manifest.documents.length, 31);
assert.equal(new Set(manifest.documents.map((document) => document.route)).size, 31);
assert.equal(manifest.documents.at(-1).route, "/docs/external-adoption/");
assert.match(manifest.documents.at(-1).sourceSha256, /^sha256:[a-f0-9]{64}$/u);

const sample = `<!-- pliegocss-site
{"schemaVersion":1,"route":"/docs/sample/","category":"Core","eyebrow":"Sample","order":1}
-->

# Sample title

Sample summary.

## First section {#first}

Sample body.
`;
assert.equal(parseDocument(sample).sections[0].body, "Sample body.");
assert.throws(
  () => parseDocument(sample.replace("## First section", "### First section")),
  /unsupported structure/u,
);
assert.throws(
  () => parseDocument(sample.replace('"order":1', '"order":1,"extra":true')),
  /metadata fields drifted/u,
);
process.stdout.write("site Markdown source contracts: pass\n");
