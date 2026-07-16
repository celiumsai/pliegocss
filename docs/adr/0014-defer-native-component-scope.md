# ADR-0014: Defer native component scope from 0.1.0

## Status

Accepted. Component scope remains `experimental/error` in every compatibility-policy schema-1
profile and is explicitly deferred beyond `0.1.0`.

## Context

The research contract asks PliegoCSS to model component scope rather than relying on conventions.
The standards-aligned primitive is native `@scope`: it establishes scoping roots and optional
limits, affects selector matching, and adds scope proximity to cascade ordering. Those semantics are
defined by the W3C [CSS Cascading and Inheritance Level 6 draft](https://www.w3.org/TR/css-cascade-6/#scope-atrule).

The current browser contract cannot support that primitive. WebDX reports
[`@scope`](https://web-platform-dx.github.io/web-features-explorer/features/scope/) as Baseline Newly
Available only since 2025-12-12, with minimum support at Chrome/Edge 143, Firefox 146, and Safari/iOS
Safari 26.2. It is expected to become Widely Available on 2028-06-12. PliegoCSS policy schema 1
deliberately freezes `baseline-widely` at Chrome/Edge 120, Firefox 121, and Safari/iOS Safari 17.2.

The pinned Lightning CSS 1.0.0-alpha.71 source parses, minifies, and serializes `CssRule::Scope`, but
does not lower `@scope` into equivalent selectors for older targets. Parsing is therefore not a
compatibility proof.

## Decision

Do not add a `scope-*` authoring variant, `@scope` emitter path, or policy allowance in `0.1.0`.
Keep the existing `component-scope` decision as `experimental/error` for `baseline-widely`,
`modern`, and `none`. Raw at-rules are not accepted through utility syntax, so no profile silently
inherits unsupported component-scope behavior.

Reject the following substitutes as mislabeled component scope:

- prefixing a generated class with an ancestor attribute, because nested scopes continue matching
  and no scope limit or proximity exists;
- stamping a component attribute on every styled element, because that is exact subject tagging,
  not subtree scope, and the current ID-only `Style` handle cannot require adapter stamping;
- using cascade layers, because precedence does not constrain selector matching;
- claiming Shadow DOM or CSS Modules semantics for a standard external stylesheet.

Revisit component scope only when all of these are true:

1. the selected compatibility profile supports native `@scope`, or a reviewed transformation proves
   equivalent root, limit, nesting, specificity, and proximity behavior;
2. schema-5 physical tracing supports final `scope` group rules and exact nested ranges;
3. PliegoRS and framework-neutral adapters expose stable component root/limit ownership through the
   shared Project Index contract; and
4. browser behavior tests cover overlapping roots, nested same/different components, limits,
   layers, important declarations, media/container conditions, and scope proximity.

## Consequences

- `0.1.0` keeps its frozen Baseline claim and fails closed instead of emitting ignored CSS.
- A3 is closed only for the explicitly scoped `0.1.0` variant set; native component scope is a
  reviewed post-0.1 capability.
- A5 still requires the hosted browser corpus, but no longer lists component-scope emission as an
  unplanned blocker for `0.1.0`.
- The implementation can proceed to A7 without reserving a misleading syntax or wire marker.

## Rejected alternative: allow `@scope` only under `--targets none`

Rejected for `0.1.0` because the host contract and physical trace would still be incomplete, and
supporting syntax in only the unmanaged profile would make the default/strict build fail after Rust
macros had already accepted the style. A later capability tier may introduce this deliberately with
complete adapter and trace contracts.

See [compatibility policy schema 2](../reference/compatibility-policy.md) and the
[research traceability matrix](../product/research-requirements.md).
