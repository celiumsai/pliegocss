# Frozen benchmark evidence

Files in this directory are immutable machine snapshots produced by the benchmark harnesses from a
clean Git commit. They record the source commit, worktree state, harness hash, toolchain, host,
power-plan probe, declared security tooling, every timing sample, and input/output hashes.

Use a filename containing the gate, UTC date, and measured source commit, for example:

```text
pliego-gate-a-2026-07-13-0123456.json
```

Generate a new file; never overwrite an existing snapshot:

```console
node scripts/measure-pliego-gate-a.mjs --evidence benchmarks/evidence/<snapshot>.json
node scripts/measure-pliego-gate-b.mjs --evidence benchmarks/evidence/<snapshot>.json
node scripts/measure-rust-check.mjs --evidence benchmarks/evidence/<snapshot>.json
```

If an audit supersedes a snapshot, remove it from the current approved set without editing its
contents; the original bytes remain preserved in Git history.

Evidence mode rejects dirty worktrees, paths outside this directory, linked parents, existing
destinations, and undeclared security-tooling state. Machine-local exploratory output remains under
`benchmarks/results/` and is ignored by Git.

Verify all committed snapshots and their source-commit harness blobs with:

```console
pnpm check:evidence
```

The verifier needs Git history for the recorded commits. It recomputes statistics, Tailwind
comparison arithmetic, and every paired Rust delta; it does not rerun performance measurements.
The approved filename/SHA-256 set is explicit in the verifier, so adding or replacing evidence
requires an intentional review of both the immutable snapshot and its allowlist entry.
Evidence-mode harnesses also require every recorded harness, fixture, and lock input to match the
exact Git blob at the clean source commit; Git's clean status alone is not treated as byte identity.
Once evidence is approved, its recorded source commit must remain reachable: do not squash, rebase,
or otherwise rewrite that commit without superseding the snapshots in the same reviewed change.

## Current clean-commit snapshots

- [`pliego-gate-a-2026-07-13-c47239c.json`](./pliego-gate-a-2026-07-13-c47239c.json)
- [`pliego-gate-b-2026-07-13-c47239c.json`](./pliego-gate-b-2026-07-13-c47239c.json)
- [`rust-check-2026-07-13-c47239c.json`](./rust-check-2026-07-13-c47239c.json)

All three were produced from commit `c47239ce0bf1d046aa01710d436baea61f4e6528` on Windows 11,
Intel Core Ultra 9 285H, Balanced power plan, with Microsoft Defender and Windows Application
Control active. The JSON files retain the complete samples and exact environment metadata.
