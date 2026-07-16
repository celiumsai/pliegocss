# Portability contract

Updated: 2026-07-15

Status: **executable contract implemented; hosted matrix evidence and Cloudflare remain open**

## Configured matrix

The Rust CI matrix targets:

- Ubuntu, Windows, and macOS;
- Rust 1.85.0 (MSRV) and 1.96.0 (current pinned toolchain);
- workspace tests and Clippy with warnings denied;
- Node.js 22.13.0 executing the same portability contract on every host. This matches the minimum
  engine required by the pinned pnpm 11.7.0 toolchain.

The WASM job also checks `pliego-css` with Rust 1.85.0 for
`wasm32-unknown-unknown` before running the existing Rust 1.96 zero-overhead micro-fixture. This is
not a claim of WASI, `no_std`, every WASM target, or a deployed Cloudflare Workers build.

The repository currently has no configured Git remote, so this document distinguishes the checked-in
matrix from hosted green-run evidence. The current canonical-token-graph vector has passed locally on
Windows x64 with the checkout's default Rust toolchain and Node.js 24.16.0, and on Debian WSL2 Linux
x64 with the same graph-bearing hashes under Node.js 20.19.2. The Linux Node version is extra local
evidence and does not lower the supported Node.js 22.13.0 floor. The earlier Rust 1.85,
Node.js 22.13.0, and MSRV WASM observations predate `pliego.tokens.json`; they remain superseded for
this vector until rerun.

## Frozen executable vector

`node scripts/check-portability.mjs` creates an isolated bundle-plan project and verifies:

- identical CSS and schema-3 manifest bytes when invoked with the same absolute plan/output arguments
  from two unrelated process directories;
- plan, source, output, and process-directory paths nested below a directory containing a space;
- a UTF-8 path with an accented character and astral content before the scanned macro;
- CRLF Rust source input whose manifest range selects the exact macro bytes, with `/` manifest paths;
- manifest byte count and SHA-256 binding to the adjacent CSS;
- a successful `--check` leaves existing artifact bytes, size, modification time, inode identity, and
  the output tree unchanged, while a missing-output check fails with an empty final directory and no
  persistent locks;
- a drift failure is read-only;
- source and theme paths cannot escape through an intermediate Unix symlink or Windows junction;
- a linked output directory fails before publication;
- explicit `.` path segments fail closed;
- ASCII case-insensitive config/output aliases fail before any mutation on every host, so Linux
  validation models the collision boundary of a later Windows replay.
- direct-CSS audit control artifacts have frozen byte hashes, and `--check` verifies the complete
  findings/manifest/receipt group without changing it.
- a manifest-5 `bundle --control` vector binds exact reachability, CSS/source-map/style-manifest,
  Asset Plan, Project Index, canonical TokenGraph, findings, control manifest, and receipt bytes; its
  `--check` is also read-only and rejects token-graph drift without repairing it.
- a separate `audit --asset-plan` invocation reopens the on-disk plan and adjacent CSS/manifest pair,
  proves canonical regeneration, freezes its own control group, and verifies read-only `--check`.
- exact-snapshot compile and watch each freeze CSS, Source Map v3, style manifest, canonical
  TokenGraph, findings, control manifest, and receipt: seven artifacts per vector. Compile `--check`
  proves the finite group is read-only and rejects token-graph drift without repairing it. Each vector
  verifies compact canonical fields, non-empty mappings, logical Unicode source identity, and exact
  manifest byte/hash cross-references while proving CSS has no injected discovery comment.
- every generated TokenGraph parses as compact canonical JSON, declares
  `pliegocss-token-graph/1`, matches `tokens.graphHash`, and is reciprocally related to the generated
  CSS and style manifest in the control manifest.

The separate `pnpm check:manifest` fixture uses only plan-relative logical paths and freezes
schema-4 graph bytes across the same host-independent source/ThemeId boundary. The established
portability vector remains default schema 3, so opt-in graph metadata cannot silently change its
bundle bytes.

