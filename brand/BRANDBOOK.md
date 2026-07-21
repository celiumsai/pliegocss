<!-- SPDX-License-Identifier: Apache-2.0 -->

# PliegoCSS brandbook

**Version:** 1.0  
**Identity status:** canonical for the PliegoCSS public preview  
**Product status:** public preview; this identity does not imply that crates,
binaries, or the final `0.1.0` release have been publicly released

## 1. Brand foundation

### Position

PliegoCSS is the deterministic CSS toolchain for teams that want ordinary CSS,
typed Rust authoring, static analysis, reversible adoption, and reproducible
build evidence without a browser styling runtime.

### Brand promise

> Compile confidence into CSS.

This line is the primary external expression. It connects the product's build
discipline with its standards-compliant output without claiming that CSS itself
becomes infallible.

### Narrative

Most CSS tooling optimizes one moment: writing classes, minifying files, or
finding a warning. PliegoCSS connects the whole path. Authored input is lowered
to a semantic model, conflicts are made explicit, output stays ordinary CSS,
and every material artifact can be traced and replayed. The brand therefore
looks like a fold in progress: one rail enters, one rail leaves, and the join is
visible rather than hidden.

### Personality

| Is | Is not |
| --- | --- |
| Exact | Pedantic for its own sake |
| Composed | Sterile |
| Direct | Abrasive |
| Technical | Jargon-heavy |
| Ambitious | Hyperbolic |
| Evidence-led | Defensive |

## 2. Messaging system

### Primary message

**Compile confidence into CSS.**

### Supporting messages

1. **Standards first.** The browser receives static CSS, not a styling runtime.
2. **Rust native.** Typed authoring and tooling share one deterministic core.
3. **Evidence built in.** Manifests, findings, traces, maps, and receipts stay
   bound to the output they describe.
4. **Adopt incrementally.** Audit ordinary CSS first; migrate only where the
   evidence justifies it.

### Short descriptors

- **12 words:** Deterministic CSS tooling for standards-first teams, Rust
  applications, and automation.
- **25 words:** Audit ordinary CSS, compile typed Rust styles, and publish
  reproducible artifacts with compatibility, accessibility, provenance, and
  rollback evidence.
- **Repository subtitle:** Standards-first CSS tooling with a deterministic
  Rust core.

### Voice

- Start with the concrete behavior, then state why it matters.
- Prefer short declarative sentences.
- Name limitations beside claims.
- Use “deterministic” only for surfaces covered by exact-output or replay
  evidence.
- Use “standards-compliant CSS output,” not “a new CSS runtime.”
- Never claim Tailwind compatibility as equivalence; link to the measured
  competitive matrix.
- Never call static accessibility findings WCAG certification.

### Approved calls to action

- Start with an audit
- Compile a typed style
- Read the evidence
- Explore the documentation
- Inspect the benchmark

Avoid generic sales language such as “supercharge,” “10×,” “magic,”
“effortless,” or “the future of CSS.”

## 3. Symbol

The symbol is constructed on a 64 × 64 grid from two open square paths with an
8-unit stroke:

- **Carbon rail:** authored source, standards, and explicit input.
- **Cobalt rail:** compiled projection, evidence, and controlled output.
- **Shared fold:** the semantic boundary where PliegoCSS resolves intent.

The mark deliberately avoids a closed container. Ordinary CSS remains usable
outside PliegoCSS, and the system is designed for incremental adoption.

### Minimum size

- Standalone symbol: 24 CSS pixels.
- App icon: 32 CSS pixels.
- Horizontal lockup: 120 CSS pixels wide.
- Print: 8 mm for the symbol.

### Clear space

Reserve at least one stroke width around every side. In the master grid, the
clear-space unit is 8.

### Background selection

- On `Paper` or white, use `pliegocss-symbol.svg`.
- On `Carbon`, use `pliegocss-symbol-reversed.svg`.
- On photography, gradients, or unknown partner surfaces, place the app icon on
  an opaque square rather than floating the two-color symbol.
- In one-color production, use the monochrome asset without rebuilding it.

### Prohibited treatments

Do not:

- alter the rail geometry or stroke proportion;
- convert line joins to rounded corners;
- apply gradients, glow, bevel, texture, or drop shadow to the mark;
- place copy inside the symbol;
- animate the symbol continuously;
- use the cobalt rail without the carbon/paper rail;
- recolor the mark to match third-party brands.

For motion, the two rails may draw once in reading order and settle within
600 ms. Reduced-motion experiences show the completed mark immediately.

## 4. Color

### Core palette

