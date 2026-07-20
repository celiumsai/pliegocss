# PliegoCSS 0.1.0 release readiness

Status: **`0.1.0-rc.1` locally verified; final promotion blocked by external release evidence**

The machine authority is [`release-readiness-0.1.0.json`](./release-readiness-0.1.0.json).

Local fast, integration segments, public API/MSRV, determinism, portability, package replay, and
Chrome/Edge computed-style gates pass. Nineteen RC-shaped crates package and compile together without
publication.

Final `0.1.0` promotion is intentionally blocked until:

1. the consolidated RC commit is pushed to the private `https://github.com/celiumsai/pliegocss` repository;
2. committed hosted Windows/Linux/macOS CI produces the required browser/OS evidence;
3. historical benchmark commit `c47239c` is restored or its snapshots are explicitly superseded from
   a clean reviewed commit; and
4. commit/push/RC publication is authorized, followed by registry installation replay.

None of these states is represented as passing. The workspace remains `0.1.0-rc.1` until the machine
readiness document is green or a reviewed release ADR explicitly changes a requirement.
