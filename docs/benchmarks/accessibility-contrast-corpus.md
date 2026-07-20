# Accessibility contrast diagnostic-quality corpus

Status: **8-case synthetic literal-sRGB slice passing; claim remains bounded**

The machine corpus is [`accessibility-contrast-corpus.json`](./accessibility-contrast-corpus.json).
It contains four expected violations and four expected passes for declared opaque, in-gamut literal
sRGB pairs at explicit 3:1, 4.5:1, or 7:1 thresholds.

The scored test calls the production accessibility projection, classifies `PCSS-A11Y-101` as the
positive prediction, and computes:

| Metric | Result |
|---|---:|
| TP | 4 |
| TN | 4 |
| FP | 0 |
| FN | 0 |
| Precision | 1.000 |
| Recall | 1.000 |

Four additional runtime-dependent cases (`var()`, `currentColor`, system colors, and alpha) are
expected abstentions. All four emit `PCSS-A11Y-108` and emit neither pass nor violation. Across the
combined 12-case corpus, decision coverage is 0.666 and abstention rate is 0.333 (integer-milli
reporting); precision/recall remain scoped only to the eight decidable cases.

This is an implementation-quality signal for one closed synthetic slice. It is not broad
accessibility accuracy, DOM/cascade inference, runtime validation, real-incident evidence, or WCAG
certification.

Reproduce with:

```console
cargo test -p pliego-css-control --features projection --test accessibility_quality --locked
```
