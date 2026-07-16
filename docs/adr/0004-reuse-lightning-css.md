# ADR-0004: Reuse Lightning CSS downstream

Status: accepted for the spike

## Decision

PliegoCSS owns utility semantics, validation, reachability, and deterministic generation. Lightning
CSS handles standards-oriented lowering, browser targets, prefixes, and minification downstream.

## Consequences

- PliegoCSS does not spend the MVP rebuilding a general CSS parser and minifier.
- The integration is isolated so benchmark or compatibility evidence can replace the backend later.
