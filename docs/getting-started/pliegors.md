# PliegoRS + PliegoCSS from an empty project

Status: implemented local-source workflow for the MVP integration fixture

This guide starts with `pliego new` and ends with deterministic SSR/SSG output that can contain
PliegoCSS classes, route and island CSS partitions, resumable state, and ordinary external CSS.
PliegoRS and PliegoCSS are not published yet, so the commands use local checkouts.

## What the integration does today

PliegoCSS emits standard CSS and stable class names. It does not add a browser styling runtime.
PliegoRS owns views, SSR, resumability, SSG publication, and asset delivery.

The current integration automatically collects typed component/route/island reachability and derives
`shared`, route, island, and `unreachable` source partitions from one `ProductRegistry`. The bridge
from the PliegoRS registry to `pliego-css-source::ApplicationTopology` is still application-owned.
It is implemented and tested in the executable fixture, but it is not yet a one-command PliegoRS
starter feature.

## 1. Scaffold the PliegoRS application

Keep the PliegoRS and PliegoCSS checkouts beside the application while the crates are unreleased:

```text
workspace/
├── PliegoCSS/
├── pliegors/
└── my-site/
```

Install the local CLI and create the project:

```console
cargo +1.85.0 install --locked --path pliegors/crates/pliego-cli
pliego new my-site --framework-path pliegors
cd my-site
pliego check
```

`pliego new` creates a standalone Rust 2024 project, standard asset directory, `pliego.toml`, and
an initial deterministic SSG application. Rust 1.85 is the minimum supported compiler.

## 2. Add the local PliegoCSS and resumability crates

Add these entries to the generated `[dependencies]`. Adjust each path for the location of your
application:

```toml
pliego-css = { path = "../PliegoCSS/crates/pliego-css" }
pliego-css-source = { path = "../PliegoCSS/crates/pliego-css-source" }
pliego-macros = { path = "../pliegors/crates/pliego-macros" }
pliego-resume = { path = "../pliegors/crates/pliego-resume" }
```

Keep the generated `pliego-dom` and `pliego-ssg` dependencies. Then refresh and verify the lockfile:

```console
cargo +1.85.0 generate-lockfile
pliego check
pliego css check --seed
```

The last command is optional delegation to the separately installed `pliego-cssc`; PliegoRS does not
embed the PliegoCSS compiler. It defaults to `--source src` and forwards additional check options.

## 3. Author a typed utility and bind it to a component

Create one source module per useful ownership boundary. For example, `src/styles/home.rs`:

```rust
use pliego_css::{Style, pc};
use pliego_ssg::ProductComponent;

pub fn component() -> ProductComponent {
    pliego_ssg::product_component!("my-site::home")
}

pub fn home_style() -> Style {
    pc!("grid gap-6 p-6 md:grid-cols-2")
}
```

`product_component!` captures the normalized declaration source. The collector can therefore bind
the `pc!` site to product topology without guessing ownership from a filename. An unowned compiled
`pc!` or `pcx!` site is an error.

Use the compiled style as a normal class value:

```rust
use pliego_dom::{IntoView, View, el};

fn home() -> View {
    el("main")
        .class(String::from(crate::styles::home::home_style()))
        .child("Hello from PliegoCSS")
        .into_view()
}
```

The resulting HTML contains an ordinary generated class. SSR clients do not need PliegoCSS IR or a
style runtime.

## 4. Register routes, components, and islands once

Create `src/product.rs` and make the registry the topology source for both CSS collection and SSG:

```rust
use pliego_ssg::{ProductIsland, ProductRegistry, ProductRoute};

pub fn application_registry() -> ProductRegistry {
    let home = crate::styles::home::component();
    let counter = crate::styles::counter::component();
    let home_id = home.id().to_owned();
    let counter_id = counter.id().to_owned();

    ProductRegistry::new()
        .component(home)
        .component(counter)
        .island(ProductIsland::new("visit-counter", "visit-counter").component(counter_id))
        .route(ProductRoute::new("home", "/").component(home_id))
        .route(ProductRoute::new("visit", "/visit").island("visit-counter"))
}
```

Call `application_registry().validate()?` before collection or publication. Registry validation
rejects duplicate IDs, unsafe routes, missing component references, and missing island references.

## 5. Derive reachability and CSS partitions

The application adapter converts the immutable `ProductRegistry` into
`pliego_css_source::ApplicationTopology`, scans the exact Cargo/rustc source inventory, and emits:

