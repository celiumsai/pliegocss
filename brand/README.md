<!-- SPDX-License-Identifier: Apache-2.0 -->

# PliegoCSS brand assets

This directory is the source of truth for the PliegoCSS visual identity. The
system is built from one 64 × 64 symbol, a restrained technical palette, and an
8-pixel layout rhythm.

## Primary assets

| Asset | Intended use |
| --- | --- |
| `pliegocss-symbol.svg` | Light surfaces at 24 CSS pixels or larger |
| `pliegocss-symbol-reversed.svg` | Dark surfaces at 24 CSS pixels or larger |
| `pliegocss-symbol-monochrome.svg` | Single-color printing or constrained integrations |
| `pliegocss-lockup.svg` | Light-surface horizontal lockup |
| `pliegocss-lockup-reversed.svg` | Dark-surface horizontal lockup |
| `pliegocss-app-icon.svg` / `.png` | Square product icon at 32 pixels or larger |
| `favicon.svg` | Browser favicon |
| `social-card.svg` / `.png` | 1200 × 630 Open Graph and social preview |
| `brand-tokens.json` | DTCG design-token source |
| `brand-tokens.css` | CSS custom-property projection |
| `images/` | Hash-bound GPT Image 2 masters and receipt contract |

The raster exports are derived from the committed SVG masters. Do not edit the
PNG files directly.

## Symbol contract

The mark is a folded cascade: the carbon rail is authored input; the cobalt
rail is the deterministic projection. The two paths share a middle fold rather
than forming a letter from a typeface.

- Keep clear space equal to one stroke width (`8/64` of the symbol width).
- Do not round, rotate, skew, outline, add shadows, or recolor individual
  segments.
- Do not place the two-color symbol on a surface where either rail falls below
  accessible non-text contrast.
- Use the monochrome mark when production cannot preserve the two-color
  relationship.

The complete identity and usage rules are in
[`BRANDBOOK.md`](./BRANDBOOK.md).

The included Instrument Sans and Fragment Mono font files are distributed
under the SIL Open Font License 1.1. Their license texts are preserved in
[`fonts/`](./fonts/).

The art-directed image specifications are in
[`image-prompts/`](./image-prompts/). Until
[`images/manifest.json`](./images/README.md) exists and passes
`pnpm check:brand`, generated imagery remains explicitly pending.

Source availability does not grant trademark rights. See
[`TRADEMARKS.md`](../TRADEMARKS.md).
