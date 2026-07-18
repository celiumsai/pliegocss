# PliegoCSS paused experimental checkpoint

**Decision date:** 2026-07-18  
**Release state:** unreleased `0.0.0`  
**Product state:** paused; no active `0.1.0` commitment

## Decision

PliegoCSS is preserved as an experimental research workspace, not continued as a second active
product beside PliegoRS. The repository is not deleted, merged into PliegoRS, or presented as a
finished MVP. Existing contracts remain available for later evaluation through the PliegoRS
OpenSDK build-plugin boundary.

PliegoRS continues with standards-compliant CSS and may integrate external CSS pipelines without
requiring PliegoCSS.

## Checkpoint scope

The pause checkpoint closes one bounded slice that was already in progress:

1. critical-style evidence remains hash-bound to the exact usage universe and reachability bytes;
2. the CLI verifies route and bundle membership against the generated Asset Plan;
3. `bundle --critical-evidence` emits deterministic per-route critical CSS;
4. `pliego.critical.json` binds evidence, universe, reachability, theme, target, format, route,
   StyleIds, CSS byte lengths, filenames, and hashes;
5. the critical outputs participate in the existing grouped publication and `--check` contracts;
6. invalid evidence leaves the previous valid output unchanged.

This does not close the real Chromium producer, hosted browser measurements, multi-browser proof,
field performance, documentation onboarding, cross-platform release evidence, or the broader R0
product gate.

## Verification

Verified locally on Windows at the checkpoint worktree:

```powershell
cargo fmt --all -- --check
cargo clippy -p pliego-css-usage -p pliego-cssc --all-targets --all-features --locked -- -D warnings
cargo test -p pliego-css-usage -p pliego-cssc --locked
```

The tests include canonical manifest parsing, strict CLI option handling, deterministic projection,
read-only check mode, exact CSS hash/length validation, repeated-build equality, and rejection of
invalid evidence without output mutation. This is focused package evidence, not a new claim that
the complete workspace, hosted matrix, or MVP is closed.

## Reactivation gates

Development resumes only after an explicit product decision confirms all of the following:

1. PliegoRS P8 and the OpenSDK build-plugin foundation are closed with evidence.
2. PliegoCSS can run as an optional plugin without becoming a PliegoRS dependency.
3. External PliegoRS users demonstrate CSS pains not sufficiently addressed by existing pipelines.
4. A reduced, maintainable first-release scope is accepted.
5. At least five representative applications demonstrate measurable diagnostic or output value.
6. Release, security, documentation, compatibility, and support work can be sustained without
   delaying PliegoRS.

Until those gates are met, open backlog items remain historical and no release date is implied.
