# pliego-css-cascade

Fail-closed standard-CSS cascade explanation for PliegoCSS.

The crate owns the bounded schema-1 static analyzer used by `pliego-cssc explain-cascade`. It does
not implement browser layout, computed style, or a styling runtime. See the
[cascade explanation reference](https://github.com/celiumsai/pliegocss/blob/main/docs/reference/cascade-explain-command.md)
for the proof boundary and non-claims.
