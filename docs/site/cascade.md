<!-- pliegocss-site
{"schemaVersion":1,"route":"/docs/cascade/","category":"Standard CSS","eyebrow":"Cascade inspection","order":15}
-->

# Explain the winning declaration.

Inspect authored CSS by element and longhand to see matching selectors, precedence, and the final winner.

## Ask one precise question {#query}

Provide a CSS file, a bounded element description, and one longhand. The command reports candidates instead of simulating an entire browser.

```console
pliego-cssc explain-cascade --input app.css \
  --element 'button#save.action' --property background-color
```

## See why it won {#winner}

Origin, importance, layer, specificity, and source position are surfaced in comparison order with a stable machine-readable form.

## Keep the model honest {#boundary}

This is a static bounded inspector. Runtime DOM state, animation interpolation, adopted sheets, and script mutation remain browser responsibilities.
