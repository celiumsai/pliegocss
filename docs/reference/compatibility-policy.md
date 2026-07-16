# Compatibility policy schema 2

Status: **implemented and machine-enforced for the current alpha catalog**

PliegoCSS separates semantic validity from browser compatibility. The compiler first validates and
normalizes typed intent, then applies one explicit compatibility profile at the Lightning CSS
boundary. A profile is a versioned policy, not a floating Browserslist query.

Inspect the complete policy without compiling styles:

```console
pliego-cssc compatibility --targets baseline-widely
```

The command writes deterministic JSON with one final newline. Its exact `baseline-widely` bytes are
frozen by `pnpm check:compatibility`.

## Profiles

| Profile | Guarantee | Browser targets | Unclassified CSS |
|---|---|---|---|
| `baseline-widely` | Frozen WebDX Baseline Widely Available snapshot | Chrome/Edge 120, Firefox 121, Safari/iOS Safari 17.2 | Rejected with `CMP001`–`CMP003` |
| `modern` | Historical fixed PliegoCSS candidate vector | Chrome/Edge 111, Firefox 128, Safari 16.4 | Allowed as legacy unclassified input |
| `none` | No browser compatibility guarantee | No Lightning CSS browser targets | Allowed and explicitly unmanaged |

`modern` remains the compile default to avoid silently changing existing candidate CSS. New
applications that want a closed compatibility claim should select `baseline-widely` explicitly.
`none` is the escape hatch when a downstream pipeline owns compatibility.

## Frozen Baseline snapshot

