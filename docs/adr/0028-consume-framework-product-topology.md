# ADR-0028: Consume framework-owned product topology

- Status: Accepted for RC.3 source preview
- Date: 2026-08-26

## Context

PliegoRS owns components, routes, islands, and Cargo source-unit registration.
PliegoCSS owns style identity, source scanning, reachability, physical bundle
planning, and artifact evidence. The original integration fixture converted
PliegoRS Rust types in application code, duplicating the adapter and tying the
proof to one dependency graph.

## Decision

`pliego-css-source` consumes the closed `pliegors-product-topology/1` JSON
snapshot. It validates framework-owned IDs, paths, route-to-island references,
and source units, then emits canonical reachability and a deterministic bundle
plan without linking any PliegoRS crate.

Physical bundle IDs remain PliegoCSS-owned. Framework IDs that are not already
safe kebab-case are mapped through a domain-separated SHA-256 prefix instead of
being copied into artifact filenames.

The snapshot does not authorize deployment, infer Cargo coverage, select an
application theme, or publish output. The initial source preview renders a seed
theme bundle plan for the executable fixture; non-seed theme, target, and
format policy must use the normal application-owned bundle-plan surface. A
complete source inventory and normal PliegoCSS artifact gates remain separate
inputs.

## Consequences

- PliegoRS declares product topology once.
- PliegoCSS remains optional and runtime-free.
- Both repositories can evolve in parallel against one versioned wire seam.
- Asset Plan verification and single-owner dev orchestration remain later
  coordinated milestones.
