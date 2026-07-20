# pliego-css-io

Minimal bounded regular-file reads shared by PliegoCSS tooling. The crate rejects link-like path
components, non-regular files, and inputs larger than the caller-provided limit.