WebDX defines **Widely Available** as support across its core browser set for at least 30 months.
PliegoCSS policy version 7 retains the browser mapping first frozen by policy version 1 on
2026-07-14 rather than resolving that moving definition on every build. The normative external
references are the WebDX
[Baseline definition](https://web-platform-dx.github.io/web-features/) and
[supported browser mapping](https://web-platform-dx.github.io/supported-browsers/).

Schema 2 replaces the earlier provenance-by-URL-only shape with exact offline data identity:

- `web-features@3.32.0`, npm SRI, immutable tarball URL, `data.json`, and its SHA-256;
- `baseline-browser-mapping@2.10.43`, npm SRI and immutable tarball URL;
- query `widelyAvailableOnDate=2026-07-14;includeDownstreamBrowsers=false`;
- SHA-256 of the canonical mapping result.

Updating the snapshot is an explicit policy change. It requires a reviewed policy version, updated
golden JSON, transform fixtures, documentation, and multi-browser evidence. Host environment,
Browserslist files, network state, and current date never change compilation implicitly.

## Schema

Schema 2 is a closed, camel-case JSON document:

| Field | Meaning |
|---|---|
| `schemaVersion` | Document grammar; currently `2`. |
| `policyVersion` | Semantic decision set; currently `7`. |
| `profile` | `baseline-widely`, `modern`, or `none`. |
| `guarantee` | Compatibility claim made by the profile. |
| `compatibilityData` | Capture date, exact packages/integrity, fixed query, and result/content hashes. |
| `baseline` | Baseline status, optional snapshot date, and normative source. |
| `browserTargets[]` | Ordered minimum browser versions passed to Lightning CSS. |
| `reset` | Explicit reset profile, action, implicit flag, and owner. |
| `scope` | Scope profile, action, and ownership boundary. |
| `features[]` | Ordered feature decisions with `id`, `tier`, `action`, and enforcement. |

Capability tiers are `core`, `extended`, `escape-hatch`, and `experimental`. Actions are `allow`,
`transform`, `warn`, and `error`:

- `allow`: compiler and profile accept the feature without compatibility lowering;
- `transform`: Lightning CSS lowers or prefixes according to the exact target vector;
- `warn`: output is not injected and the host must make an explicit choice;
- `error`: the profile rejects the feature before output publication.

Unknown schema versions, policy versions, profile names, tiers, or actions must be rejected by
strict consumers. An incompatible wire change requires a new schema. A decision or browser-vector
change requires a new policy version even when the JSON shape is unchanged.

## `baseline-widely` decisions

| Feature | Tier | Action | Enforcement |
|---|---|---|---|
| Typed utility catalog | Core | Allow | Compiler-validated semantic IR |
| Typed custom-property references | Core | Allow | Typed reference and domain hint |
| Typed ARIA/data variants | Extended | Allow | Compiler-validated native selector |
| Configurable attribute variants | Extended | Allow | Compiler-bounded attribute selector |
| `ltr`/`rtl` direction variants | Core | Allow | Compiler-validated `:dir()` selector |
| Typed writing-mode utilities | Core | Allow | Compiler-validated native property |
| Typed container queries | Extended | Allow | Compiler-validated native condition |
| Vendor prefixes | Core | Transform | Lightning target vector |
| Selector syntax | Extended | Transform | Lightning target vector |
| Media-query syntax | Core | Transform | Lightning target vector |
| Color syntax | Extended | Transform | Lightning target vector |
| Logical properties | Extended | Transform | Lightning target vector |
| Arbitrary values | Escape hatch | Error | `CMP001` |
| Arbitrary properties | Escape hatch | Error | `CMP002` |
| Arbitrary selectors | Escape hatch | Error | `CMP003` |
| `light-dark()` | Experimental | Error | Not classified in typed IR |
| Custom media | Experimental | Error | Draft parser disabled |
| Browser reset | Core | Warn | No implicit output; host-owned import |
| Typed cascade layers | Extended | Allow | Compiler-fixed order and native CSS |
| Standard class scope | Core | Allow | Native external stylesheet class |
| Component scope | Experimental | Error | Not implemented yet |

Policy version 2 added the original typed selector rows. Policy version 3 added typed container
establishment/query conditions and their native CSS output. Policy version 4 adds bounded,
configurable ARIA/data selector variants. None changed the frozen browser vector or JSON schema.
Policy version 5 adds the three typed native writing-mode utilities and their conflict-checked IR
slot, again without changing the browser vector or schema.

Policy version 6 added four compiler-owned typed cascade layers, their fixed order statement, and
physical provenance support, again without changing the browser vector or schema.

Policy version 7 and schema 2 bind the exact official dataset artifacts and mapping query/result.
They also provide the evidence source for standards-first audit findings. The browser vector and
existing typed-feature actions do not change.

The policy does not claim support merely because Lightning CSS can parse a syntax. A feature must be
classified in semantic IR or governed as an explicit escape hatch before the strict profile accepts
it.

## Fail-closed diagnostics

Under `baseline-widely`, unclassified CSS fails before CSS or manifest publication:

```text
CMP001  arbitrary value
CMP002  arbitrary property
CMP003  arbitrary selector
```

Human diagnostics retain the deterministic source label or exact byte range. JSON diagnostics keep
the same CMP code and use category `compatibility`. A failed compile keeps the last valid output
group unchanged. Use a typed utility/token, or select `--targets none` to state explicitly that
compatibility is managed elsewhere.

## Reset and scope boundary

Policy version 1 has reset profile `none`. PliegoCSS never injects Preflight, normalization, or a
browser reset as a transitive side effect. Applications may import a reset explicitly and own its
ordering, bytes, and compatibility.

The implemented scope profile is `standard-class`: generated `pc_*` classes work in any native
external stylesheet consumer. Typed `layer-*` variants explicitly opt assignments into the fixed
`pliego.*` cascade graph; unprefixed styles receive no implicit layer. Framework components, Shadow
DOM, CSS Modules, and `@scope` receive no implicit semantics. Component scope must first become a
typed IR dimension under A3/A5, then move from `error` to a reviewed policy decision. ADR-0014
defers that work beyond `0.1.0`: the frozen browser vector predates native `@scope`, and selector
prefixing is not an equivalent transformation.

## Verification

```console
pnpm check:compatibility
cargo +1.85 test --locked -p pliego-css-build --features artifacts -p pliego-cssc
```

The gate verifies exact policy bytes, deterministic reproduction, all three profiles, typed strict
selector/container compilation, manifest labeling, structured CMP codes, and last-valid-artifact
preservation.

See [ADR-0012](../adr/0012-freeze-baseline-compatibility-policy.md),
[ADR-0014](../adr/0014-defer-native-component-scope.md), the
[CLI reference](./cli.md), and the broader [candidate compatibility contract](./compatibility.md).
