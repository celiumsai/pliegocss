// SPDX-License-Identifier: Apache-2.0

import { createHash } from "node:crypto";
import {
  existsSync,
  readFileSync,
  readdirSync,
  statSync,
} from "node:fs";
import { dirname, extname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const brand = resolve(root, "brand");

function fail(message) {
  throw new Error(`brand contract: ${message}`);
}

function requireFile(path, minimumBytes = 1) {
  const absolute = resolve(root, path);
  if (!existsSync(absolute) || !statSync(absolute).isFile()) {
    fail(`missing ${path}`);
  }
  if (statSync(absolute).size < minimumBytes) {
    fail(`${path} is unexpectedly small`);
  }
  return absolute;
}

function text(path, minimumBytes = 1) {
  return readFileSync(requireFile(path, minimumBytes), "utf8");
}

function svg(path, viewBox) {
  const source = text(path, 120);
  if (
    !source.startsWith("<svg") ||
    !source.includes('xmlns="http://www.w3.org/2000/svg"') ||
    !source.includes(`viewBox="${viewBox}"`) ||
    /<(?:image|script|foreignObject)\b|(?:href|src)="data:/u.test(source)
  ) {
    fail(`${path} violates the standalone SVG contract`);
  }
  return source;
}

function pngDimensions(path) {
  const bytes = readFileSync(requireFile(path, 64));
  const signature = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
  if (!bytes.subarray(0, 8).equals(signature) || bytes.toString("ascii", 12, 16) !== "IHDR") {
    fail(`${path} is not a canonical PNG`);
  }
  return {
    width: bytes.readUInt32BE(16),
    height: bytes.readUInt32BE(20),
  };
}

function assertDimensions(path, expected) {
  const actual = pngDimensions(path);
  if (actual.width !== expected.width || actual.height !== expected.height) {
    fail(
      `${path} is ${actual.width}x${actual.height}; expected ${expected.width}x${expected.height}`,
    );
  }
}

const required = [
  ["brand/README.md", 800],
  ["brand/BRANDBOOK.md", 10_000],
  ["brand/brand-tokens.json", 1_000],
  ["brand/brand-tokens.css", 500],
  ["brand/pliegocss-symbol.svg", 200],
  ["brand/pliegocss-symbol-reversed.svg", 200],
  ["brand/pliegocss-symbol-monochrome.svg", 200],
  ["brand/pliegocss-lockup.svg", 300],
  ["brand/pliegocss-lockup-reversed.svg", 300],
  ["brand/pliegocss-app-icon.svg", 300],
  ["brand/pliegocss-app-icon.png", 1_000],
  ["brand/favicon.svg", 200],
  ["brand/social-card.svg", 1_000],
  ["brand/social-card.png", 10_000],
  ["brand/fonts/instrument-sans-variable.woff2", 10_000],
  ["brand/fonts/fragment-mono-regular.woff2", 10_000],
  ["brand/fonts/LICENSE-instrument-sans.txt", 1_000],
  ["brand/fonts/LICENSE-fragment-mono.txt", 1_000],
  ["brand/image-prompts/README.md", 700],
  ["brand/images/README.md", 700],
  ["brand/images/manifest.schema.json", 1_000],
];
required.forEach(([path, bytes]) => requireFile(path, bytes));

for (const path of [
  "brand/fonts/instrument-sans-variable.woff2",
  "brand/fonts/fragment-mono-regular.woff2",
]) {
  const signature = readFileSync(resolve(root, path)).toString("ascii", 0, 4);
  if (signature !== "wOF2") fail(`${path} is not WOFF2`);
}
for (const path of [
  "brand/fonts/LICENSE-instrument-sans.txt",
  "brand/fonts/LICENSE-fragment-mono.txt",
]) {
  if (!text(path).includes("SIL OPEN FONT LICENSE Version 1.1")) {
    fail(`${path} is not the expected OFL 1.1 text`);
  }
}

svg("brand/pliegocss-symbol.svg", "0 0 64 64");
svg("brand/pliegocss-symbol-reversed.svg", "0 0 64 64");
svg("brand/pliegocss-symbol-monochrome.svg", "0 0 64 64");
svg("brand/favicon.svg", "0 0 64 64");
svg("brand/pliegocss-app-icon.svg", "0 0 512 512");
svg("brand/pliegocss-lockup.svg", "0 0 392 80");
svg("brand/pliegocss-lockup-reversed.svg", "0 0 392 80");
const socialSvg = svg("brand/social-card.svg", "0 0 1200 630");
if (!socialSvg.includes("<title") || !socialSvg.includes("<desc")) {
  fail("brand/social-card.svg needs an accessible title and description");
}
assertDimensions("brand/pliegocss-app-icon.png", { width: 512, height: 512 });
assertDimensions("brand/social-card.png", { width: 1200, height: 630 });

const tokens = JSON.parse(text("brand/brand-tokens.json"));
const css = text("brand/brand-tokens.css");
const tokenProjection = new Map([
  ["--pliego-carbon-950", tokens.color?.carbon?.["950"]?.$value],
  ["--pliego-carbon-800", tokens.color?.carbon?.["800"]?.$value],
  ["--pliego-carbon-600", tokens.color?.carbon?.["600"]?.$value],
  ["--pliego-paper-50", tokens.color?.paper?.["50"]?.$value],
  ["--pliego-paper-0", tokens.color?.paper?.["0"]?.$value],
  ["--pliego-cobalt-600", tokens.color?.cobalt?.["600"]?.$value],
  ["--pliego-cobalt-400", tokens.color?.cobalt?.["400"]?.$value],
  ["--pliego-cyan-400", tokens.color?.cyan?.["400"]?.$value],
  ["--pliego-coral-500", tokens.color?.semantic?.danger?.$value],
  ["--pliego-amber-500", tokens.color?.semantic?.warning?.$value],
  [
    "--pliego-grid-unit",
    `${tokens.dimension?.grid?.$value?.value}${tokens.dimension?.grid?.$value?.unit}`,
  ],
  [
    "--pliego-radius-control",
    `${tokens.dimension?.["radius-control"]?.$value?.value}${tokens.dimension?.["radius-control"]?.$value?.unit}`,
  ],
  [
    "--pliego-radius-panel",
    `${tokens.dimension?.["radius-panel"]?.$value?.value}${tokens.dimension?.["radius-panel"]?.$value?.unit}`,
  ],
  [
    "--pliego-ease-fold",
    `cubic-bezier(${tokens.cubicBezier?.fold?.$value?.join(", ")})`,
  ],
]);
for (const [name, value] of tokenProjection) {
  if (!value || !css.includes(`${name}: ${value};`)) {
    fail(`CSS token projection drifted for ${name}`);
  }
}
if (!String(tokens.$schema).includes("design-tokens")) {
  fail("brand-tokens.json does not declare the DTCG schema");
}

const brandbook = text("brand/BRANDBOOK.md");
for (const marker of [
  "Compile confidence into CSS.",
  "## 3. Symbol",
  "## 4. Color",
  "## 5. Typography",
  "## 7. Motion",
  "## 8. Imagery and 3D",
  "## 11. Asset governance",
  "prefers-reduced-motion",
  "GPT Image 2",
]) {
  if (!brandbook.includes(marker)) fail(`brandbook is missing ${marker}`);
}

const promptPaths = readdirSync(resolve(brand, "image-prompts"))
  .filter((name) => /^\d{2}-.*\.md$/u.test(name))
  .sort();
if (promptPaths.length !== 4) {
  fail(`expected four canonical image prompts; found ${promptPaths.length}`);
}
for (const name of promptPaths) {
  const source = text(`brand/image-prompts/${name}`, 900);
  for (const marker of ["Color palette:", "Constraints:", "no logo", "no watermark", "Avoid:"]) {
    if (!source.includes(marker)) {
      fail(`brand/image-prompts/${name} is missing ${marker}`);
    }
  }
  if (!/no (?:readable )?text/u.test(source)) {
    fail(`brand/image-prompts/${name} is missing the no-text constraint`);
  }
}
const promptIndex = text("brand/image-prompts/README.md");
if (
  !promptIndex.includes("Model: GPT Image 2 through DigitalOcean Serverless Inference") ||
  !promptPaths.every((name) => promptIndex.includes(`\`${name}\``))
) {
  fail("image prompt index drifted");
}

const generatedManifestPath = resolve(brand, "images", "manifest.json");
let generatedImages = {
  status: "pending-generation",
  count: 0,
  note: "Canonical prompt specifications exist; GPT Image 2 masters are not committed.",
};
if (existsSync(generatedManifestPath)) {
  const manifest = JSON.parse(readFileSync(generatedManifestPath, "utf8"));
  const expectedRoles = new Set([
    "cascade-chamber",
    "semantic-fold",
    "evidence-archive",
    "material-study-carbon",
    "material-study-paper",
    "material-study-cobalt",
  ]);
  if (manifest.schemaVersion !== 1 || !Array.isArray(manifest.assets)) {
    fail("brand/images/manifest.json violates schema version 1");
  }
  if (manifest.assets.length !== 6) {
    fail(`expected six generated-image receipts; found ${manifest.assets.length}`);
  }
  for (const asset of manifest.assets) {
    if (
      !expectedRoles.delete(asset.role) ||
      asset.model !== "openai-gpt-image-2" ||
      asset.quality !== "high" ||
      asset.reviewedAgainstBrandbook !== true ||
      !Number.isInteger(asset.width) ||
      asset.width < 1024 ||
      !Number.isInteger(asset.height) ||
      asset.height < 1024 ||
      !/^[a-f0-9]{64}$/u.test(asset.masterSha256 ?? "") ||
      !/^[A-Za-z0-9][A-Za-z0-9._-]*\.png$/u.test(asset.master ?? "") ||
      extname(asset.master).toLowerCase() !== ".png" ||
      !/^\.\.\/image-prompts\/\d{2}-[a-z0-9-]+\.md$/u.test(
        asset.promptFile ?? "",
      ) ||
      typeof asset.finalPrompt !== "string" ||
      asset.finalPrompt.length < 500
    ) {
      fail(`invalid generated image receipt for ${asset.role ?? "unknown"}`);
    }
    const masterPath = resolve(brand, "images", asset.master);
    if (!existsSync(masterPath)) fail(`missing generated master ${asset.master}`);
    const dimensions = pngDimensions(`brand/images/${asset.master}`);
    if (
      dimensions.width !== asset.width ||
      dimensions.height !== asset.height
    ) {
      fail(
        `generated master dimensions drifted for ${asset.master}: ` +
          `${dimensions.width}x${dimensions.height} != ${asset.width}x${asset.height}`,
      );
    }
    const promptPath = resolve(brand, "images", asset.promptFile);
    if (
      !promptPaths.some(
        (name) => resolve(brand, "image-prompts", name) === promptPath,
      )
    ) {
      fail(`generated image receipt references an unknown prompt ${asset.promptFile}`);
    }
    const digest = createHash("sha256")
      .update(readFileSync(masterPath))
      .digest("hex");
    if (digest !== asset.masterSha256) {
      fail(`generated master hash drifted for ${asset.master}`);
    }
  }
  if (expectedRoles.size !== 0) {
    fail(`generated image roles are incomplete: ${[...expectedRoles].join(", ")}`);
  }
  generatedImages = {
    status: "passed",
    count: manifest.assets.length,
    model: "openai-gpt-image-2",
  };
}

process.stdout.write(
  `${JSON.stringify(
    {
      schemaVersion: 1,
      result: "passed",
      canonicalFiles: required.length + promptPaths.length,
      symbolVariants: 7,
      rasterExports: 2,
      tokenProjection: tokenProjection.size,
      generatedImages,
    },
    null,
    2,
  )}\n`,
);
