# Generic CSS declaration usage

Status: **bounded positive-observation contract; absence remains unknown**

`pliego-cssc audit --format json` emits deterministic declaration identities in `PCSS-AUDIT-000`.
Each identity binds conditional/layer context, selector list, importance, and canonical declaration
bytes under `pliegocss-generic-css-declaration-v1`.

Build conservative usage evidence with:

```console
pliego-cssc generic-css-usage \
  --findings pliego.css.findings.json \
  --observed observed-identities.json \
  --scope chrome-local-session-1 \
  --output pliego.css.generic-usage.json \
  --control-dir dist/control
```

`observed-identities.json` is a JSON array of exact `sha256:` identities for which the named scope
has positive evidence. The report assigns only `observed` or `unknown`; missing evidence never means
`dead`. The optional control directory publishes the usage report, findings, manifest, and receipt as
one rollback-capable group bound to exact input/output hashes.

This contract does not infer DOM reachability, cascade winners, viewport execution, or dead CSS. It
provides a stable identity/evidence join for browser or host adapters that can supply positive
observations.
