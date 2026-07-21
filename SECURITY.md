<!-- SPDX-License-Identifier: Apache-2.0 -->

# Security policy

The canonical online trust center will be
<https://pliegocss.dev/security/>. Until it is deployed, this file is the
authority.

## Supported versions

Before the first stable release, security fixes target the latest prerelease
and the default branch.

| Version | Supported |
| --- | --- |
| `0.1.0-rc.1` | Yes |
| Earlier snapshots | No |

## Report a vulnerability

Do not open a public issue. Use GitHub private vulnerability reporting or email
`hello@pliegocss.dev` with subject `SECURITY: short description`. Include the
affected version or commit, crate or command, minimal reproduction, impact,
prerequisites, and any known mitigation. Do not include unrelated credentials,
personal data, or proprietary source.

We aim to acknowledge a complete report within three business days and provide
an initial assessment within seven business days. These are response goals, not
a service-level agreement. Disclosure timing is coordinated after a fix is
available.

## Scope

Security-sensitive surfaces include source discovery, parsers, the compiler,
the LSP, migration and repair operations, publication locks, manifests,
receipts, browser validation, build scripts, and generated artifacts.

Good-faith research must avoid unauthorized access, privacy violations, data
destruction, and service degradation. This policy does not authorize testing a
system you do not own or have permission to test.

## Dependency maintenance

CI enforces `cargo audit`, `cargo deny`, pinned lockfiles, and CodeQL when the
repository is public. A clean advisory report is evidence about known
disclosures only; it is not proof that no vulnerability exists.
