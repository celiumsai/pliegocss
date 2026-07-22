<!-- SPDX-License-Identifier: Apache-2.0 -->

# PliegoCSS website

The official site shell is authored as escaped PliegoRS views and published by
the PliegoRS SSG. Public documentation prose lives in `docs/site/**/*.md`;
`site/scripts/generate-docs.mjs` strictly parses those files and writes the
tracked `site/src/docs.generated.json` projection with the exact source path
and SHA-256 for every page. Rust consumes that projection and does not carry a
second hand-maintained document catalog. The release-readiness page is itself
generated from the schema-3 machine authority.

PliegoCSS `pc!` literals provide real compile-time style identities and the
same Rust source is compiled into the shipped utility bundle.
GSAP, Lenis, and Three.js run behind an explicit client lifecycle with reduced
motion, Save-Data, visibility, and WebGL fallbacks.

```console
pnpm site:build
pnpm site:preview
pnpm check:site-docs
pnpm check:site
```

Run `node site/scripts/generate-docs.mjs --write` after an intentional Markdown
change. Build and validation use `--check` and fail if the tracked projection
drifts. Each Markdown file must keep the closed `pliegocss-site` metadata
header, one H1, a one-line summary, and sections shaped as
`## Title {#stable-id}` with one body paragraph and at most one code fence.

The generated site is written to `site/target/site/` and is not committed.
The public deployment profile in `wrangler.jsonc` serves that exact directory
from Cloudflare Workers Static Assets at `pliegocss.dev`, with preview URLs and
`workers.dev` disabled. `pnpm check:site-deployment` performs a local Wrangler
dry run and does not publish.

Deployment is intentionally separate from build and validation. A production
deployment requires explicit authorization after the final source commit,
hosted CI, benchmark evidence, DNS review, and release-readiness blockers are
closed.
