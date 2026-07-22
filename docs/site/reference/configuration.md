<!-- pliegocss-site
{"schemaVersion":1,"route":"/docs/reference/configuration/","category":"Reference","eyebrow":"Configuration","order":28}
-->

# Configuration is part of identity.

Theme, target, format, manifests, reachability, pruning, and token selections are explicit, validated, and hash-bound.

## Conventional TOML discovery only {#discovery}

Without --config, --tokens, or --seed, the CLI searches for pliego.theme.toml from the workspace and input package roots. Ambiguous candidates fail.

## Selectors are mutually exclusive {#exclusive}

--config, --tokens, and --seed cannot be combined. JSON token documents are never discovered implicitly.

## Bind the complete choice {#hash}

Targets, format, emission settings, manifest version, pruning, theme bytes, resolver selections, and reachability bytes participate in the control configuration hash.
