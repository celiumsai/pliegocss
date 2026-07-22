# PliegoCSS repository CLI package

This is an npm-format package, but it is **not published to npmjs**. A release
build embeds the supported `pliego-cssc` and `pliego-css-lsp` native binaries
and publishes the resulting `.tgz` only as an immutable GitHub Release asset.

The package has no dependencies and no lifecycle scripts. Its small Node.js
launchers select one embedded binary without downloading or executing any
installer code.

Download and verify the exact release asset before installing it locally:

```console
gh release download <tag> --repo celiumsai/pliegocss --pattern 'pliegocss-pnpm-*.tgz'
gh release verify-asset <tag> ./pliegocss-pnpm-<version>.tgz --repo celiumsai/pliegocss
gh attestation verify ./pliegocss-pnpm-<version>.tgz --repo celiumsai/pliegocss
pnpm add --save-dev --save-exact ./pliegocss-pnpm-<version>.tgz
pnpm exec pliego-cssc --version
```

Supported hosts are Windows x64, Linux x64 GNU, and macOS arm64. Cargo remains
the supported installation path for other Rust targets.
