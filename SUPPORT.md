<!-- SPDX-License-Identifier: Apache-2.0 -->

# Support policy

## Documentation first

Read the [project documentation](docs/index.md), CLI help, and
[troubleshooting guides](docs/troubleshooting/lsp.md) before opening a request.
Include stable diagnostic identifiers whenever one is emitted.
The exact certified adapter/version matrix and explicit exclusions are published in the
[0.1.x adapter support policy](docs/reference/adapter-support-policy-0.1.x.md).

## Public support

Once the repository is public, use GitHub Issues for reproducible compiler,
integration, or documentation defects. Search existing issues first and
include:

- PliegoCSS version or commit;
- operating system, architecture, Rust and browser versions when relevant;
- a minimal reproduction;
- complete sanitized diagnostics;
- expected and observed behavior.

General Rust or CSS consulting, private-project debugging, and requests without
a reproduction may be redirected or closed.

Use `hello@pliegocss.dev` for private project correspondence.
Vulnerabilities must follow [SECURITY.md](SECURITY.md). PliegoCSS is provided
under Apache-2.0 without a support SLA or warranty.
