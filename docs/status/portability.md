# Portability contract

Updated: 2026-07-22

Status: **executable contract and hosted CI matrix implemented; release-candidate binding remains
open**

## Authority

`.github/workflows/ci.yml` runs the same portability contract on Ubuntu, Windows, and macOS with
Rust 1.85.0 and 1.96.0. Node.js 22.13.0 executes the byte-level vector on every Rust/OS lane. A run
is evidence only for its exact `head_sha`; a green branch run does not certify an older tag and does
not promote `0.1.0-rc.2`.

The separate browser-output workflow owns the G4 Windows/Linux/macOS × Chromium/Firefox/WebKit
matrix. Portability proves compiler and artifact identity; browser output proves rendered behavior.
Neither substitutes for a named release-candidate replay.

The WASM job additionally checks the public crate with Rust 1.85.0 for
`wasm32-unknown-unknown`. This is not a WASI, `no_std`, every-WASM-target, or deployed Cloudflare
Workers claim.

## Frozen executable vector

`node scripts/check-portability.mjs` builds an isolated project and fails closed unless all of these
properties hold:

- identical CSS and manifest bytes from unrelated process directories;
- a path containing a space, UTF-8 `src/café.rs`, an astral character, and CRLF source bytes;
- exact logical provenance and macro byte ranges with forward-slash paths;
- CSS byte count and SHA-256 bound by the adjacent style manifest;
- read-only `--check`, including missing-output and drift failures;
- rejection of symlink/junction escape, linked output/theme paths, dot segments, and
  ASCII-case aliases before publication;
- exact direct-audit, bundle-control, Asset Plan, compile, watch, and DTCG control groups;
- compact canonical `pliegocss-token-graph/1` bytes, reciprocal artifact relationships, and receipt
  checks;
- complete Resolver 2025.10 projection: 12 sources, four canonical selections, 53 tokens per theme,
  one alias, one deprecation, and `{ "color": 3 }` DTCG inventory;
- selection identity in `configHash` even when the resulting ThemeId and artifact bytes converge.

The current shared-engine vector freezes:

| Artifact | Frozen value |
|---|---:|
| CSS bytes | 424 |
| CSS SHA-256 | `65dbbcff74b6886fdd749b1c7c0587eaff06b4f4720405e10716c2b0dfee9bbc` |
| Style manifest SHA-256 | `453f708c9c4edbd388eda1ee9c883212cd5d31421077d123cb1d32bc0b28d4bb` |
| Seed TokenGraph SHA-256 | `3298873e66ade310459593530c86419c05a973d2404b71882c2d0ae430030a28` |
| Provenance path | `src/café.rs` |

The generation/control groups freeze these identities in addition to their complete per-file maps
inside `scripts/check-portability.mjs`:

