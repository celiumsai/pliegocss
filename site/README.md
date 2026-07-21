<!-- SPDX-License-Identifier: Apache-2.0 -->

# PliegoCSS website

The official site is authored as escaped PliegoRS views and published by the
PliegoRS SSG. PliegoCSS `pc!` literals provide real compile-time style
identities and the same source is compiled into the shipped utility bundle.
GSAP, Lenis, and Three.js run behind an explicit client lifecycle with reduced
motion, Save-Data, visibility, and WebGL fallbacks.

```console
pnpm site:build
pnpm site:preview
pnpm check:site
```

The generated site is written to `site/target/site/` and is not committed.
The public deployment profile in `wrangler.jsonc` serves that exact directory
from Cloudflare Workers Static Assets at `pliegocss.dev`, with preview URLs and
`workers.dev` disabled. `pnpm check:site-deployment` performs a local Wrangler
dry run and does not publish.

Deployment is intentionally separate from build and validation. A production
deployment requires explicit authorization after the final source commit,
hosted CI, benchmark evidence, DNS review, and release-readiness blockers are
closed.
