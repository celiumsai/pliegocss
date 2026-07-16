# Usage observation sidecar schema 1

Status: **implemented explicit input contract; producer integrations remain application-owned**

An observation sidecar records positive bundle-qualified StyleId hits for one exact usage universe
and reachability snapshot. It is never auto-discovered and cannot prove deadness by absence.

```json
{
  "schemaVersion": 1,
  "universeSha256": "64-lowercase-hex-digits",
  "reachabilitySha256": "64-lowercase-hex-digits",
  "coverage": "sampled",
  "producer": {
    "name": "pliegors-browser-matrix",
    "version": "1.0.0"
  },
  "contexts": {
    "routes": ["/", "/account"],
    "islands": ["account-menu"],
    "themes": ["dark", "light"],
    "browsers": ["chromium-138"],
    "viewports": ["390x844", "1280x720"],
    "states": ["authenticated", "default", "focus-visible"]
  },
  "unknownDynamicInputs": ["third-party-widget-content"],
  "observedStyles": [
    {
      "bundleId": "application",
      "styleId": "32-lowercase-hex-digits"
    }
  ]
}
```

Use it explicitly:

```console
pliego-cssc bundle \
  --plan pliego.bundles.toml \
  --output-dir dist/assets \
  --manifest-version 5 \
  --reachability pliego.reachability.json \
  --usage-report \
  --observations pliego.observations.json
```

## Workflow

1. Build `--usage-report` without observations.
2. Retain `universeSha256` and `application.inputSha256` from `pliego.usage.json`.
3. Exercise declared routes, islands, themes, browsers, viewports, and states against the same
   immutable application snapshot.
4. Record only exact positive `(bundleId, StyleId)` hits and every known dynamic coverage gap.
5. Build again with `--observations FILE`; stale hashes, unknown styles, and contradictions fail.

The route, island, theme, browser, viewport, and state arrays are bounded producer-declared context
labels. PliegoCSS records and canonicalizes them but does not claim they are graph-owned identifiers
or prove that the producer exercised them. Graph consistency is enforced through the exact universe,
reachability digest, known positive StyleId hits, and unreachable-hit contradiction check.

The public `pliego-css-usage` adapter API can generate canonical bytes:

```rust,no_run
use pliego_css_usage::{
    UsageObservationCoverage, UsageObservationInput, UsageObservationScopeInput,
    UsageObservedStyleInput, build_usage_observation,
};

let input = UsageObservationInput::new(
    "0".repeat(64),
    "1".repeat(64),
    UsageObservationCoverage::Sampled,
    "pliegors-browser-matrix",
    "1.0.0",
    UsageObservationScopeInput::new(
        vec!["/".into()],
        vec![],
        vec!["light".into()],
        vec!["chromium-138".into()],
        vec!["1280x720".into()],
        vec!["default".into()],
    ),
    vec!["authenticated-route".into()],
    vec![UsageObservedStyleInput::new(
        "application",
        "11111111111111111111111111111111",
    )],
);
let json = build_usage_observation(input)?;
# Ok::<(), String>(())
```

## Coverage semantics

`coverage` is `sampled` or `producer-attested-complete`. In both cases, a listed style is observed
and an absent style is unobserved within this declared snapshot. Neither absence state proves dead
CSS. Only complete static reachability can derive `usageState: dead`.

`producer-attested-complete` is not independently certified by PliegoCSS. The tool validates schema,
hashes, identities, ordering, and graph consistency; it cannot know whether the producer exercised
every real route, auth state, feature flag, locale, viewport, theme, browser behavior, or dynamic
input. Unknowns must be listed rather than silently treated as covered.

The sidecar deliberately contains no wall-clock expiry decision. Review systems may integrity-bind
timestamps as external metadata, but identical inputs must not change verdicts when the local clock
changes.

## Validation and canonical form

- all fields are required and objects are closed;
- both hashes are exactly 64 lowercase hexadecimal characters;
- producer name/version and every opaque producer context label are nonempty, bounded, and contain
  no controls;
- bundle IDs use the Asset Plan identifier contract;
- StyleIds are exactly 32 lowercase hexadecimal characters;
- every observed style must exist in the bound universe;
- context arrays, unknown inputs, and observed styles are sorted canonically;
- duplicates fail rather than being silently removed;
- maximum document size is 16 MiB and maximum aggregate items is 65,535; and
- canonical builder output is two-space pretty JSON with one final LF.

An observation of a statically unreachable entry in the exact bound reachability snapshot is a hard
evidence conflict. PliegoCSS publishes no partial bundle group and asks the producer/adapter owner to
resolve the stale or incorrect evidence.
