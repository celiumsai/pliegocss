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
| Direct CSS audit | `a353b52f386f0b896db3c354f45bc0b304dffab1477904a5ee37cd8768be468a` findings | `b8513ecd80e9fe566778aa0bfb0b142ecfb06a014bb34a1caa02c1ad28567fd3` | `81777f9533bb6cd203f68e0f052e5974d09a2d773d7adf8f062f9361100a1c28` |
| Bundle control | `65dbbcff74b6886fdd749b1c7c0587eaff06b4f4720405e10716c2b0dfee9bbc` CSS | `e75b481bd2755a70d5c2ac1030729633cb96d28d8107e562b55ae0951913ba12` | `6fe898d54ad8c745d63d1e6cdd8d31ae6c157130ad4d047542ba6f3cb1f4b5cf` |
| Reopened Asset Plan | `bd9fa74558e57f08cc9bdff0bf6f6fa6502c483487451652795353a14dab579d` findings | `f254eebbfc716351308f56f34d6494e4317866965cbea6d4bf6b27922f0b09ea` | `d79e5cca007ee6af62cbafaa36532503860e6243f7d534398d27a1bde4e40dce` |
| Compile control | `65dbbcff74b6886fdd749b1c7c0587eaff06b4f4720405e10716c2b0dfee9bbc` CSS | `2ab00fa0f7b8a06e7bff3780fdc06d61b26b7136e163f8e6437907ddb013343e` | `3dfb3de854d95b39c7f69325f9eae71b37e62607f9c4e367e8188a63f848da70` |
| Watch control | `65dbbcff74b6886fdd749b1c7c0587eaff06b4f4720405e10716c2b0dfee9bbc` CSS | `537de8064740008b879872d77abfd767352c3a75474b72f79a7d95aab3982c85` | `602c062c391c030112356f996eb5bec251757cdf5ffb59b0f299b575dc869a49` |
| Direct DTCG | `9a349f5059ffb3ace6c985dfe1fadcd9cb9ac45668b5cd0c00e26625e5a6c6c3` CSS | `96a9fb6769b6ba1260e4581528c834d248a7dcb52e9a9be616ed1edfdb686c84` | `62968ba59c63c76ef01c73a8091978c30855ba8d39cf10f6717332786086546d` |

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
