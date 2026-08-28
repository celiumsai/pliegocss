<!-- pliegocss-site
{"schemaVersion":1,"route":"/docs/composition/","category":"Core","eyebrow":"Composition","order":11}
-->

# Compose by meaning, not source order.

Compatible slots commute. Conflicting slots require an explicit branch-over-base operation.

## Equivalent input, equivalent identity {#commute}

Utilities that occupy independent semantic slots canonicalize to the same identity even when authored in a different order.

```text
flex gap-4 p-6
p-6 flex gap-4
```

## Ambiguous lists are rejected {#conflict}

p-4 p-6 does not mean last one wins. Both declarations claim the same slot under the same condition, so compilation stops.

```console
pliego-cssc check --style "p-4 p-6" --seed
```

## Overrides are explicit operations {#override}

When an application intentionally replaces one semantic assignment with another, use composition so the override relationship remains visible and inspectable.

```console
pliego-cssc compile --compose "p-4" "p-6" --seed
```