| Token | Value | Role |
| --- | --- | --- |
| Carbon 950 | `#11151a` | Primary ink, dark surfaces |
| Carbon 800 | `#283039` | Secondary dark surface, dividers |
| Carbon 600 | `#59636d` | Muted copy on light surfaces |
| Paper 50 | `#f4f7f8` | Primary light field |
| Paper 0 | `#ffffff` | Raised light surfaces |
| Cobalt 600 | `#2855ff` | Primary action and compiled rail |
| Cobalt 400 | `#6f8cff` | Dark-surface rail and supporting accent |
| Cyan 400 | `#65d9ff` | Trace, focus, and data accent |
| Coral 500 | `#ff5a47` | Errors and destructive state only |
| Amber 500 | `#d99a20` | Warnings and pending state only |

### Color behavior

- Cobalt is a structural signal, not decoration. Use it for selected states,
  links, generated output, and the compiled rail.
- Cyan is for trace information, focus, and data visualization.
- Coral and amber always retain semantic meaning.
- Prefer broad fields of carbon or paper to multicolor gradients.
- Decorative gradients are restricted to subtle depth in WebGL or motion
  backgrounds and must not sit behind body text.

### Accessible pairs

Approved default pairs:

- Paper text on Carbon.
- Carbon text on Paper.
- White text on Cobalt 600 for large or bold controls.
- Cobalt 600 links on Paper, with underline or another non-color cue.
- Cobalt 400 is reserved for dark surfaces.

Every new color pair must be tested against the applicable WCAG contrast
threshold before publication.

## 5. Typography

### Families

- **Display and body:** Instrument Sans variable.
- **Code, labels, data, and metadata:** Fragment Mono.
- **Fallback body:** Arial, sans-serif.
- **Fallback code:** Consolas, monospace.

Instrument Sans gives the identity a precise but human voice. Fragment Mono
makes compiler traces and evidence artifacts feel native to the system rather
than pasted into the interface.

### Scale

| Role | Suggested size | Guidance |
| --- | --- | --- |
| Display | `clamp(3.5rem, 10vw, 9rem)` | Tight tracking, 650–760 weight |
| H1 | `clamp(2.75rem, 7vw, 6rem)` | One idea, no forced line breaks on mobile |
| H2 | `clamp(2rem, 4vw, 4rem)` | 650–720 weight |
| H3 | `1.25rem–1.75rem` | 650 weight |
| Body large | `1.125rem–1.375rem` | Maximum 65 characters per line |
| Body | `1rem` | 1.5–1.65 line height |
| Label | `0.6875rem–0.8125rem` | Mono, uppercase, 0.06em tracking |
| Code | `0.8125rem–0.9375rem` | 1.55–1.7 line height |

Sentence case is the default. Uppercase is reserved for compact mono labels,
not headings or paragraphs.

## 6. Layout and shape

### Grid

The base unit is 8 px. Major layouts use a 12-column grid and visible alignment
lines. The identity should feel engineered, not boxed into a dashboard.

### Shape language

- Controls: 4 px radius.
- Panels: 12 px radius.
- Editorial media and large WebGL stages: square or 12 px radius.
- Pills are reserved for statuses, filters, and compact metadata.
- Borders are preferred to decorative shadows.

### Signature composition

The signature layout is **the cascade rail**: a vertical or horizontal sequence
where authored input, semantic resolution, emitted CSS, and evidence occupy
different aligned tracks. One rail may become sticky while the other changes
on scroll. This pattern should appear at least once in major PliegoCSS
experiences.

## 7. Motion

Motion explains compilation and causality. It is not ambient ornament.

### Principles

1. **Fold:** authored and emitted rails converge on a semantic hinge.
2. **Resolve:** conflicting inputs settle into one explicit state.
3. **Trace:** evidence follows the same path as the artifact it describes.
4. **Return:** reversible actions visibly restore the prior state.

### Timing

- Control response: 120–180 ms.
- Panel transition: 240–360 ms.
- Section reveal: 420–600 ms.
- Hero choreography: no more than 1,200 ms before primary content is usable.
- Default ease: `cubic-bezier(0.22, 1, 0.36, 1)`.

Avoid infinite text movement, scroll hijacking, motion that obscures code, and
multiple competing animation systems. Lenis may smooth wheel input, but native
keyboard navigation and anchor semantics remain authoritative.

### Reduced motion

When `prefers-reduced-motion: reduce` is active:

- disable Lenis interpolation;
- remove parallax and camera travel;
- render Three.js at a static representative frame or replace it with SVG;
- preserve opacity changes below 150 ms only when they communicate state;
- keep every interaction and piece of content available.

## 8. Imagery and 3D

PliegoCSS does not use generic developer stock photography or abstract neon
blobs. Imagery is generated from product concepts:

- folded planes;
- cascade rails;
- semantic graphs;
- source-to-output diffs;
- evidence envelopes and hash-linked artifacts;
- macro views of paper fibers only when they remain technical and restrained.