| Group | CSS or primary artifact | Control manifest | Receipt |
|---|---|---|---|
| Direct CSS audit | `79e4817107cbb7a20b5c0be1a8a881b11ebfc3f9e94f83a93f9d318f74059cf9` findings | `711c183edb566062d3feec8e7a7e42117a8b6b6b0d05d237afc0469abd09d78b` | `1162d5743e2ccd7d03614605a700cd55099bde9380c19df5d95d9cfb65875479` |
| Bundle control | `65dbbcff74b6886fdd749b1c7c0587eaff06b4f4720405e10716c2b0dfee9bbc` CSS | `cf7e29c1a78cc63120b53a0b88b7c9395edd51f9322a0c6c2b565a8a03458475` | `da835752a54b00ec3d7922ec1394958c4c1465c728c86f21f6474540242e1d7c` |
| Reopened Asset Plan | `e036dcb3587507db07e39a55cca5404dd074b4ede3abb70e487005e18ab478b7` findings | `a4e31d7f69c2ed88447eaf984c0bfdf6e85180a2ea80990234a51cb5141cc9ba` | `43d2297a39dd74dacd71a49ffee43febee1ba9e83d80bf0a55e1435eb9944fb8` |
| Compile control | `65dbbcff74b6886fdd749b1c7c0587eaff06b4f4720405e10716c2b0dfee9bbc` CSS | `92e17db715bef4de971f75da73c790b68c2cc1b017edae91b4df283d5d9b6277` | `4a4321f4142bfe0a06ebb419223fce693b82966ed2e72d4230cba71d52beaf3c` |
| Watch control | `65dbbcff74b6886fdd749b1c7c0587eaff06b4f4720405e10716c2b0dfee9bbc` CSS | `8fc31e3d221d70fe99931fcb8c1a2e414ed4a6e78145a667a179d7c3ada03543` | `3f43fa1238c25ed4f17942c4b6ef42d2c1610fe6ffdc80e4153397066c2335c1` |
| Direct DTCG | `9a349f5059ffb3ace6c985dfe1fadcd9cb9ac45668b5cd0c00e26625e5a6c6c3` CSS | `8758c9d6b69e42f5adc48ba7d84f4fb026618af8737fea690185bb53dd7f1994` | `aafe05a73d83a00aaa3be700bc554624389c60fb351e2f7a1c8d2cd6f0cae5aa` |

The direct DTCG vector freezes `examples/product.resolver.json` at
`5965f868707ad92b0164788b5a7b8b8a2234ce03d7e6c71e1df66d612801b876`, the complete graph at
`8659f9c443cb5c77c399fd741a27d476ec18887742c5459002fa6b82c22e864a`, and these selection hashes:

| Selection | `configHash` |
|---|---|
| `appearance=dark,channel=light` | `sha256:e6ce8e2578b2bbb68c0b533495dab2a781e5c7a32665d4c4a4845146f014ba3e` |
| `appearance=dark,channel=dark` | `sha256:5bab41c2b1a8d06d05fbaae1517d425a35b0bb8143fcdaeaee3c5faf48d9469e` |

## StyleId format-2 refresh audit

The shared pure compilation engine intentionally moved the portable output from the RC.2 identity
stream to StyleId format 2. Before updating the current goldens, a detached `v0.1.0-rc.2` worktree at
`064dcbce96a3a5cc97a940d07566c008bb5e2d3e` was built with Rust 1.85.0 and replayed with Node.js
22.13.0. It reproduced the legacy CSS hash
`acdb0bfee613ddb3fdae0feb9a8e5118dc5f74d385ed53eb610ff4eefd05d454`, manifest hash
`8c1dc690c2cbb8102b6935a970e60938bbda7e50a0c0bf766c5c800305cce598`, and every legacy control
hash byte for byte.

The current checkout then reproduced the new complete vector with Node.js 22.13.0 and 24.16.0.
CSS remains 424 bytes; source bytes, Resolver hash, graph cardinalities, canonical selections,
read-only checks, and rejection cases are unchanged. Only identities and artifacts derived from the
new identity stream were refreshed. The temporary audit worktree and Docker volume were removed
after verification.

Changing any frozen value still requires an intentional compatibility review. Update mode prints
witness values but does not bypass semantic assertions.

## Boundaries still open

- A new named release candidate must bind its tag, source commit, tree, portability run, browser
  matrix, registry replay, production replay, signed distribution, and final authorization. RC.2
  cannot inherit this branch's evidence.
- Normal compile/watch without `--control-dir` preserves author-supplied provenance and is not
  promised byte-identical across hosts.
- Filesystem locks are not certified for NFS/SMB or non-PliegoCSS writers; grouped publication is
  rollback-capable for handled failures, not durable crash atomicity.
- The WASM check is a library/target gate. Cloudflare Workers execution and application-level SSR
  remain separate release evidence.
- Native ARM64 binaries and signed distribution artifacts remain publication work even though the
  hosted macOS runner contributes compiler and browser evidence.
