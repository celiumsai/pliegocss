# Fixture contracts

The Tailwind and PliegoCSS versions must preserve DOM element order, text, form controls, ARIA
attributes, theme values, and state. Only class/style authoring may differ.

## Button

- Primary and disabled secondary actions.
- Inline alignment, horizontal and vertical spacing, radius, border, shadow, and color.
- Verify base, hover, focus-visible, and disabled.
- The disabled control must remain a native disabled button.

## Card

- Surface, media placeholder, eyebrow, heading, and body copy.
- Column layout on mobile and row layout from the medium breakpoint.
- Verify border, shadow, spacing, max text width, and responsive media size.

## Navbar

- Brand link, two navigation links, and one call-to-action.
- Column layout on mobile and distributed row layout from the medium breakpoint.
- Verify accessible navigation label and link hover states.

## Form

- Text input, select, textarea, cancel action, and submit action.
- One column on mobile and two columns from the medium breakpoint.
- Verify placeholder, focus, border, ring, resize behavior, and native form semantics.

## Dashboard

- Four metric cards.
- One, two, and four columns at the frozen breakpoints.
- Verify repeated-rule deduplication, one responsive span, typography, and spacing.

## Viewports and states

- 375×812.
- 768×1024.
- 1440×900.
- Base screenshot after fonts and transitions settle.
- Hover on primary interactive target.
- Keyboard focus-visible on primary interactive target.
- Disabled state where present.

Automated verification must disable or await transitions, block external network requests, compare a
normalized DOM, and inspect computed styles at named anchor elements. Pixel comparison is secondary to
DOM and computed-style equivalence.
