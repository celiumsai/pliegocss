<!-- SPDX-License-Identifier: Apache-2.0 -->

# Contributing to PliegoCSS

PliegoCSS accepts focused, reviewable contributions once the repository is
public. Open an issue before changing public APIs, serialized formats,
dependencies, security boundaries, or behavior spanning multiple crates.

## Development setup

Install the toolchains declared by `rust-toolchain.toml`, Node.js 22.13 or
newer, and pnpm 11.7.

```console
pnpm install --frozen-lockfile
pnpm verify:fast
pnpm check:release-contract
```

Run the narrowest relevant test while developing. Before requesting review,
run every applicable fast and integration gate described in the
[release process](docs/contributing/release-process.md).

## Change contract

- Preserve deterministic output, fail-closed analysis, bounded child
  processes, and transactional publication or migration behavior.
- Add regression tests for fixes and contract tests for serialized formats.
- Keep source, CLI help, examples, generated catalogs, and documentation in
  sync.
- Explain every production dependency's purpose, license, maintenance state,
  and size impact.
- Do not commit credentials, private datasets, generated build output, or
  third-party media without redistribution rights.
- Call out compatibility, performance, security, and evidence effects.

By submitting a contribution, you represent that you may submit it and agree
that it is licensed under Apache-2.0. Participation is governed by
[CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md). Security reports follow
[SECURITY.md](SECURITY.md), never the public issue tracker.