`pnpm check:trace` adds a separate host-independent schema-5 golden with exact UTF-8 CSS ranges and
proves that schema 5 projects to the frozen schema-4 graph without changing the CSS artifact.

The vector freezes these bytes for both supported Rust toolchains:

| Artifact | Contract |
|---|---:|
| CSS bytes | 424 |
| CSS SHA-256 | `d208d2960bd28fb29354b1a89c74c106686c8a8a2d71a9cc7cc9971eb2e97070` |
| Manifest SHA-256 | `3a89e04856d5ff8244c29e91325271d6a57e33eec99242b121308c477facd169` |
| Provenance path | `src/café.rs` |

The direct-CSS audit vector additionally freezes:

| Control artifact | SHA-256 |
|---|---|
| `pliego.css.findings.json` | `75015787cd20bd52e4c1d504943b675785f55d7376a583a103cad4ab5c7de464` |
| `pliego.css.manifest.json` | `bac4099153b23cfd990fc2e70506153d08b725ca27661ad102934f45df721ac1` |
| `pliego.css.receipt.json` | `cb680d32a41321b089cf2fd98b7f60501fa8ba306f408d5f448a11551598b9b1` |

The manifest-5 bundle-build vector freezes the complete nine-artifact group:

| Bundle artifact | SHA-256 |
|---|---|
| `app.css` | `d208d2960bd28fb29354b1a89c74c106686c8a8a2d71a9cc7cc9971eb2e97070` |
| `app.css.map` | `39a03b016583b0827ec8bc92a2a1677dcb4d151324dcdd9c77d303af29b1dddf` |
| `app.manifest.json` | `0b04b7cf34339e598aeb256c5667a808461dfd02e90ce18aaec0a60de501eb9a` |
| `pliego.assets.json` | `b004fd1b8c28a13b1af0f80c105fc71a7615bab45ea3a1619f7f65084d1b343e` |
| `pliego.css.findings.json` | `e03d275890f0db3ccceaddfb6dc5b43acd0f0f731b99c77bd0b9bd9b6bf61d40` |
| `pliego.css.manifest.json` | `19bbb01d407c958d0938ae536c4c08889d1084fd86f6ee246bae0d0326a98db3` |
| `pliego.css.receipt.json` | `2bac654c6060a50f4a1fa21ea22baa1ef5d87c7f741aa37c41a5068dba1bf52d` |
| `pliego.index.json` | `6d60bcce6f91e4dc71ca98d921e9ce8bb267cb4d027d51d664b4e87477313dec` |
| `pliego.tokens.json` | `bb525a903714688d1ada713bff7a0c831631ee1c08b10434d1d03c79d742d204` |

The independently reopened Asset Plan audit vector freezes:

| Asset Plan audit artifact | SHA-256 |
|---|---|
| `pliego.css.findings.json` | `67e1d24a75adc042312286ade5572379ec706df1ad177031b9160862d27c5e59` |
| `pliego.css.manifest.json` | `004d39320c6aab32a6293896db98bbef21a040ec457afc2114e82fc67e7ab795` |
| `pliego.css.receipt.json` | `d392124440e0295760fb5e8629fb5e4d07271c1f5fcffc03b628bdfbdbab65ce` |

The exact-snapshot direct compile vector freezes:

| Compile artifact | SHA-256 |
|---|---|
| `app.css` | `d208d2960bd28fb29354b1a89c74c106686c8a8a2d71a9cc7cc9971eb2e97070` |
| `app.css.map` | `39a03b016583b0827ec8bc92a2a1677dcb4d151324dcdd9c77d303af29b1dddf` |
| `app.manifest.json` | `3a89e04856d5ff8244c29e91325271d6a57e33eec99242b121308c477facd169` |
| `pliego.css.findings.json` | `b59823b27e3f045625bcc9a8ecc3842de039ae1d70d2c98348bd0444550abe04` |
| `pliego.css.manifest.json` | `51bd81698f58ea3c4ab66c6f5c111135523b62e0177f970a6ae40229ac96d247` |
| `pliego.css.receipt.json` | `f94939da3e2c04c8d373b2fe881e9234b5abcee8bbb81b6b0d7e1a9c3fdb4fce` |
| `pliego.tokens.json` | `bb525a903714688d1ada713bff7a0c831631ee1c08b10434d1d03c79d742d204` |