- canonical reachability schema 1;
- an automatic bundle plan for shared, route, island, and unreachable sources;
- schema-5 CSS manifests and a verified asset plan;
- a Project Index joining source sites to semantic and physical CSS IDs.

Use these fixture files as the current executable implementation:

- [registry shared by collection and SSG](../../integration-tests/pliegors-smoke/src/product.rs)
- [registry-to-topology and automatic partition adapter](../../integration-tests/pliegors-smoke/src/css_adapter.rs)
- [collector entry point](../../integration-tests/pliegors-smoke/src/bin/collector.rs)
- [verified SSG consumer](../../integration-tests/pliegors-smoke/src/bin/ssg.rs)

Do not infer ownership from module names or manually maintain a second route graph. The registry,
compiled Cargo source inventory, reachability sidecar, bundle plan, manifests, asset plan, and SSG
ledger are checked against each other and fail closed on drift.

Run the complete reference build from the PliegoCSS checkout:

```console
pnpm integration:pliegors
```

## 6. Publish route and island CSS through normal assets

PliegoRS publishes compiler output as ordinary `Asset` values and links only the bundle selection
for the current route plus its rendered islands. A `Head` can also preload a stylesheet that it
already links:

```rust
use pliego_ssg::{Asset, Head, Site};

let css = std::fs::read("target/pliegocss/shared.css")?;
let site = Site::new().asset(Asset::new("assets/shared.css", css));
let head = Head::new("Home")
    .preload_stylesheet("/assets/shared.css")
    .stylesheet("/assets/shared.css");
```

Preload is explicit delivery policy, not automatic performance magic. The reference adapter selects
only the single theme-bearing shared bundle; the browser gate proves reuse of one request but does
not claim a latency improvement.

`pliego build` writes the deployable site, `pliego.build.json`, and the receipt-bound causal graph:

```console
pliego build
pliego inspect
pliego why artifact /
pliego preview
```

The output under `target/site` is standard HTML, CSS, JavaScript, and optional WASM.

## 7. Add a resumable island

PliegoRS islands serialize state into SSR markup. Actions update the existing DOM instead of
reconstructing the view:

```rust
use pliego_dom::{IntoView, el};
use pliego_resume::{Island, increment, text_binding};

let button = increment(
    el("button").attr("type", "button").child("+5"),
    "minutes",
    5,
)?;
let value = text_binding("minutes", "15")?;
let island = Island::new("visit-counter")?
    .state_i64("minutes", 15)?
    .child(el("section").child(value).child(button))
    .into_view()?;
```

Register the island and its route occurrence in the same `ProductRegistry`. The route then selects
its route bundles plus the bundles owned by the rendered island. The executable browser gate builds
the Rust/WASM client and proves state `15 -> 20` while preserving the same document, island, button,
and bound-text DOM objects:

```console
pnpm integration:pliegors-browser
```

## 8. Keep external CSS when it is the right tool

PliegoCSS is additive. Existing class strings and external stylesheets remain valid:

```rust
const LEGACY_CSS: &[u8] = include_bytes!("../assets/legacy.css");

let site = Site::new().asset(Asset::new(
    "assets/legacy.css",
    LEGACY_CSS.to_vec(),
));
let head = Head::new("Home").stylesheet("/assets/legacy.css");
let view = el("div").class("legacy-card").child("Interoperable");
```

Use external CSS for third-party packages, global documents, or gradual migration. Use typed
utilities where compile-time validation, provenance, reachability, or token policy adds value.

## 9. Run the development loop

Run PliegoCSS asset generation and the PliegoRS server as separate processes:

```console
# Terminal 1
pliego-cssc watch --source src --output assets/pliego.css \
  --manifest assets/pliego.manifest.json

# Terminal 2
pliego dev
```

The current certified loop is a PliegoRS-owned full-page reload over SSE, not CSS-only HMR. See the
[development-loop guide](../how-to/pliegors-dev-loop.md) for atomic publication and measured local
latency.

## Current boundary

The local gates prove deterministic SSR/SSG, complete ownership for the exact three built Cargo
targets, route/island partitioning, dead-style pruning, asset integrity, one Chromium resumability
event, preload reuse, and one measured WASM client. They do not yet prove hosted deployment,
multiple browsers, every Cargo feature/target combination, or a production-application budget.
