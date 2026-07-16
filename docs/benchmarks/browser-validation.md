# Browser validation

Date: 2026-07-13

Status: Chromium smoke contract passed

The StyleId format-2 migration was revalidated against manifest schema 3 and the complete CSS
artifact `11f632e3e4321ab93a2c8df77e7ad1ce6034f912cbec48e62b92654f01a700fe`.
The canonical-stream rule order preserved the core, responsive, disabled, and focus assertions
below. The dedicated dark-theme assertion remains evidence from the initial smoke run.

The generated Gate-A and full fixture artifacts were loaded in the Codex in-app Chromium browser
through an isolated `127.0.0.1` server. The HTML retained the fixture DOM and replaced only utility
lists with manifest-provided `pc_*` class names. No remote assets or network dependencies were used.

## Core computed styles

At 1440×900, representative computed values matched the semantic input:

- body: `min-height: 900px`, seed canvas/ink OKLCH colors, and the system sans stack;
- primary button: flex layout, `8px` gap, `16px`/`8px` padding, `14px` type, weight `600`, and
  `8px` radius;
- card: column flex layout, `24px` gap/padding, solid `1px` border, surface background, and the
  composed seed shadow;
- form and dashboard: grid layout with the expected `24px` and `16px` gaps.

## Responsive contract

The full five-fixture artifact was checked at the three frozen methodology viewports:

| Viewport | Evidence |
|---|---|
| 375×812 | Main inline padding `16px`; card column; form and dashboard one column. |
| 768×1024 | Main inline padding `24px`; card row and centered; form and dashboard two columns. |
| 1440×900 | Main inline padding `32px`; dashboard four columns; stat cards retain solid borders and shadow. |

## State, pseudo-element, and theme contract

- Disabled button computed to `cursor: not-allowed` and opacity `0.5`.
- Placeholder text resolved to the seed muted OKLCH color.
- Focusing the project-name input produced the accent border, no native outline, a `2px` composed
  ring, and the expected 20% accent color mix.
- A dedicated `[data-theme="dark"]` fixture changed surface background to seed ink and foreground to
  white.

## Remaining browser work

This is a real computed-style smoke test, not the final compatibility matrix. Hover, reduced-motion,
contrast preferences, prefix behavior, Firefox/WebKit, visual screenshots, and automated regression
execution in CI remain open. The CLI already freezes explicit `modern` Lightning CSS targets for
Chrome/Edge 111, Firefox 128, and Safari 16.4; those configured targets still require this behavioral
certification.
