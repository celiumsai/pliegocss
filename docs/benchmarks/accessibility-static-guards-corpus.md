# Accessibility static-guard diagnostic-quality corpus

Status: **8-case synthetic static slice passing; claim remains bounded**

The machine corpus is
[`accessibility-static-guards-corpus.json`](./accessibility-static-guards-corpus.json).
It exercises the production accessibility projection for motion, focus visibility, forced colors,
and input modality. Every case freezes one expected finding code and check identity.

| Check | Cases | Frozen outcomes |
|---|---:|---|
| Motion | 2 | guarded pass; unguarded violation |
| Focus visibility | 2 | explicit suppression violation; runtime/manual-required |
| Forced colors | 2 | `none` violation; `auto` pass |
| Input modality | 2 | two exact hover/focus equivalence passes |

This is a synthetic implementation-quality corpus. It does not measure real-world prevalence,
browser rendering, manual interaction quality, broad precision/recall, or WCAG conformance.

Reproduce with:

```console
cargo test -p pliego-css-control --features projection --test accessibility_quality --locked
```
