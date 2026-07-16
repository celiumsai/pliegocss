# Property, parallel, and fuzz testing

Updated: 2026-07-13

PliegoCSS uses three complementary gates. They have different failure models and none substitutes
for the others.

## Fixed-seed property contracts

Run:

```console
pnpm check:properties
```

`crates/pliego-css-compiler/tests/property_contract.rs` uses an in-tree PRNG and no new dependency.
It covers 2,048 catalog-derived format/semantic/IR cases, 2,048 non-conflicting authoring-order
permutations, and 16 threads compiling 192 shared cases through one immutable `ThemeRegistry`.
Properties include formatter idempotence, identical StyleId/identity/CSS after normalization,
byte-exact IR encode/decode/encode, and equality with sequential results. The compiler package
already excludes `tests/**`, so this suite contributes zero bytes to its registry archive.

## Parallel process determinism

Run:

```console
pnpm check:determinism
```

The gate first creates two sequential schema-5 references: `modern`/minified and `none`/pretty. It
then runs five cohorts of eight simultaneous workers. Two independent-output cohorts per profile
must match their reference byte for byte; two same-destination cohorts accept only complete success
or the exact advisory-lock failure; and one cross-profile cohort requires the shared CSS/manifest
pair to equal one complete profile, never a torn combination. No temporary/backup file may remain.

A separate helper holds both destination locks while one compiler invocation must fail with the
documented lock diagnostic and create no output. This makes lock rejection deterministic even when
a same-destination cohort happens to serialize naturally. In total the gate executes 43 compiler
invocations: two references, 40 workers across five cohorts, and one rejected lock probe. Maximum
compiler concurrency is eight. Persistent empty `.pliego.lock` files are expected by the contract.

Set `PLIEGO_CSSC` and `PLIEGO_LOCK_HOLDER` to previously built executables to skip the default Rust
1.85 builds. Restricted Windows hosts can build both into an approved target directory and point
these variables at that directory. CI supplies the matrix-built binaries and repeats this gate on
Rust 1.85/1.96 for Windows, Linux, and macOS. Until those hosted jobs have actually run, the matrix
is configured coverage rather than hosted evidence.

To prepare an approved directory manually, build both binaries into the same target and then point
the two variables at its `debug/` executables:

```console
cargo +1.85.0 build --locked -p pliego-cssc --target-dir <approved-target>
cargo +1.85.0 build --locked --manifest-path integration-tests/publication-lock-holder/Cargo.toml --target-dir <approved-target>
```

## Coverage-guided fuzzing

The independent `fuzz/` workspace pins its complete dependency graph in `fuzz/Cargo.lock`. It has
two targets:

- `parser_pipeline` drives UTF-8 parsing, formatting, semantic lowering, identity, binary IR, and
  CSS emission with differential/idempotence assertions;
- `binary_artifacts` mixes raw bytes with mutations of a valid artifact, optionally repairs the
  payload digest to reach deeper decoder validation, and requires every accepted artifact to
  re-encode byte exactly.

Tracked seeds live under `fuzz/corpus/`; `fuzz/pliegocss.dict` supplies grammar fragments and the
real `PLGCIR\0\0` semantic-IR marker. The bounded-small change gate uses cargo-fuzz 0.13.2,
`nightly-2026-06-26`, AddressSanitizer, a fixed seed, 1,024-byte maximum inputs, and 20,000
executions per target. Defensive large-artifact limits remain covered by deterministic decoder
tests; this smoke does not claim to explore the 16 MiB boundary.

```console
pnpm check:fuzz
```

On 2026-07-13 the current exact harness completed all 40,000 bounded executions under Debian WSL2.
This is local execution evidence, not a substitute for the configured hosted job, native hosted-OS
coverage, or weekly soak history.

Install the exact tools with:

```console
rustup toolchain install nightly-2026-06-26 --profile minimal --component rust-src
cargo install cargo-fuzz --version 0.13.2 --locked
```

On Windows, install Visual Studio C++ AddressSanitizer and run from a Developer PowerShell, or add
the MSVC `Hostx64/x64` directory containing `clang_rt.asan_dynamic-x86_64.dll` to `PATH`. Restricted
hosts may set `CARGO_FUZZ_BIN` to an installed executable and `PLIEGO_FUZZ_TARGET_DIR` to an approved
build directory. The harness copies tracked corpus inputs into ignored `target/fuzz-runs`, never
mutates the tracked corpus, and retains that run directory when a target fails.

The weekly workflow runs the command equivalent of:

```console
pnpm soak:fuzz
```

for five minutes per target and uploads retained reproductions on failure. A configured schedule is
not historical evidence: release review must inspect actual completed runs and unresolved artifacts.
