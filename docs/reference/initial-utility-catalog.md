# Initial utility catalog

F0 defines families, not every concrete token expansion. `gap-{space}` is one family and may accept
several theme values.

## Layout

- `block`, `inline-block`, `flex`, `inline-flex`, `grid`, `hidden`
- `flex-row`, `flex-col`, `flex-wrap`
- `items-start`, `items-center`, `items-end`
- `justify-start`, `justify-center`, `justify-between`
- `grid-cols-{1..12}`, `col-span-{1..12}`

## Spacing

- `gap-{space}`
- `p-{space}`, `px-{space}`, `py-{space}`
- `m-{space}`, `mx-{space|auto}`, `mt-{space}`

## Sizing

- `w-{size}`, `h-{size}`, `max-w-{size}`, `min-h-{size}`

## Typography and paint

- `text-{font-size}`
- `font-{weight}`
- `leading-{line-height}`
- `text-{color}`
- `bg-{color}`

## Borders and effects

- `border`, `border-{width}`, `border-{color}`
- `rounded-{radius}`
- `shadow-{shadow}`
- `ring-{width}`, `ring-{color}`
- `opacity-{opacity}`

Prefixes that accept multiple domains are resolved through the theme catalog. A numeric color token
or a color named `sm` is rejected if it would make `text-*`, `border-*`, or `ring-*` ambiguous.

## Seed tokens

- Spacing: `0`, `1`, `2`, `3`, `4`, `6`, `8`.
- Font sizes: `sm`, `base`, `lg`, `xl`.
- Weights: `medium`, `semibold`, `bold`.
- Line heights: `tight`, `normal`.
- Radius: `sm`, `md`, `lg`, `full`.
- Shadow: `sm`, `md`.
- Opacity: `50`, `60`.
- Ring width: `1`, `2`.

The Tailwind baseline currently uses 90 unique concrete utilities. F1 begins with these 40 semantic
families, then expands only where the fixtures and PliegoRS integration require it.
