# Plain HTML contract fixture

Status: **implemented local reference contract; not a published framework adapter**

PliegoCSS output is standard external CSS and standard class attributes. A page does not need
PliegoRS, JavaScript, WebAssembly, or a styling runtime to consume it. The repository proves that
boundary with `integration-tests/plain-html-smoke`:

```console
pnpm integration:plain-html
```

The gate compiles two explicit utility lists, reads their classes from manifest schema 3, renders a
plain HTML template, serves only the resulting HTML and CSS on an ephemeral loopback port, and checks
the page in local headless Chrome through CDP.

## Exact input contract

The fixture does not scan HTML for class-like text. Its three inputs are explicit:

- `styles.txt` contains one complete utility list per LF-terminated line;
- `adapter.json` maps a stable slot ID to a one-based style line and one exact template marker;
- `index.template.html` contains each declared marker exactly once.

The fixture-local adapter schema is closed and rejects unknown fields, unsafe paths, duplicate IDs,
duplicate lines, duplicate markers, missing lines, and stale or repeated template markers. For each
slot it requires exactly one manifest origin with:

```text
macroKind = input
reason = line-oriented-input
source + byteStart + byteEnd = exact styles.txt line
```

Only after that check does it replace `{{PLIEGOCSS_CLASS:<slot>}}` with the compiler-produced
`pc_*` class. This is an explicit build contract, not probabilistic extraction and not an assertion
that arbitrary JavaScript strings are statically reachable.

## Browser evidence

The current gate requires all of the following:

- CSS bytes and SHA-256 equal the adjacent schema-3 manifest;
- both classes in rendered HTML equal their exact manifest classes;
- computed `grid`, `inline-flex`, alignment, gap, padding, radius, and shadow values match the
  authored utilities;
- the stylesheet is loaded once with `link` as its resource initiator;
- the document contains zero `script` elements and loads zero `.wasm` resources;
- browser exceptions, warnings, network failures, and local-server errors are empty.

The zero-runtime result applies to this complete plain-HTML fixture. It does not replace the separate
measurement of a complete PliegoRS application's WASM payload and does not imply that every host
application is script-free.

## Direct use outside Rust

A generic repository can use the same line-oriented compiler input without Rust source scanning:

```console
pliego-cssc compile \
  --input styles.txt \
  --seed \
  --targets modern \
  --output app.css \
  --manifest app.manifest.json \
  --manifest-version 3
```

The host build must obtain class names from the numbered manifest and bind them through an explicit
mapping like the fixture. It must not copy the current hash algorithm, guess classes from utility
text, or treat text search as complete application reachability.

## Boundary and next adapter

This closes the plain-HTML consumption fixture required by A9 and gives non-Rust repositories a
deterministic no-runtime path. It does not yet provide:

- a published reusable HTML/Vite/Astro package;
- typed discovery of dynamic JavaScript or template branches;
- route/island reachability, Asset Plan, or Project Index generation from a non-Rust framework;
- hosted Chrome/Firefox/WebKit evidence.

The next non-Pliego adapter must emit exact typed source/topology facts from its framework compiler
and prove that identical semantic input produces the same StyleId, class, CSS, and supported
artifacts as the CLI/Rust frontend. See the [CLI reference and manifest schema 3](../reference/cli.md#manifest-schema-3) and
[Project Index contract](../reference/project-index-schema.md).