The exact-snapshot watch vector freezes:

| Watch artifact | SHA-256 |
|---|---|
| `app.css` | `d208d2960bd28fb29354b1a89c74c106686c8a8a2d71a9cc7cc9971eb2e97070` |
| `app.css.map` | `39a03b016583b0827ec8bc92a2a1677dcb4d151324dcdd9c77d303af29b1dddf` |
| `app.manifest.json` | `3a89e04856d5ff8244c29e91325271d6a57e33eec99242b121308c477facd169` |
| `pliego.css.findings.json` | `3df2586a1041d39a0e3e66db4e6674df39479157b3ac413c0fea8b7f85ad23bd` |
| `pliego.css.manifest.json` | `b0713b0a3957f267e960ec026088441f4a4e3d476e52becd30c1cfe08af0aa64` |
| `pliego.css.receipt.json` | `fe5633bfa90a7edb953e390e5185597fd909d5f9b696059f36dd1e2c0ed6d2e9` |
| `pliego.tokens.json` | `bb525a903714688d1ada713bff7a0c831631ee1c08b10434d1d03c79d742d204` |

The direct DTCG Resolver vector selects `appearance=dark,channel=light`, freezes the exact
`examples/product.resolver.json` input at
`5965f868707ad92b0164788b5a7b8b8a2234ce03d7e6c71e1df66d612801b876`, and freezes:

| Direct DTCG artifact | SHA-256 |
|---|---|
| `app.css` | `cec35891bc4d167bb359b57cf5fcda10a06e2b0665398424ff63b8e01c8b4850` |
| `app.css.map` | `ce30a183a990c8dca861721b4c5ea953884fd4fb4898220023192e20abc62c3c` |
| `app.manifest.json` | `88597038d0aab327fc11d11a4371b03e1aa53b44199cc137eb84729f3eddd00a` |
| `pliego.css.findings.json` | `2135fdb346d08d7e25516006a9d7829780d155b8e1e2374f19ddd7a08a7916c9` |
| `pliego.css.manifest.json` | `9fb936c7c415af5c78a3edd298e31e6a4d750a84374e830961b2472117ac3e97` |
| `pliego.css.receipt.json` | `b24d23b286df8c35711684c585962a2b25d84bf1c6de84b7c618c08101738183` |
| `pliego.tokens.json` | `7a0862dcd2cd8152cface3ea82fbccf1877e75c6f505efa8a3032f8652f354f0` |

The vector also freezes `configHash` as
`sha256:e06966ae5f001ba6b1b29bfa4d52a3c3d26f9c4ba588d8ff558bd820c23767a9`.
Changing only `channel=dark` retains the same `ThemeId`, CSS, style manifest, and complete graph but
changes `configHash` to
`sha256:d0ab9d7cdc46d564adc14142be10d15b14dee36066966a29f4857e22a0f3616c`.
This proves that canonical selections remain identity-bound independently of the compiled registry.

The generated bundle/compile/watch manifest hashes include each exact Source Map v3 reference, the
canonical `pliegocss-token-graph/1` artifact and identity, reciprocal graph relationships, and
distinct referenced-token coverage. Direct CSS and reopened Asset Plan audit remain unchanged because
they do not own generation and their token state is explicitly unavailable.

