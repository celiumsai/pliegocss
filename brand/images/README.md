<!-- SPDX-License-Identifier: Apache-2.0 -->

# Generated image masters

This directory is reserved for the canonical GPT Image 2 masters and their
hash-bound generation receipt. No placeholder or substitute image is a
canonical PliegoCSS asset.

The required roles are:

1. `cascade-chamber` — 16:9 homepage editorial field.
2. `semantic-fold` — 4:3 compiler narrative.
3. `evidence-archive` — 4:3 evidence narrative.
4. `material-study-carbon` — square material crop.
5. `material-study-paper` — square material crop.
6. `material-study-cobalt` — square material crop.

When generation is available:

- use the exact specifications in [`../image-prompts/`](../image-prompts/);
- keep the lossless PNG master in this directory;
- derive AVIF and WebP renditions without overwriting the master;
- review every result against [`../BRANDBOOK.md`](../BRANDBOOK.md);
- write `manifest.json` conforming to
  [`manifest.schema.json`](./manifest.schema.json);
- record the final prompt, model name, quality, dimensions, seed or generation
  identifier when supplied, and SHA-256 of each master.

The canonical model receipt is the exact DigitalOcean Serverless Inference
catalog identifier `openai-gpt-image-2`; “GPT Image 2” is the human-facing model
name used in the art direction.

The absence of `manifest.json` means **generation is pending**, not that the
asset set passed visual review.
