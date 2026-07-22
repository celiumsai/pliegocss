# ADR-0025: Distribute the Node launcher from immutable repository releases

- Status: Accepted for the G5 distribution architecture
- Date: 2026-07-22

## Context

PliegoCSS needs convenient CLI and LSP installation without making npmjs an authority for native
release bytes. Installing a Git dependency or mutable remote tarball directly is also too weak for
the LTS boundary: it does not prove that a reviewed release owns the exact bytes. Lifecycle download
scripts would add another executable supply-chain boundary during package installation.

The Rust library graph remains an exact-version crates.io compatibility unit. This decision applies
only to the optional Node/native distribution.

## Decision

PliegoCSS will build one npm-format package named `@pliegocss/cli`, but will never publish it to
npmjs. The source manifest is `private: true`. The packed artifact has no dependencies and no
`preinstall`, `install`, `postinstall`, `prepare`, or publication lifecycle scripts.

The package embeds `pliego-cssc` and `pliego-css-lsp` for exactly:

- `x86_64-pc-windows-msvc`;
- `x86_64-unknown-linux-gnu`;
- `aarch64-apple-darwin`.

Small Node launchers select the declared host and execute the adjacent native binary without a shell,
network request, mutable lookup, or environment override. Unsupported host pairs fail explicitly and
use Cargo instead.

The canonical distribution channel is a draft-first immutable GitHub Release. Every candidate asset
set includes native archives, the universal pnpm tarball, SHA-256 checksums, CycloneDX JSON SBOMs, a
machine distribution manifest, and GitHub/Sigstore provenance attestations. A tag-triggered workflow
must fail before draft creation if repository release immutability is disabled.

The supported consumer sequence is download → release-asset verification → attestation verification
→ local `pnpm add --save-exact`. Direct npmjs installation, a moving branch or tag, and an unverified
remote tarball are outside the support boundary.

## Consequences

- npmjs cannot become a second mutable release authority.
- Package installation runs no third-party or PliegoCSS lifecycle code.
- One lockable artifact works across the three declared hosts at the cost of carrying all six native
  binaries.
- pnpm is the supported and recommended Node package manager; the tarball format remains interoperable
  for inspection but other managers are not release-tested.
- GitHub/Sigstore provenance proves source/workflow identity. It does not claim Authenticode signing,
  Apple notarization, or binary support beyond the declared matrix.
- RC.2 cannot inherit this later-source evidence. A new exact-source candidate must build, attest,
  install, and publish its own immutable asset set.

## Verification

```console
pnpm check:repository-distribution
pnpm check:distribution-bundle -- --root=<release-assets>
pnpm check:distribution-install -- --tarball=<verified-local-tarball>
```

Hosted proof is owned by `.github/workflows/distribution.yml`. Release authority remains fail-closed
until the exact promotion candidate references unexpired hashed G5 artifacts and successful
attestation/install runs.