On 2026-07-14 the complete updated script passed all six exact control groups on local Windows x64
using Node.js 24.16.0 and again on Debian WSL2 Linux x64 using Node.js 20.19.2 with its runtime on the
native Linux filesystem. The Linux replay used the graph-bearing bundle/compile/watch and direct
DTCG vectors shown above, including canonical graph parsing, exact Resolver-ledger identity,
case/order convergence, reciprocal relationships, exact receipt evidence, drift rejection, and
read-only checks. All frozen direct DTCG hashes matched Windows byte for byte. No
hosted-runner, macOS, Rust 1.85 graph-bearing, supported-floor Node.js 22.13, or Cloudflare
portability claim is made.

Bundle-plan schema 2 also passed its three DTCG E2E cases on local Windows x64 and on Debian WSL2
Linux x64 with a native Linux target directory. Those cases validate schema-1 compatibility,
default/dark contexts, exact `token-resolver` evidence, the complete graph, same-ThemeId selection
identity, read-only `--check`, and preservation of the last valid group. This is cross-OS execution
evidence, not a new frozen byte vector: the temporary plan path and exact plan bytes intentionally
participate in bundle `configHash`.

On 2026-07-15 the Cargo build-macro DTCG bridge passed the complete Debian WSL2 Linux x64 workspace
with a native Linux target directory, Clippy with warnings denied, the locked Rust 1.85 workspace
check, and the Rust 1.85 downstream public-API identity smoke. That smoke froze
`StyleId=1b052ee4ca1192db2e1ef91594166f70`,
`ThemeId=aeba2dade1fb8ef54e6db50494167fbd`, 722 CSS bytes, and CSS SHA-256
`25a0aa84e0ba5256c21ecd5740d9e6094632aeb2fc5600992f9d4f16013efd2d`; it also proved TOML registry
convergence and direct DTCG macro/CLI identity. Linux executed an actual file-symlink rejection.

The post-change Windows x64 dirty package gate compiled every archive and ran separate extracted
Rust 1.85 DTCG and legacy-TOML build-script consumers with byte-identical selected-registry output.
The Windows file-symlink test is conditional: this checkout could not create the link because the OS
returned error 1314, so this milestone does not claim an observed Windows reparse-point rejection.
The code still fails closed on reparse metadata and opens with `FILE_FLAG_OPEN_REPARSE_POINT`, but
that is an implemented contract rather than executed evidence here. Native macOS, native ARM64,
and hosted runners remain open. Commit `9714b09` passed the complete clean Debian WSL2 package gate:
all eleven archives compiled, and the extracted Rust 1.85 DTCG and legacy-TOML consumers produced
byte-identical output.

Changing one of these values requires an intentional compatibility review, not a silent fixture
update.

## Line-ending contract

`.gitattributes` fixes LF for source, configuration, documentation, workflow, fixture, CSS, HTML, and
diagnostic snapshot files. Binary media and WASM remain binary. The explicit CRLF source inside the
portability vector proves that scanner offsets still describe the exact supplied bytes; it does not
make checked-in line endings host-dependent.

## Boundaries still open

- Normal compile/watch without `--control-dir` preserves author-supplied provenance and is not
  promised byte-identical across hosts. Controlled compile/watch and bundle-plan provenance use
  portable logical paths and are the cross-OS contracts.
- The hosted matrix must run green after a remote repository exists.
- The canonical-token-graph vector still needs macOS, Rust 1.85.0, and supported-floor Node.js
  22.13.0 evidence; local Linux x64 is now covered but is not hosted evidence.
- Hosted runner labels and action major tags are intentionally not release-pinned yet; supply-chain
  SHA pinning and recording the runner image belong to the release gate.
- PliegoRS integration still depends on a dirty sibling checkout and is not a clean-clone CI job.
- Filesystem locks are advisory and are not certified for NFS/SMB or non-PliegoCSS writers.
- Grouped publication is rollback-capable for handled failures, not gap-free, crash-atomic, or durable
  against power loss. A stale compiler process can still publish after a newer compilation.
- Browser behavior still lacks a Chromium/Firefox/WebKit matrix.
- Cloudflare Workers deployment and an application-level WASM/SSR gate remain separate work.
