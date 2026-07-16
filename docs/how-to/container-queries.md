# Use typed container queries

PliegoCSS supports native inline-size container queries without a browser runtime. The element that
owns the available space establishes containment; descendants opt into a named theme breakpoint.

## Establish the container

Use `container-inline` on the layout boundary:

```rust,ignore
let shell = pc!("container-inline rounded-lg border p-4");
```

It emits `container-type:inline-size`. Use `container-normal` to remove that containment in a
conditional branch or composed style. These utilities conflict when assigned to the same condition,
just like two values for any other typed semantic slot.

## Query its inline size

Prefix descendant utilities with `cq-<breakpoint>:`:

```rust,ignore
let card = pc!("grid gap-4 cq-sm:grid-cols-2 cq-lg:grid-cols-3");
```

The seed mappings are:

| Variant | Minimum container width |
|---|---:|
| `cq-sm:` | `40rem` |
| `cq-md:` | `48rem` |
| `cq-lg:` | `64rem` |

The names and widths come from the active `ThemeRegistry`. If a project changes `sm` to `32rem`,
both `sm:` viewport conditions and `cq-sm:` container conditions resolve to `32rem`. This keeps one
typed responsive scale; it does not hard-code a second hidden container scale.

## Compose conditions

Container conditions commute with viewport, theme, preference, state, and classified selector
dimensions:

```rust,ignore
pc!("md:cq-sm:rtl:aria-expanded:hover:flex")
```

Reordering those commuting prefixes produces the same StyleId and CSS. Native output places the
viewport media query outside the container query:

```css
@media (min-width:48rem) {
  @container (min-width:40rem) {
    .pc_<id>:hover:dir(rtl)[aria-expanded=true] { display:flex }
  }
}
```

Lightning CSS may serialize the equivalent min-width condition as `(width>=40rem)` for a selected
target profile. That printer change does not alter PliegoCSS semantic identity.

## Diagnostics

- `cq-sm:cq-md:grid` fails with `PCS012`: one condition cannot contain two container breakpoints.
- `cq-mx:grid` fails with `PCS003` and suggests the nearest active-theme name when one exists.
- A theme cannot define a breakpoint whose name starts with `cq-`; that prefix is reserved.
- A theme cannot shadow a built-in selector/direction variant such as `rtl` or `aria-expanded`.

Typed container conditions were introduced by policy 3 and remain allowed by the current
compatibility policy schema 2 / policy 7, including `baseline-widely`. Arbitrary properties such as
`[container-type:inline-size]` remain unclassified
and therefore fail with `CMP002` under that strict profile; prefer `container-inline`.

## Host CSS interop

The query selects the nearest eligible ancestor according to native CSS rules. PliegoCSS does not
invent component boundaries or container names. A host stylesheet may establish
`container-type:inline-size` itself, and PliegoCSS descendants can still use `cq-*`. Conversely,
normal CSS may query an ancestor established by `container-inline`.

Current alpha scope is size queries against the nearest eligible container. Named containers,
style/scroll-state queries, component scope, and `@scope` are outside the `0.1.0` contract and must
not be inferred from this feature. Typed cascade layers are independent of container selection. See
[ADR-0014](../adr/0014-defer-native-component-scope.md).