Three.js scenes should use orthographic or shallow-perspective folded planes,
subtle material response, and cobalt edge lighting. The scene must support the
content hierarchy and respect device budgets. WebGL is progressive enhancement,
not a requirement for documentation.

### Generated-image collection

The canonical GPT Image 2 specifications live in `brand/image-prompts/`. The
collection has four roles: cascade chamber, semantic fold, evidence archive,
and material studies. A generation is not canonical merely because it followed
the prompt. It becomes a brand asset only after visual review, responsive crop
review, accessibility assignment, and a committed prompt receipt.

The current canonical collection was generated with `openai-gpt-image-2`,
reviewed on 2026-07-20, and recorded in
`brand/images/generation-manifest.json`. Its six PNG masters are:

- `cascade-chamber` — hero environment and authored-to-compiled transition;
- `semantic-fold` — editorial explanation of semantic lowering;
- `evidence-archive` — provenance, receipts, and authority;
- `material-study-carbon` — authored-source material;
- `material-study-paper` — readable evidence material;
- `material-study-cobalt` — compiled-artifact material.

AVIF and WebP files are delivery derivatives, not independent masters. Regenerate
them from the reviewed PNG and keep the manifest hashes, dimensions, alt-text
assignment, and prompt receipt aligned.

Generated images never contain product claims, code, interface screenshots, or
the logo. HTML carries all meaningful text. The image system supports the
product narrative; it does not replace real interactive demonstrations.

On the website, generated imagery should bridge spatial modes:

- Carbon laboratory to Paper workbench;
- semantic fold to emitted Cobalt field;
- evidence archive to measured status.

Do not place an isolated image in a generic card. Crop it as an editorial field,
align its structural fold to the layout grid, and connect it to the live
cascade rail.

### Digital composition modes

Major experiences rotate through four related modes instead of repeating one
dark dashboard:

1. **Laboratory** — Carbon field, active compiler state, sparse controls.
2. **Workbench** — Paper field, readable documentation, direct manipulation.
3. **Compilation field** — Cobalt field, emitted output, large type.
4. **Archive** — restrained Carbon/Paper evidence and provenance.

Every mode preserves typography, square fold geometry, and semantic color
roles. This is one identity changing function, not four unrelated themes.

## 9. UI application

### Buttons

- Primary: Cobalt field, white label, rectangular 4 px radius.
- Secondary: transparent, Carbon/Paper border.
- Tertiary: text link with directional cue.
- Focus: 2 px Cyan outline with at least 2 px offset.

### Code and evidence

- Separate authored code, emitted CSS, and evidence using labels and rail color,
  not only syntax highlighting.
- Every interactive compile example must have a copy action, deterministic
  reset, error state, and keyboard path.
- Evidence IDs and hashes use Fragment Mono and may wrap at explicit break
  opportunities.

### Status language

Use these exact status concepts:

- **Measured:** reproduced in the current evidence boundary.
- **Inherited:** supported by prior accepted evidence but not re-run here.
- **Pending:** defined work without passing evidence.
- **Uncertain:** insufficient evidence to make a claim.

Never substitute “complete” when the evidence only proves a local or partial
surface.

## 10. Repository and ecosystem

### Badges

The repository may show CI, CodeQL, crates.io, docs.rs, release, license, MSRV,
and release-status badges. Badges must resolve to live targets; unpublished
package badges remain absent until the corresponding release exists.

### Relationship to PliegoRS

PliegoCSS and PliegoRS are sibling products:

- both use an open folded geometry and square joins;
- both use Carbon/Paper as neutral anchors;
- PliegoRS owns Terminal red and append-only event semantics;
- PliegoCSS owns Cobalt and the authored-to-emitted cascade;
- their symbols must not be combined into a new lockup.

### Domain use

Use `pliegocss.dev` for product and documentation URLs once the domain is
configured and verified. Until then, repository links must not imply that a
site is already live.

### Public-preview provenance

The website status rail uses the canonical line:

`PUBLIC PREVIEW / MEDELLÍN / 2026`

The footer identifies PliegoCSS as an independent Celiums Solutions LLC
project, includes `© 2026 Celiums Solutions LLC`, credits Medellín, and states
the Apache-2.0 software license. Public preview describes product maturity; it
does not override the release-readiness record or authorize publication.

## 11. Asset governance

- SVG files in this directory are the masters.
- Raster derivatives must be regenerated from the exact committed SVG.
- Brand-token changes require updating both DTCG JSON and CSS projections.
- Material identity changes require a changelog entry and visual regression.
- Product source availability does not grant trademark rights.

The brand system is versioned with the repository. Public identity claims begin
only after the corresponding release and site are deliberately published.
