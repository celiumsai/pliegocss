<!-- pliegocss-site
{"schemaVersion":1,"route":"/docs/tooling/cli/","category":"Tooling","eyebrow":"Command-line reference","order":22}
-->

# One CLI, explicit modes.

Compile, inspect, audit, transform, explain, bundle, watch, migrate, format, plan, and verify with strict argument contracts.

## Discover the real command surface {#discover}

Root and command help are generated from the maintained command synopsis. Unknown options, duplicate single-value options, and missing values fail.

```console
pliego-cssc --help
pliego-cssc compile --help
```

## Select diagnostic transport globally {#diagnostics}

Human and JSON diagnostics can be requested before or after the command without stealing option values that happen to resemble flags.

```console
pliego-cssc --diagnostic-format json check --style "p-4 p-6" --seed
```

## Use --check in CI {#determinism}

Commands that publish controlled or generated artifacts expose a read-only comparison path. CI should verify the checked-in or staged output, not regenerate it silently.
