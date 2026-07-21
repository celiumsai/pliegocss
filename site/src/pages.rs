// SPDX-License-Identifier: Apache-2.0

use pliego_css::{Style, pc};
use pliego_dom::{Element as DomElement, IntoView, View, el as dom_el};
use pliego_ssg::{Head, Page};
use serde_json::json;
use std::cell::Cell;
use std::collections::BTreeMap;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Locale {
    En,
    Es,
}

impl Locale {
    const fn code(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Es => "es",
        }
    }
}

thread_local! {
    static CURRENT_LOCALE: Cell<Locale> = const { Cell::new(Locale::En) };
}

fn with_locale<T>(locale: Locale, render: impl FnOnce() -> T) -> T {
    CURRENT_LOCALE.with(|current| {
        let previous = current.replace(locale);
        let rendered = render();
        current.set(previous);
        rendered
    })
}

fn current_locale() -> Locale {
    CURRENT_LOCALE.get()
}

fn spanish_catalog() -> &'static BTreeMap<String, String> {
    static CATALOG: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("../i18n/es.json"))
            .expect("site/i18n/es.json must be a string-to-string JSON object")
    })
}

fn translate(locale: Locale, value: &str) -> String {
    match locale {
        Locale::En => value.to_owned(),
        Locale::Es => spanish_catalog()
            .get(value)
            .cloned()
            .unwrap_or_else(|| value.to_owned()),
    }
}

fn localize_route(locale: Locale, route: &str) -> String {
    if locale == Locale::En || route.starts_with("/es/") {
        return route.to_owned();
    }
    if route == "/" {
        "/es/".to_owned()
    } else if route == "/404.html" {
        "/es/404.html".to_owned()
    } else {
        format!("/es{route}")
    }
}

fn localized_attribute(name: &str, value: String) -> String {
    if name == "href"
        && value.starts_with('/')
        && !value.starts_with("//")
        && !value.starts_with("/assets/")
        && !(value.starts_with("/brand/") && value != "/brand/")
        && !value.starts_with("/media/")
        && !value.starts_with("/fonts/")
        && !value.starts_with("/favicon")
        && !value.starts_with("/icon-")
        && !value.starts_with("/social-card")
        && !value.starts_with("/site.webmanifest")
        && !value.starts_with("/.well-known/")
    {
        localize_route(current_locale(), &value)
    } else if matches!(
        name,
        "alt" | "aria-label" | "aria-description" | "title" | "placeholder"
    ) {
        translate(current_locale(), &value)
    } else {
        value
    }
}

#[derive(Clone)]
struct Element(DomElement);

fn el(tag: &str) -> Element {
    Element(dom_el(tag))
}

trait LocalizedChild {
    fn into_localized_view(self) -> View;
}

impl LocalizedChild for Element {
    fn into_localized_view(self) -> View {
        self.into_view()
    }
}

impl LocalizedChild for View {
    fn into_localized_view(self) -> View {
        self
    }
}

impl LocalizedChild for String {
    fn into_localized_view(self) -> View {
        translate(current_locale(), &self).into_view()
    }
}

impl LocalizedChild for &str {
    fn into_localized_view(self) -> View {
        translate(current_locale(), self).into_view()
    }
}

impl Element {
    fn class(self, value: impl Into<String>) -> Self {
        Self(self.0.class(value))
    }

    fn id(self, value: impl Into<String>) -> Self {
        Self(self.0.id(value))
    }

    fn attr(self, name: impl AsRef<str>, value: impl Into<String>) -> Self {
        let name = name.as_ref();
        Self(self.0.attr(name, localized_attribute(name, value.into())))
    }

    fn attr_raw(self, name: impl AsRef<str>, value: impl Into<String>) -> Self {
        Self(self.0.attr(name, value.into()))
    }

    fn child(self, child: impl LocalizedChild) -> Self {
        Self(self.0.child(child.into_localized_view()))
    }
}

impl IntoView for Element {
    fn into_view(self) -> View {
        self.0.into_view()
    }
}

const HERO_STYLE: Style = pc!("flex flex-col gap-6");
const ACTION_STYLE: Style = pc!(
    "inline-flex items-center gap-2 rounded-lg bg-accent px-4 py-2 text-sm font-semibold text-white hover:bg-accent-strong focus-visible:ring-2"
);
const PANEL_STYLE: Style = pc!("rounded-xl border border-line bg-surface p-6 shadow-md");
const LABEL_STYLE: Style = pc!("text-xs font-semibold text-muted");

struct DocPage {
    route: &'static str,
    title: &'static str,
    category: &'static str,
    eyebrow: &'static str,
    summary: &'static str,
    sections: &'static [DocSection],
}

struct DocSection {
    id: &'static str,
    title: &'static str,
    body: &'static str,
    code: Option<&'static str>,
}

const DOCS: &[DocPage] = &[
    DocPage {
        route: "/docs/getting-started/",
        title: "Start with the CSS you already have.",
        category: "Start",
        eyebrow: "Getting started",
        summary: "Audit first, compile when it adds value, and keep the browser contract ordinary.",
        sections: &[
            DocSection {
                id: "install",
                title: "Install the release candidate",
                body: "The GitHub tag is available in the private canonical repository. Crates.io publication is intentionally pending.",
                code: Some(
                    "cargo install --git https://github.com/celiumsai/pliegocss \\\n  --tag v0.1.0-rc.1 pliego-cssc",
                ),
            },
            DocSection {
                id: "audit",
                title: "Audit ordinary CSS",
                body: "Start without changing authoring. The analyzer emits human, canonical JSON, or SARIF findings and fails closed on unknown surfaces.",
                code: Some(
                    "pliego-cssc audit --input app.css \\\n  --targets baseline-widely --format human",
                ),
            },
            DocSection {
                id: "compile",
                title: "Add typed identities selectively",
                body: "Rust literals are validated at compile time and compile to static standards-compliant CSS. There is no styling runtime.",
                code: Some(
                    "use pliego_css::{pc, Style};\n\nconst BUTTON: Style = pc!(\n  \"inline-flex gap-2 rounded-lg bg-accent px-4 py-2\"\n);",
                ),
            },
        ],
    },
    DocPage {
        route: "/docs/typed-styles/",
        title: "Types at author time. CSS at run time.",
        category: "Core",
        eyebrow: "Typed styles",
        summary: "Stable 128-bit identities connect Rust source, emitted classes, manifests, and physical declarations.",
        sections: &[
            DocSection {
                id: "pc",
                title: "One literal, one stable identity",
                body: "pc! accepts one visible string literal. The macro parses semantics, rejects conflicting slots, and returns a sealed Style.",
                code: Some(
                    "const CARD: Style = pc!(\n  \"grid gap-4 rounded-xl border border-line p-6 shadow-md\"\n);",
                ),
            },
            DocSection {
                id: "pcx",
                title: "Visible state spaces",
                body: "pcx! compiles every visible branch combination and selects one identity at runtime. Independent clauses that can collide are rejected.",
                code: Some(
                    "let style = pcx!(\n  \"block\",\n  if active { \"opacity-100\" } else { \"opacity-50\" }\n);",
                ),
            },
            DocSection {
                id: "lineage",
                title: "Follow the physical effect",
                body: "Manifest schema 5 joins semantic declarations to final rules, conditions, byte ranges, and synthesized effects such as box-shadow.",
                code: None,
            },
        ],
    },
    DocPage {
        route: "/docs/audit-and-transform/",
        title: "Standards-first, not framework-first.",
        category: "Standard CSS",
        eyebrow: "Audit and transform",
        summary: "Analyze standard CSS, enforce policy, and transform output without claiming ownership of authored source.",
        sections: &[
            DocSection {
                id: "classifier",
                title: "Bounded classification",
                body: "The classifier recognizes deliberate standard-CSS surfaces and reports unsupported constructs instead of guessing. A versioned corpus controls the claim.",
                code: None,
            },
            DocSection {
                id: "transform",
                title: "Deterministic transformation",
                body: "Target browsers, minify, and check output drift. Add --check in CI to remain read-only.",
                code: Some(
                    "pliego-cssc transform-css --input app.css \\\n  --output dist/app.css --targets modern --format minified",
                ),
            },
            DocSection {
                id: "policy",
                title: "Compatibility, accessibility, and budgets",
                body: "Policy documents bind findings to explicit targets and limits. The same canonical findings feed terminals, JSON automation, and SARIF.",
                code: None,
            },
        ],
    },
    DocPage {
        route: "/docs/evidence/",
        title: "A green build should explain itself.",
        category: "Evidence",
        eyebrow: "Evidence and provenance",
        summary: "Artifacts carry hashes, source lineage, graphs, findings, and receipts designed for replay.",
        sections: &[
            DocSection {
                id: "manifest",
                title: "Canonical manifests",
                body: "Style identity, class identity, theme identity, origins, conditions, physical declarations, and final CSS hashes live in a deterministic document.",
                code: Some(
                    "pliego-cssc compile --source src --seed --theme \\\n  --output dist/app.css --manifest dist/app.manifest.json",
                ),
            },
            DocSection {
                id: "receipts",
                title: "Receipt-last publication",
                body: "Controlled operations stage output, lock the destination, verify inputs, publish artifacts, and write the receipt last.",
                code: None,
            },
            DocSection {
                id: "benchmarks",
                title: "Measured, inherited, pending, uncertain",
                body: "Release readiness preserves the evidence class. Old hosted evidence never silently passes changed source.",
                code: None,
            },
        ],
    },
    DocPage {
        route: "/docs/migration/",
        title: "Adopt without surrendering the exit.",
        category: "Migration",
        eyebrow: "Controlled migration",
        summary: "Inventory first, generate bounded plans, preserve originals, and detect drift immediately before publication.",
        sections: &[
            DocSection {
                id: "inventory",
                title: "Read-only discovery",
                body: "Tailwind, Sass, CSS Modules, and standard CSS can be inventoried before any replacement plan exists.",
                code: Some("pliego-cssc migration-project-plan ./migration-declaration.json"),
            },
            DocSection {
                id: "reversible",
                title: "Reversible replacements",
                body: "Plans bind source and destination hashes. Apply and rollback use adjacent locks, pre-publication revalidation, backups, and group compensation.",
                code: None,
            },
            DocSection {
                id: "limits",
                title: "No speculative codemods",
                body: "Unsupported or context-dependent constructs stay in the inventory. The migration layer does not infer framework semantics it cannot prove.",
                code: None,
            },
        ],
    },
    DocPage {
        route: "/docs/integrations/pliegors/",
        title: "Native together. Independent by contract.",
        category: "Integrations",
        eyebrow: "PliegoRS integration",
        summary: "PliegoRS is the first native consumer, not the only target. It supplies topology; PliegoCSS supplies style identity and artifacts.",
        sections: &[
            DocSection {
                id: "boundary",
                title: "Explicit ownership",
                body: "PliegoRS owns routes, pages, assets, and browser lifecycle. PliegoCSS owns parsing, semantic conflicts, CSS, manifests, and policy findings.",
                code: None,
            },
            DocSection {
                id: "bundles",
                title: "Route and island bundles",
                body: "A reachability sidecar projects application-owned topology into selected CSS bundles without PliegoCSS pretending to understand the framework.",
                code: None,
            },
            DocSection {
                id: "proof",
                title: "Replayed from published crates",
                body: "The maintained fixture locks PliegoRS 0.0.2 registry packages and verifies the remote-reachable framework revision separately.",
                code: Some("PLIEGORS_ROOT=/path/to/pliegors pnpm integration:pliegors"),
            },
        ],
    },
    DocPage {
        route: "/docs/installation/",
        title: "Choose an adoption surface.",
        category: "Start",
        eyebrow: "Installation",
        summary: "Install the CLI for any project, add the Rust crates only when typed authoring belongs in the application.",
        sections: &[
            DocSection {
                id: "cli",
                title: "CLI-only adoption",
                body: "The command-line compiler can audit, transform, explain, bundle, and migrate standard CSS without adding a runtime dependency to the application.",
                code: Some(
                    "cargo install --git https://github.com/celiumsai/pliegocss \\\n  --tag v0.1.0-rc.1 pliego-cssc\npliego-cssc --version",
                ),
            },
            DocSection {
                id: "rust",
                title: "Typed Rust authoring",
                body: "Add pliego-css and its macro crate to applications that want compile-time style identities. The emitted result remains ordinary CSS.",
                code: Some(
                    "[dependencies]\npliego-css = { git = \"https://github.com/celiumsai/pliegocss\", tag = \"v0.1.0-rc.1\" }",
                ),
            },
            DocSection {
                id: "pin",
                title: "Pin the exact release identity",
                body: "Before the public release, use the signed project tag plus Cargo.lock. Do not track a moving branch in a production build.",
                code: None,
            },
        ],
    },
    DocPage {
        route: "/docs/first-audit/",
        title: "Learn before changing.",
        category: "Start",
        eyebrow: "First audit",
        summary: "Run a read-only CSS audit, inspect canonical findings, then decide whether transformation or policy is justified.",
        sections: &[
            DocSection {
                id: "human",
                title: "Read the first report",
                body: "Human output is optimized for local diagnosis. Every finding carries a stable code and source location rather than an opaque score.",
                code: Some(
                    "pliego-cssc audit --input app.css \\\n  --targets baseline-widely --format human",
                ),
            },
            DocSection {
                id: "automation",
                title: "Feed automation safely",
                body: "Canonical JSON and SARIF preserve the same finding model for CI, code scanning, and archival evidence.",
                code: Some(
                    "pliego-cssc audit --input app.css --targets modern --format sarif > findings.sarif",
                ),
            },
            DocSection {
                id: "unknown",
                title: "Unknown means unknown",
                body: "The standard-CSS classifier is bounded. Unsupported selectors, properties, or values fail closed under compatibility policy rather than being treated as safe.",
                code: None,
            },
        ],
    },
    DocPage {
        route: "/docs/editor-setup/",
        title: "Put the compiler in the feedback loop.",
        category: "Start",
        eyebrow: "Editor setup",
        summary: "Use the language server for diagnostics, completion, hover evidence, and catalog-aware authoring.",
        sections: &[
            DocSection {
                id: "vscode",
                title: "VS Code extension",
                body: "The maintained extension launches pliego-css-lsp, discovers the project catalog, and renders diagnostics in Rust source.",
                code: Some("pnpm --filter pliegocss-vscode package"),
            },
            DocSection {
                id: "generic",
                title: "Any LSP client",
                body: "Launch the server over stdio and point it at the same compiler and workspace used by CI. The protocol stays editor-neutral.",
                code: Some("pliego-css-lsp --stdio"),
            },
            DocSection {
                id: "safety",
                title: "Bound external tools",
                body: "Compiler subprocesses have explicit deadlines and output caps. A stalled custom executable must not freeze the editor forever.",
                code: None,
            },
        ],
    },
    DocPage {
        route: "/docs/syntax/",
        title: "A finite language with visible edges.",
        category: "Core",
        eyebrow: "Utility syntax",
        summary: "Utilities lower into semantic slots, conditions, and typed values before any CSS serialization occurs.",
        sections: &[
            DocSection {
                id: "shape",
                title: "Utility plus typed value",
                body: "Spacing, color, typography, layout, effects, and interactivity draw from one validated theme registry. Arbitrary escape hatches remain policy-visible.",
                code: Some("flex items-center gap-4 rounded-lg bg-surface px-4 py-2 text-sm"),
            },
            DocSection {
                id: "conditions",
                title: "Conditions are structure",
                body: "Responsive, state, attribute, container, layer, and media variants become normalized condition paths. Their order is not incidental string decoration.",
                code: Some("md:grid-cols-2 hover:bg-accent-strong aria-[expanded=true]:ring-2"),
            },
            DocSection {
                id: "important",
                title: "Importance follows the physical effect",
                body: "A trailing exclamation mark marks the semantic contribution important. Synthesized effects such as the composed box-shadow preserve that authority in final CSS and lineage.",
                code: Some("ring-2! shadow-md!"),
            },
        ],
    },
    DocPage {
        route: "/docs/composition/",
        title: "Compose by meaning, not source order.",
        category: "Core",
        eyebrow: "Composition",
        summary: "Compatible slots commute. Conflicting slots require an explicit branch-over-base operation.",
        sections: &[
            DocSection {
                id: "commute",
                title: "Equivalent input, equivalent identity",
                body: "Utilities that occupy independent semantic slots canonicalize to the same identity even when authored in a different order.",
                code: Some("flex gap-4 p-6\np-6 flex gap-4"),
            },
            DocSection {
                id: "conflict",
                title: "Ambiguous lists are rejected",
                body: "p-4 p-6 does not mean last one wins. Both declarations claim the same slot under the same condition, so compilation stops.",
                code: Some("pliego-cssc check --style \"p-4 p-6\" --seed"),
            },
            DocSection {
                id: "override",
                title: "Overrides are explicit operations",
                body: "When an application intentionally replaces one semantic assignment with another, use composition so the override relationship remains visible and inspectable.",
                code: Some("pliego-cssc compile --compose \"p-4\" \"p-6\" --seed"),
            },
        ],
    },
    DocPage {
        route: "/docs/variants/",
        title: "Conditions without string-order folklore.",
        category: "Core",
        eyebrow: "Variants and conditions",
        summary: "Every variant joins a typed condition path with deterministic normalization and specificity behavior.",
        sections: &[
            DocSection {
                id: "responsive",
                title: "Responsive paths",
                body: "Theme breakpoints are validated once and emitted as ordered media conditions. Assignments at different breakpoints do not conflict.",
                code: Some("grid-cols-1 md:grid-cols-2 lg:grid-cols-3"),
            },
            DocSection {
                id: "state",
                title: "State and attributes",
                body: "Pseudo-class and bounded attribute variants stay in semantic IR so policy and physical lineage can explain their selectors.",
                code: Some("hover:bg-accent focus-visible:ring-2 data-[state=open]:opacity-100"),
            },
            DocSection {
                id: "layers",
                title: "Cascade layers",
                body: "Fixed layer variants model author-controlled precedence without allowing arbitrary source-order dependence to leak into identity.",
                code: Some("layer-components:bg-surface layer-utilities:p-4"),
            },
        ],
    },
    DocPage {
        route: "/docs/themes/",
        title: "Resolve tokens before styles.",
        category: "Core",
        eyebrow: "Themes and tokens",
        summary: "TOML themes and DTCG Resolver documents feed the same typed registry, identity, graph, and emitted variables.",
        sections: &[
            DocSection {
                id: "toml",
                title: "Extend the seed",
                body: "A project theme can override or add typed colors, spacing, radii, typography, and breakpoints without changing utility grammar.",
                code: Some(
                    "schema = 1\nextends = \"seed\"\n\n[tokens.color]\nbrand = \"oklch(62% 0.18 250)\"",
                ),
            },
            DocSection {
                id: "dtcg",
                title: "Select DTCG contexts explicitly",
                body: "Resolver inputs are never auto-discovered. Modifier selections participate in configuration identity even when two contexts resolve to equal values.",
                code: Some(
                    "pliego-cssc compile --tokens product.resolver.json \\\n  --token-input appearance=dark --style \"bg-brand\"",
                ),
            },
            DocSection {
                id: "graph",
                title: "Publish the complete Token Graph",
                body: "Controlled output binds selected CSS to canonical token provenance, aliases, permutations, and reachability-aware usage evidence.",
                code: None,
            },
        ],
    },
    DocPage {
        route: "/docs/standard-css/policies/",
        title: "Make browser support and budgets executable.",
        category: "Standard CSS",
        eyebrow: "Policies",
        summary: "Compatibility, accessibility, payload, package, route, and ownership policies turn expectations into reviewable inputs.",
        sections: &[
            DocSection {
                id: "compatibility",
                title: "Version the target",
                body: "baseline-widely and modern are frozen policy profiles with explicit browser and source-data identities. They do not drift silently with a dependency update.",
                code: Some("pliego-cssc compatibility --targets baseline-widely"),
            },
            DocSection {
                id: "budgets",
                title: "Bind budgets to owners",
                body: "Direct files can name budget subjects manually. Asset Plans use an ownership sidecar to aggregate package and composed-route costs without guessing responsibility.",
                code: Some(
                    "pliego-cssc audit --asset-plan dist/pliego.assets.json \\\n  --ownership pliego.ownership.json --budget-policy budgets.json",
                ),
            },
            DocSection {
                id: "accessibility",
                title: "Audit static accessibility contracts",
                body: "Contrast and interaction guards use explicit policy plus token evidence. Dynamic visual behavior still requires browser and assistive-technology testing.",
                code: None,
            },
        ],
    },
    DocPage {
        route: "/docs/cascade/",
        title: "Explain the winning declaration.",
        category: "Standard CSS",
        eyebrow: "Cascade inspection",
        summary: "Inspect authored CSS by element and longhand to see matching selectors, precedence, and the final winner.",
        sections: &[
            DocSection {
                id: "query",
                title: "Ask one precise question",
                body: "Provide a CSS file, a bounded element description, and one longhand. The command reports candidates instead of simulating an entire browser.",
                code: Some(
                    "pliego-cssc explain-cascade --input app.css \\\n  --element 'button#save.action' --property background-color",
                ),
            },
            DocSection {
                id: "winner",
                title: "See why it won",
                body: "Origin, importance, layer, specificity, and source position are surfaced in comparison order with a stable machine-readable form.",
                code: None,
            },
            DocSection {
                id: "boundary",
                title: "Keep the model honest",
                body: "This is a static bounded inspector. Runtime DOM state, animation interpolation, adopted sheets, and script mutation remain browser responsibilities.",
                code: None,
            },
        ],
    },
    DocPage {
        route: "/docs/manifests/",
        title: "Trace source intent into emitted bytes.",
        category: "Evidence",
        eyebrow: "Style manifests",
        summary: "Manifest schema 5 connects source origins, semantic assignments, final selectors, physical declarations, and CSS ranges.",
        sections: &[
            DocSection {
                id: "identity",
                title: "Identity chain",
                body: "StyleId identifies canonical semantics. Class identity, theme identity, configuration, and final CSS hash bind that style into one build.",
                code: Some(
                    "pliego-cssc compile --source src --manifest dist/app.manifest.json \\\n  --manifest-version 5 --output dist/app.css",
                ),
            },
            DocSection {
                id: "physical",
                title: "Physical lineage",
                body: "Generated custom properties and synthesized declarations retain contributors, importance, conditions, selector, property, value, and output byte range.",
                code: None,
            },
            DocSection {
                id: "versions",
                title: "Request the schema you consume",
                body: "Schema 3 supports style manifests, schema 4 adds reachability ownership, and schema 5 adds closed physical tracing. Consumers must pin their contract.",
                code: None,
            },
        ],
    },
    DocPage {
        route: "/docs/control-artifacts/",
        title: "Publish the receipt last.",
        category: "Evidence",
        eyebrow: "Control artifacts",
        summary: "Controlled builds stage related outputs, lock the destination, verify hashes, compensate failures, and expose one completed receipt.",
        sections: &[
            DocSection {
                id: "group",
                title: "One rollback-capable group",
                body: "CSS, source map, style manifest, Token Graph, findings, control manifest, and receipt are generated from one exact input snapshot.",
                code: Some(
                    "pliego-cssc compile --source src --output dist/app.css \\\n  --manifest dist/app.manifest.json --control-dir dist",
                ),
            },
            DocSection {
                id: "check",
                title: "Check without writing",
                body: "The same command with --check recomputes every artifact and fails on byte drift without taking publication locks or replacing files.",
                code: Some(
                    "pliego-cssc compile --source src --output dist/app.css \\\n  --manifest dist/app.manifest.json --control-dir dist --check",
                ),
            },
            DocSection {
                id: "receipt",
                title: "Completion is explicit",
                body: "Consumers treat the fixed receipt as the publication boundary. Its absence means the group is incomplete, even if individual files are present.",
                code: None,
            },
        ],
    },
    DocPage {
        route: "/docs/critical-css/",
        title: "Prove what rendered above the fold.",
        category: "Evidence",
        eyebrow: "Critical style evidence",
        summary: "Join observed browser behavior to project topology and retained style identity without letting an observation silently authorize removal.",
        sections: &[
            DocSection {
                id: "observe",
                title: "Observe a real route",
                body: "Capture viewport, route, interaction state, loaded assets, matched styles, and source identities from a browser run against the built application.",
                code: None,
            },
            DocSection {
                id: "join",
                title: "Join exact universes",
                body: "Evidence is accepted only when its project index, asset plan, reachability, and CSS identities match the build under review.",
                code: None,
            },
            DocSection {
                id: "authority",
                title: "Observation does not equal deletion",
                body: "A style unseen in one scope remains unknown outside that scope. Explicit retention and reachability policies control removal authority.",
                code: None,
            },
        ],
    },
    DocPage {
        route: "/docs/migration/tailwind/",
        title: "Inventory Tailwind before translating it.",
        category: "Migration",
        eyebrow: "Tailwind migration",
        summary: "Recover the real utility surface, classify supported roles, and keep unsupported framework behavior visible.",
        sections: &[
            DocSection {
                id: "inventory",
                title: "Scan without replacement",
                body: "The inventory recognizes bounded class-bearing surfaces and reports candidates with source ranges. It does not rewrite template semantics during discovery.",
                code: Some("pliego-cssc migration-inventory tailwind src/App.tsx"),
            },
            DocSection {
                id: "plan",
                title: "Generate a hash-bound project plan",
                body: "A declaration selects roots and destinations. The plan records exact source and output hashes before any replacement can be applied.",
                code: Some("pliego-cssc migration-project-plan migration.json"),
            },
            DocSection {
                id: "gaps",
                title: "Preserve unsupported behavior",
                body: "Plugins, generated variants, arbitrary selectors, and runtime class construction remain explicit migration findings unless a project-owned adapter proves them.",
                code: None,
            },
        ],
    },
    DocPage {
        route: "/docs/migration/sass/",
        title: "Separate syntax conversion from semantic adoption.",
        category: "Migration",
        eyebrow: "Sass migration",
        summary: "Inventory variables, nesting, mixins, imports, and generated surfaces without pretending every Sass abstraction is a utility.",
        sections: &[
            DocSection {
                id: "inventory",
                title: "Map the source surface",
                body: "The inventory records statically visible Sass roles and unresolved dynamic behavior so the team can choose what stays authored CSS.",
                code: Some("pliego-cssc migration-inventory sass styles/main.scss"),
            },
            DocSection {
                id: "css-first",
                title: "Ordinary CSS is a valid destination",
                body: "Use the standard-CSS transformer for syntax and compatibility work. Adopt typed PliegoCSS styles only where identity and provenance pay for themselves.",
                code: None,
            },
            DocSection {
                id: "rollback",
                title: "Keep originals and rollback evidence",
                body: "Applied replacements preserve source backups and hash-bound rollback records. A changed destination aborts rather than overwriting concurrent edits.",
                code: None,
            },
        ],
    },
    DocPage {
        route: "/docs/migration/css-modules/",
        title: "Preserve component ownership.",
        category: "Migration",
        eyebrow: "CSS Modules migration",
        summary: "Discover module exports and usage while keeping application-owned component topology separate from CSS semantics.",
        sections: &[
            DocSection {
                id: "inventory",
                title: "Inventory exported classes",
                body: "The scanner identifies bounded module class definitions and their source locations without assuming a particular bundler naming strategy.",
                code: Some("pliego-cssc migration-inventory css-modules src/Card.module.css"),
            },
            DocSection {
                id: "ownership",
                title: "Let the application own reachability",
                body: "Route and component reachability comes from the framework or product index, not from filename heuristics inside the CSS compiler.",
                code: None,
            },
            DocSection {
                id: "mixed",
                title: "Mixed adoption is first-class",
                body: "A project can retain modules for component styles, audit their emitted CSS, and introduce typed identities only for new or shared surfaces.",
                code: None,
            },
        ],
    },
    DocPage {
        route: "/docs/tooling/cli/",
        title: "One CLI, explicit modes.",
        category: "Tooling",
        eyebrow: "Command-line reference",
        summary: "Compile, inspect, audit, transform, explain, bundle, watch, migrate, format, plan, and verify with strict argument contracts.",
        sections: &[
            DocSection {
                id: "discover",
                title: "Discover the real command surface",
                body: "Root and command help are generated from the maintained command synopsis. Unknown options, duplicate single-value options, and missing values fail.",
                code: Some("pliego-cssc --help\npliego-cssc compile --help"),
            },
            DocSection {
                id: "diagnostics",
                title: "Select diagnostic transport globally",
                body: "Human and JSON diagnostics can be requested before or after the command without stealing option values that happen to resemble flags.",
                code: Some("pliego-cssc --diagnostic-format json check --style \"p-4 p-6\" --seed"),
            },
            DocSection {
                id: "determinism",
                title: "Use --check in CI",
                body: "Commands that publish controlled or generated artifacts expose a read-only comparison path. CI should verify the checked-in or staged output, not regenerate it silently.",
                code: None,
            },
        ],
    },
    DocPage {
        route: "/docs/tooling/lsp/",
        title: "Compiler feedback without editor lockups.",
        category: "Tooling",
        eyebrow: "Language server",
        summary: "The LSP shares parser and catalog contracts with the CLI while bounding subprocess time and output.",
        sections: &[
            DocSection {
                id: "features",
                title: "Diagnostics, completion, and hover",
                body: "Visible pc! and pcx! literals receive parser diagnostics, catalog-backed completion, and inspectable utility information.",
                code: None,
            },
            DocSection {
                id: "cancellation",
                title: "Cancel stale document work",
                body: "Document version changes cancel obsolete checks. Independent process deadlines prevent an unchanged document from waiting forever on a stalled compiler.",
                code: None,
            },
            DocSection {
                id: "limits",
                title: "Bound untrusted output",
                body: "stdout and stderr are read under explicit caps. Oversized or timed-out compiler responses become diagnostics instead of unbounded memory growth.",
                code: None,
            },
        ],
    },
    DocPage {
        route: "/docs/tooling/watch/",
        title: "Keep the last complete truth.",
        category: "Tooling",
        eyebrow: "Watch mode",
        summary: "Poll coherent snapshots, republish only valid groups, and retain the last valid output across transient edits.",
        sections: &[
            DocSection {
                id: "run",
                title: "Watch inputs and output",
                body: "The watcher accepts the same source, theme, target, format, manifest, reachability, and controlled-output options as compile.",
                code: Some(
                    "pliego-cssc watch --source src --theme \\\n  --output dist/app.css --manifest dist/app.manifest.json",
                ),
            },
            DocSection {
                id: "snapshot",
                title: "Confirm one coherent snapshot",
                body: "Inputs are observed twice before compilation so an editor save sequence does not publish a mixture of old and new files.",
                code: None,
            },
            DocSection {
                id: "last-valid",
                title: "Never replace validity with a partial edit",
                body: "A parser error, invalid token document, or failed group publication leaves the previous complete artifact set in place.",
                code: None,
            },
        ],
    },
    DocPage {
        route: "/docs/tooling/repair/",
        title: "Plan first. Authorize exact bytes.",
        category: "Tooling",
        eyebrow: "Repair agent",
        summary: "Generate bounded repair proposals, bind authorization to source hashes, verify in staging, then publish receipts.",
        sections: &[
            DocSection {
                id: "plan",
                title: "Join findings to a proposal",
                body: "Repair plans name exact finding codes, files, byte ranges, replacements, expected hashes, and verification commands.",
                code: Some(
                    "pliego-cssc plan --findings findings.json \\\n  --proposal proposal.json --source-root .",
                ),
            },
            DocSection {
                id: "dry-run",
                title: "Verify before mutation",
                body: "Dry-run applies the authorized patch inside a staging copy and executes bounded Rust or browser checks without replacing the source tree.",
                code: Some(
                    "pliego-cssc fix --plan plan.json --findings findings.json \\\n  --source-root . --dry-run",
                ),
            },
            DocSection {
                id: "resource",
                title: "Time and output are part of safety",
                body: "Test, browser, toolchain, and version subprocesses use explicit deadlines and output limits while the repair lock is held.",
                code: None,
            },
        ],
    },
    DocPage {
        route: "/docs/integrations/plain-html/",
        title: "Audit and transform without a framework.",
        category: "Integrations",
        eyebrow: "Plain HTML",
        summary: "Use PliegoCSS as a standards control layer around an ordinary stylesheet and static HTML build.",
        sections: &[
            DocSection {
                id: "audit",
                title: "Start with emitted CSS",
                body: "No adapter is required. Audit the exact stylesheet the browser loads and archive canonical findings beside the build.",
                code: Some("pliego-cssc audit --input public/app.css --targets baseline-widely"),
            },
            DocSection {
                id: "transform",
                title: "Transform explicitly",
                body: "Compatibility transformation and minification are separate from analysis so a read-only audit never rewrites authored output.",
                code: Some(
                    "pliego-cssc transform-css --input src/app.css \\\n  --output public/app.css --targets modern --format minified",
                ),
            },
            DocSection {
                id: "typed",
                title: "Typed authoring is optional",
                body: "Rust applications can compile typed styles into the same static site, but plain HTML consumers only need the final CSS asset.",
                code: None,
            },
        ],
    },
    DocPage {
        route: "/docs/integrations/build-tools/",
        title: "Integrate through files, not hidden hooks.",
        category: "Integrations",
        eyebrow: "Generic build tools",
        summary: "Vite, webpack, esbuild, CI systems, and custom pipelines consume explicit CSS, manifests, findings, maps, and asset plans.",
        sections: &[
            DocSection {
                id: "producer",
                title: "Run PliegoCSS as a producer",
                body: "Invoke compile or transform before the application bundler and pass its ordinary CSS output through the normal asset graph.",
                code: Some(
                    "\"scripts\": {\n  \"css\": \"pliego-cssc compile --source src --output dist/pliego.css\",\n  \"build\": \"npm run css && vite build\"\n}",
                ),
            },
            DocSection {
                id: "consumer",
                title: "Consume stable sidecars",
                body: "Use the numbered JSON schemas for manifests, Token Graphs, findings, reachability, project indexes, and Asset Plans instead of scraping terminal text.",
                code: None,
            },
            DocSection {
                id: "ownership",
                title: "Keep topology in its owner",
                body: "A framework adapter may export route and component reachability. PliegoCSS validates and projects that sidecar but does not crawl application internals.",
                code: None,
            },
        ],
    },
    DocPage {
        route: "/docs/reference/configuration/",
        title: "Configuration is part of identity.",
        category: "Reference",
        eyebrow: "Configuration",
        summary: "Theme, target, format, manifests, reachability, pruning, and token selections are explicit, validated, and hash-bound.",
        sections: &[
            DocSection {
                id: "discovery",
                title: "Conventional TOML discovery only",
                body: "Without --config, --tokens, or --seed, the CLI searches for pliego.theme.toml from the workspace and input package roots. Ambiguous candidates fail.",
                code: None,
            },
            DocSection {
                id: "exclusive",
                title: "Selectors are mutually exclusive",
                body: "--config, --tokens, and --seed cannot be combined. JSON token documents are never discovered implicitly.",
                code: None,
            },
            DocSection {
                id: "hash",
                title: "Bind the complete choice",
                body: "Targets, format, emission settings, manifest version, pruning, theme bytes, resolver selections, and reachability bytes participate in the control configuration hash.",
                code: None,
            },
        ],
    },
    DocPage {
        route: "/docs/reference/diagnostics/",
        title: "Stable codes, precise ranges.",
        category: "Reference",
        eyebrow: "Diagnostics",
        summary: "Parser, source scanner, composition, policy, invocation, migration, and formatter failures share a canonical transport.",
        sections: &[
            DocSection {
                id: "human",
                title: "Human diagnostics",
                body: "Terminal output favors the next corrective action while retaining the stable code, severity, file, line, column, and byte range.",
                code: None,
            },
            DocSection {
                id: "json",
                title: "Canonical JSON",
                body: "JSON mode writes one schema-1 document to stderr on failure and leaves stdout empty. Consumers should branch on codes, not localized prose.",
                code: Some("pliego-cssc --diagnostic-format json check --style \"p-4 p-6\" --seed"),
            },
            DocSection {
                id: "sarif",
                title: "SARIF for code scanning",
                body: "Audit findings can be serialized as SARIF while preserving canonical rule identity and source regions for hosted review systems.",
                code: None,
            },
        ],
    },
];

pub fn all() -> Vec<Page> {
    let mut pages = Vec::new();
    for locale in [Locale::En, Locale::Es] {
        pages.extend(with_locale(locale, || {
            let mut localized = vec![
                page(
                    locale,
                    "/",
                    "PliegoCSS — Compile confidence into CSS",
                    home(),
                ),
                page(
                    locale,
                    "/playground/",
                    "Cascade Laboratory — PliegoCSS",
                    playground_page(),
                ),
                page(
                    locale,
                    "/examples/",
                    "Interactive examples — PliegoCSS",
                    examples_page(),
                ),
                page(locale, "/docs/", "Documentation — PliegoCSS", docs_index()),
                page(
                    locale,
                    "/docs/utilities/",
                    "Utility catalog — PliegoCSS",
                    utility_catalog_page(),
                ),
                page(
                    locale,
                    "/benchmarks/",
                    "Benchmarks — PliegoCSS",
                    benchmarks(),
                ),
                page(locale, "/brand/", "Brand — PliegoCSS", brand()),
                page(locale, "/changelog/", "Changelog — PliegoCSS", changelog()),
                page(locale, "/security/", "Security — PliegoCSS", security()),
                page(locale, "/legal/", "Legal register — PliegoCSS", legal_hub()),
                page(
                    locale,
                    "/accessibility/",
                    "Accessibility — PliegoCSS",
                    accessibility(),
                ),
                page(
                    locale,
                    "/404.html",
                    "Page not found — PliegoCSS",
                    not_found(),
                ),
            ];
            for slug in ["terms", "privacy", "cookies", "acceptable-use"] {
                localized.push(page(
                    locale,
                    &format!("/legal/{slug}/"),
                    legal_title(slug),
                    legal_document(slug),
                ));
            }
            localized.extend(
                DOCS.iter().map(|document| {
                    page(locale, document.route, document.title, doc_page(document))
                }),
            );
            localized
        }));
    }
    pages
}

fn page(locale: Locale, base_route: &str, title: &str, body: View) -> Page {
    let route = localize_route(locale, base_route);
    let title = translate(locale, title);
    let description = translate(
        locale,
        "Standards-first CSS analysis, typed Rust authoring, deterministic artifacts, and controlled migration.",
    );
    let canonical = if route == "/404.html" {
        "https://pliegocss.dev/".to_owned()
    } else if route == "/es/404.html" {
        "https://pliegocss.dev/es/".to_owned()
    } else {
        format!("https://pliegocss.dev{route}")
    };
    let english_url = if base_route == "/404.html" {
        "https://pliegocss.dev/".to_owned()
    } else {
        format!("https://pliegocss.dev{base_route}")
    };
    let spanish_url = if base_route == "/404.html" {
        "https://pliegocss.dev/es/".to_owned()
    } else {
        format!(
            "https://pliegocss.dev{}",
            localize_route(Locale::Es, base_route)
        )
    };
    let head = Head::new(&title)
        .description(&description)
        .canonical(canonical)
        .alternate("en", &english_url)
        .alternate("es", &spanish_url)
        .alternate("x-default", &english_url)
        .icon("/favicon.svg")
        .manifest("/site.webmanifest")
        .apple_touch_icon("/icon-512.png")
        .stylesheet("/assets/pliegocss.css")
        .stylesheet("/assets/laboratory.css")
        .stylesheet("/assets/site.css")
        .module_script("/assets/site.js")
        .meta("theme-color", "#070b14")
        .meta("generator", "PliegoRS 0.0.2")
        .property_meta("og:title", &title)
        .property_meta(
            "og:description",
            translate(locale, "Compile confidence into CSS."),
        )
        .property_meta("og:image", "https://pliegocss.dev/social-card.png")
        .property_meta("og:type", "website")
        .property_meta(
            "og:locale",
            if locale == Locale::En {
                "en_US"
            } else {
                "es_CO"
            },
        )
        .meta("twitter:card", "summary_large_image")
        .json_ld(json!({
            "@context": "https://schema.org",
            "@type": "SoftwareApplication",
            "name": "PliegoCSS",
            "applicationCategory": "DeveloperApplication",
            "operatingSystem": "Cross-platform",
            "license": "https://www.apache.org/licenses/LICENSE-2.0"
        }));
    Page::new(&route, head, shell(base_route, body))
        .language(locale.code())
        .source("src/pages.rs")
}

fn shell(route: &str, body: View) -> View {
    View::Fragment(vec![
        el("a")
            .class("skip-link")
            .attr("href", "#main")
            .child("Skip to content")
            .into_view(),
        site_header(route),
        el("main").id("main").child(body).into_view(),
        site_footer(),
        command_palette(),
    ])
}

fn command_palette() -> View {
    el("dialog")
        .class("command-palette")
        .attr("data-command-palette", "")
        .attr("aria-labelledby", "command-title")
        .child(
            el("div")
                .class("command-search-row")
                .child(
                    el("span")
                        .class("command-mark")
                        .attr("aria-hidden", "true")
                        .child("⌕"),
                )
                .child(
                    el("label")
                        .attr("for", "command-search")
                        .child(
                            el("span")
                                .id("command-title")
                                .class("sr-only")
                                .child("Search PliegoCSS"),
                        )
                        .child(
                            el("input")
                                .id("command-search")
                                .attr("type", "search")
                                .attr("autocomplete", "off")
                                .attr("placeholder", "Search docs, commands, utilities, and labs…")
                                .attr("data-command-search", ""),
                        ),
                )
                .child(
                    el("button")
                        .attr("type", "button")
                        .attr("data-command-close", "")
                        .attr("aria-label", "Close search")
                        .child("ESC"),
                ),
        )
        .child(
            el("div")
                .class("command-results")
                .attr("data-command-results", "")
                .child(
                    el("p")
                        .class("command-loading")
                        .child("Loading search index…"),
                ),
        )
        .child(
            el("p")
                .class("command-hint")
                .child("↑↓ navigate · enter open · esc close"),
        )
        .into_view()
}

fn site_header(route: &str) -> View {
    let locale = current_locale();
    let items = [
        ("Docs", "/docs/"),
        ("Laboratory", "/playground/"),
        ("Examples", "/examples/"),
        ("Benchmarks", "/benchmarks/"),
    ];
    let mut nav = el("nav")
        .class("nav-links")
        .attr("aria-label", "Primary navigation");
    for (label, href) in items {
        let active = route == href || (href != "/" && route.starts_with(href));
        let mut link = el("a").attr("href", href).child(label);
        if active {
            link = link.attr("aria-current", "page");
        }
        nav = nav.child(link);
    }
    el("div")
        .class("site-masthead")
        .child(
            el("header")
                .class("site-header")
                .child(
                    el("a")
                        .class("brand-link")
                        .attr("href", "/")
                        .attr("aria-label", "PliegoCSS home")
                        .child(
                            el("img")
                                .attr("src", "/brand/pliegocss-symbol-reversed.svg")
                                .attr("width", "32")
                                .attr("height", "32")
                                .attr("alt", ""),
                        )
                        .child(el("span").child("PliegoCSS")),
                )
                .child(nav)
                .child(
                    el("nav")
                        .class("language-switcher")
                        .attr("aria-label", "Language")
                        .child(
                            el("a")
                                .attr_raw("href", route)
                                .attr("lang", "en")
                                .attr(
                                    "aria-current",
                                    if locale == Locale::En {
                                        "true"
                                    } else {
                                        "false"
                                    },
                                )
                                .child("EN"),
                        )
                        .child(
                            el("a")
                                .attr_raw("href", localize_route(Locale::Es, route))
                                .attr("lang", "es")
                                .attr(
                                    "aria-current",
                                    if locale == Locale::Es {
                                        "true"
                                    } else {
                                        "false"
                                    },
                                )
                                .child("ES"),
                        ),
                )
                .child(
                    el("button")
                        .class("header-search")
                        .attr("type", "button")
                        .attr("data-command-open", "")
                        .attr("aria-label", "Search PliegoCSS")
                        .child(el("span").child("Search"))
                        .child(el("kbd").child("⌘ K")),
                ),
        )
        .child(
            el("div")
                .class("preview-rail")
                .attr("aria-label", "Public preview status")
                .child(el("span").child("RUST-NATIVE CSS TOOLCHAIN"))
                .child(el("strong").child("PUBLIC PREVIEW / MEDELLÍN / 2026")),
        )
        .into_view()
}

fn site_footer() -> View {
    el("footer")
        .class("site-footer")
        .child(
            el("div")
                .class("footer-identity")
                .child(
                    el("p")
                        .class("footer-statement")
                        .child("Visible utilities. Evidence underneath."),
                )
                .child(el("p").class("footer-mark").child("PLIEGOCSS.DEV"))
                .child(el("p").child("PliegoCSS is an independent Celiums Solutions LLC project."))
                .child(
                    el("a")
                        .attr("href", "mailto:hello@pliegocss.dev")
                        .child("hello@pliegocss.dev ↗"),
                ),
        )
        .child(
            el("div")
                .class("footer-directory")
                .child(
                    el("nav")
                        .attr("aria-label", "Framework")
                        .child(el("strong").child("Framework"))
                        .child(el("a").attr("href", "/docs/").child("Documentation"))
                        .child(
                            el("a")
                                .attr("href", "/docs/getting-started/")
                                .child("Getting started"),
                        )
                        .child(
                            el("a")
                                .attr("href", "/docs/utilities/")
                                .child("Utility catalog"),
                        )
                        .child(el("a").attr("href", "/playground/").child("Laboratory"))
                        .child(el("a").attr("href", "/examples/").child("Examples")),
                )
                .child(
                    el("nav")
                        .attr("aria-label", "Project")
                        .child(el("strong").child("Project"))
                        .child(el("a").attr("href", "/brand/").child("Brand"))
                        .child(el("a").attr("href", "/benchmarks/").child("Benchmarks"))
                        .child(el("a").attr("href", "/changelog/").child("Changelog"))
                        .child(el("a").attr("href", "/security/").child("Security"))
                        .child(
                            el("a")
                                .attr("href", "/accessibility/")
                                .child("Accessibility"),
                        ),
                )
                .child(
                    el("nav")
                        .attr("aria-label", "Legal")
                        .child(el("strong").child("Legal"))
                        .child(el("a").attr("href", "/legal/").child("Legal register"))
                        .child(el("a").attr("href", "/legal/terms/").child("Terms"))
                        .child(el("a").attr("href", "/legal/privacy/").child("Privacy"))
                        .child(el("a").attr("href", "/legal/cookies/").child("Cookies"))
                        .child(
                            el("a")
                                .attr("href", "/legal/acceptable-use/")
                                .child("Acceptable use"),
                        ),
                )
                .child(
                    el("nav")
                        .attr("aria-label", "External")
                        .child(el("strong").child("External"))
                        .child(
                            el("a")
                                .attr("href", "https://github.com/celiumsai/pliegocss")
                                .child("GitHub ↗"),
                        )
                        .child(
                            el("a")
                                .attr("href", "mailto:hello@pliegocss.dev")
                                .child("Contact ↗"),
                        ),
                ),
        )
        .child(
            el("div")
                .class("footer-bottom")
                .child(el("span").child("© 2026 Celiums Solutions LLC"))
                .child(el("span").child("Made in Medellín · Worldwide"))
                .child(el("span").child("Open source · Apache-2.0")),
        )
        .into_view()
}

fn home() -> View {
    let hero_class = format!("hero {}", HERO_STYLE.class_name());
    el("div")
        .class("home")
        .child(
            el("section")
                .class(hero_class)
                .attr("data-hero", "")
                .child(
                    el("p")
                        .class("hero-preview-label")
                        .child("PLIEGOCSS / 0.1.0-RC.1 / PUBLIC PREVIEW"),
                )
                .child(brand_picture(
                    "cascade-chamber",
                    "A dark architectural cascade chamber where authored planes become one compiled rail",
                    "hero-art",
                ))
                .child(el("canvas").class("hero-canvas").attr("data-hero-canvas", "").attr("aria-hidden", "true"))
                .child(el("div").class("hero-grid").attr("aria-hidden", "true"))
                .child(
                    el("div")
                        .class("hero-copy")
                        .child(kicker("THE CSS TOOLCHAIN THAT CAN EXPLAIN ITSELF"))
                        .child(
                            el("h1")
                                .child(el("span").child("CSS you can"))
                                .child(" ")
                                .child(el("span").class("hero-accent").child("prove.")),
                        )
                        .child(
                            el("p")
                                .class("hero-lede")
                                .child("Author visible utilities in Rust. Ship ordinary static CSS. Trace every identity, diagnostic, and physical declaration back to intent."),
                        )
                        .child(
                            el("div")
                                .class("hero-actions")
                                .child(action_link("Open the laboratory", "/playground/"))
                                .child(
                                    el("a")
                                        .class("text-link")
                                        .attr("href", "/docs/getting-started/")
                                        .child("Read the docs ↗"),
                                ),
                        ),
                )
                .child(
                    el("aside")
                        .class("hero-terminal")
                        .attr("data-reveal", "")
                        .child(
                            el("div")
                                .class("terminal-bar")
                                .child(el("span").child("LIVE / BUILD-TIME CORPUS"))
                                .child(el("span").class("terminal-state").child("VERIFIED")),
                        )
                        .child(
                            el("div")
                                .class("terminal-input")
                                .child(el("span").child("pc!"))
                                .child(
                                    el("code")
                                        .attr("data-hero-recipe", "")
                                        .child("\"rounded-lg ring-2! shadow-md!\""),
                                ),
                        )
                        .child(
                            el("div")
                                .class("terminal-fold")
                                .attr("aria-hidden", "true")
                                .child(el("span"))
                                .child(el("span"))
                                .child(el("span")),
                        )
                        .child(
                            el("dl")
                                .class("terminal-proof")
                                .child(metric("Identity", "128-bit stable"))
                                .child(metric("Output", "static CSS"))
                                .child(metric("Runtime", "0 styling bytes")),
                        )
                        .child(
                            el("button")
                                .class("terminal-cycle")
                                .attr("type", "button")
                                .attr("data-hero-cycle", "")
                                .child("Cycle compiler proof"),
                        ),
                ),
        )
        .child(proof_marquee())
        .child(product_theatre())
        .child(conflict_lab())
        .child(cascade_story())
        .child(catalog_teaser())
        .child(evidence())
        .child(
            closing(
                "Make CSS observable.",
                "Use the real laboratory, inspect the full catalog, then adopt only the surfaces your project needs.",
                "Enter the playground",
                "/playground/",
            ),
        )
        .into_view()
}

fn proof_marquee() -> Element {
    el("section")
        .class("proof-marquee")
        .attr("aria-label", "Product contract")
        .child(
            el("div")
                .child(el("span").child("STATIC CSS"))
                .child(el("span").child("60 UTILITY FORMS"))
                .child(el("span").child("51 THEME TOKENS"))
                .child(el("span").child("3 BREAKPOINTS"))
                .child(el("span").child("128-BIT STYLE ID"))
                .child(el("span").child("ZERO STYLING RUNTIME")),
        )
}

fn product_theatre() -> Element {
    el("section")
        .id("theatre")
        .class("product-theatre")
        .attr("data-theatre", "")
        .attr("aria-labelledby", "theatre-title")
        .child(
            el("div")
                .class("theatre-head section-shell")
                .child(kicker("DIRECT MANIPULATION / EXACT OUTPUT"))
                .child(
                    el("h2")
                        .id("theatre-title")
                        .child("Design the surface.")
                        .child(el("span").child("Inspect the consequence.")),
                )
                .child(
                    el("p").child(
                        "Change the component, viewport, and state. Every selectable recipe was compiled by this repository’s PliegoCSS binary before the page was built.",
                    ),
                ),
        )
        .child(
            el("div")
                .class("theatre-workbench")
                .child(
                    el("aside")
                        .class("theatre-controls")
                        .child(lab_control(
                            "layout",
                            "Layout",
                            &[("stack", "Stack"), ("split", "Split")],
                            "stack",
                        ))
                        .child(lab_control(
                            "gap",
                            "Gap",
                            &[
                                ("tight", "Tight"),
                                ("balanced", "Balanced"),
                                ("open", "Open"),
                            ],
                            "balanced",
                        ))
                        .child(lab_control(
                            "padding",
                            "Padding",
                            &[
                                ("compact", "Compact"),
                                ("roomy", "Roomy"),
                                ("editorial", "Editorial"),
                            ],
                            "roomy",
                        ))
                        .child(lab_control(
                            "radius",
                            "Radius",
                            &[("precise", "Precise"), ("soft", "Soft")],
                            "soft",
                        ))
                        .child(lab_control(
                            "depth",
                            "Depth",
                            &[("quiet", "Quiet"), ("raised", "Raised")],
                            "raised",
                        ))
                        .child(lab_control(
                            "tone",
                            "Tone",
                            &[("paper", "Paper"), ("compiled", "Compiled")],
                            "paper",
                        ))
                        .child(
                            el("button")
                                .class("lab-reset")
                                .attr("type", "button")
                                .attr("data-lab-reset", "")
                                .child("Reset exact recipe"),
                        ),
                )
                .child(
                    el("div")
                        .class("theatre-stage-wrap")
                        .child(
                            el("div")
                                .class("viewport-switcher")
                                .attr("aria-label", "Preview width")
                                .child(viewport_button("phone", "390", false))
                                .child(viewport_button("tablet", "768", false))
                                .child(viewport_button("desktop", "1280", true)),
                        )
                        .child(
                            el("div")
                                .class("theatre-stage")
                                .attr("data-stage-width", "desktop")
                                .child(
                                    el("article")
                                        .class("preview-card")
                                        .attr("data-lab-preview", "")
                                        .child(
                                            el("div")
                                                .class("preview-visual")
                                                .child(
                                                    el("div")
                                                        .class("preview-fold-mark")
                                                        .attr("aria-hidden", "true"),
                                                )
                                                .child(
                                                    el("p")
                                                        .class("preview-overline")
                                                        .child("STYLE / B312…"),
                                                )
                                                .child(el("h3").child("One visible intent."))
                                                .child(
                                                    el("p").child(
                                                        "Stable identity, ordinary output, replayable evidence.",
                                                    ),
                                                ),
                                        )
                                        .child(
                                            el("div")
                                                .class("preview-meta")
                                                .child(el("span").child("manifest schema 3"))
                                                .child(el("span").child("modern target")),
                                        ),
                                ),
                        ),
                )
                .child(
                    el("aside")
                        .class("theatre-output")
                        .child(
                            el("div")
                                .class("output-tabs")
                                .attr("role", "tablist")
                                .attr("aria-label", "Compiler output")
                                .child(output_tab("intent", "Intent", true))
                                .child(output_tab("css", "CSS", false))
                                .child(output_tab("proof", "Proof", false)),
                        )
                        .child(
                            el("div")
                                .class("output-panel")
                                .id("output-panel-intent")
                                .attr("role", "tabpanel")
                                .attr("aria-labelledby", "output-tab-intent")
                                .attr("tabindex", "0")
                                .attr("data-lab-panel", "intent")
                                .child(
                                    el("pre")
                                        .child(
                                            el("code")
                                                .attr("data-lab-input", "")
                                                .child("Loading compiler corpus…"),
                                        ),
                                ),
                        )
                        .child(
                            el("div")
                                .class("output-panel")
                                .id("output-panel-css")
                                .attr("role", "tabpanel")
                                .attr("aria-labelledby", "output-tab-css")
                                .attr("tabindex", "0")
                                .attr("data-lab-panel", "css")
                                .attr("hidden", "")
                                .child(
                                    el("pre")
                                        .child(
                                            el("code")
                                                .attr("data-lab-css", "")
                                                .child("Loading emitted CSS…"),
                                        ),
                                ),
                        )
                        .child(
                            el("div")
                                .class("output-panel proof-panel")
                                .id("output-panel-proof")
                                .attr("role", "tabpanel")
                                .attr("aria-labelledby", "output-tab-proof")
                                .attr("tabindex", "0")
                                .attr("data-lab-panel", "proof")
                                .attr("hidden", "")
                                .child(
                                    el("dl")
                                        .child(metric("Class", "—"))
                                        .child(metric("Style ID", "—"))
                                        .child(metric("Corpus hash", "—")),
                                ),
                        )
                        .child(
                            el("p")
                                .class("corpus-note")
                                .child("Bounded build-time corpus. No JavaScript compiler imitation."),
                        ),
                ),
        )
}

fn conflict_lab() -> Element {
    el("section")
        .class("conflict-lab")
        .attr("data-conflict-lab", "")
        .attr("aria-labelledby", "conflict-title")
        .child(
            el("div")
                .class("conflict-copy")
                .child(kicker("FAIL CLOSED / BEFORE THE BROWSER"))
                .child(
                    el("h2")
                        .id("conflict-title")
                        .child("The cascade should not be a crime scene."),
                )
                .child(
                    el("p").child(
                        "Place two values in the same semantic slot and PliegoCSS refuses to guess. Move one value into a narrower condition and the state space becomes explicit.",
                    ),
                )
                .child(
                    el("div")
                        .class("conflict-switches")
                        .attr("role", "tablist")
                        .attr("aria-label", "Conflict scenarios")
                        .child(conflict_button("padding", "p-4 + p-6", true))
                        .child(conflict_button(
                            "condition-safe",
                            "p-4 + md:p-6",
                            false,
                        ))
                        .child(conflict_button(
                            "type-size",
                            "text-sm + text-lg",
                            false,
                        )),
                ),
        )
        .child(
            el("div")
                .class("diagnostic-console")
                .id("conflict-diagnostic")
                .attr("role", "region")
                .attr("aria-live", "polite")
                .attr("aria-label", "Compiler diagnostic")
                .child(
                    el("div")
                        .class("diagnostic-status")
                        .child(
                            el("span")
                                .attr("data-conflict-status", "")
                                .child("REJECTED"),
                        )
                        .child(el("span").attr("data-conflict-code", "").child("PCS005")),
                )
                .child(
                    el("pre")
                        .child(
                            el("code")
                                .attr("data-conflict-input", "")
                                .child("p-4 p-6"),
                        ),
                )
                .child(
                    el("p")
                        .class("diagnostic-message")
                        .attr("data-conflict-message", "")
                        .child("Utilities assign conflicting values in the same condition."),
                )
                .child(
                    el("p")
                        .class("diagnostic-suggestion")
                        .attr("data-conflict-suggestion", "")
                        .child("Remove one utility or move it to a narrower variant."),
                ),
        )
}

fn catalog_teaser() -> Element {
    el("section")
        .class("catalog-teaser section-shell")
        .attr("aria-labelledby", "catalog-title")
        .child(
            el("div")
                .class("catalog-number")
                .child(el("span").child("60"))
                .child(el("small").child("GENERATED UTILITY FORMS")),
        )
        .child(
            el("div")
                .class("catalog-copy")
                .child(kicker("ONE CATALOG / EVERY TOOL"))
                .child(
                    el("h2")
                        .id("catalog-title")
                        .child("Parser. Completion. Diagnostics. Reference."),
                )
                .child(
                    el("p").child(
                        "The same generated catalog drives compiler semantics and the searchable website reference. No hand-maintained marketing inventory.",
                    ),
                )
                .child(action_link("Explore all utility forms", "/docs/utilities/")),
        )
        .child(
            el("div")
                .class("catalog-stream")
                .attr("aria-hidden", "true")
                .child(el("span").child("grid-cols-{value}"))
                .child(el("span").child("ring-{width-or-color}"))
                .child(el("span").child("bg-{color}"))
                .child(el("span").child("[property:value]"))
                .child(el("span").child("writing-vertical-rl")),
        )
}

fn cascade_story() -> Element {
    el("section")
        .class("cascade-story")
        .attr("data-cascade-story", "")
        .child(
            el("div")
                .class("cascade-copy")
                .child(kicker("ONE SEMANTIC FOLD"))
                .child(el("h2").child("Source becomes proof, not mystery."))
                .child(el("p").child("A style starts as visible intent, lowers into typed slots, resolves conflicts, emits CSS, and remains traceable to physical declarations."))
                .child(
                    el("ol")
                        .class("cascade-steps")
                        .child(step("01", "Parse", "Visible utilities and standard CSS"))
                        .child(step("02", "Lower", "Typed semantic slots and conditions"))
                        .child(step("03", "Resolve", "Explicit conflict and precedence rules"))
                        .child(step("04", "Emit", "Static CSS plus canonical evidence")),
                ),
        )
        .child(
            el("div")
                .class("cascade-visual")
                .attr("aria-label", "Animated compiler pipeline")
                .child(el("div").class("rail rail-source").attr("aria-hidden", "true"))
                .child(el("div").class("rail rail-output").attr("aria-hidden", "true"))
                .child(
                    el("pre")
                        .class("cascade-code")
                        .child(el("code").child("pc!(\"grid gap-4\\nrounded-xl shadow-md\")\n\n→ pc_1k3…\n→ .pc_1k3… { display:grid; … }\n→ manifest + lineage")),
                ),
        )
}

fn playground_page() -> View {
    el("div")
        .class("laboratory-page")
        .child(
            el("section")
                .class("lab-hero")
                .child(kicker("CASCADE LABORATORY / BUILD-TIME VERIFIED"))
                .child(
                    el("h1")
                        .child("Touch every")
                        .child(" ")
                        .child(el("span").child("consequence.")),
                )
                .child(
                    el("p").child(
                        "Compose a bounded recipe, inspect exact compiler output, search the generated catalog, and watch PliegoCSS reject semantic ambiguity before CSS reaches the browser.",
                    ),
                )
                .child(
                    el("div")
                        .class("lab-hero-facts")
                        .child(metric("Recipes", "144 exact combinations"))
                        .child(metric("Compiler", "pliego-cssc 0.1.0-rc.1"))
                        .child(metric("Boundary", "build-time corpus")),
                ),
        )
        .child(product_theatre())
        .child(explanation_explorer())
        .child(conflict_lab())
        .child(catalog_explorer())
        .child(
            closing(
                "Take the laboratory into your repository.",
                "The online corpus is deliberately bounded. Local PliegoCSS compiles your actual Rust source, CSS, tokens, and migration inventory.",
                "Install and audit",
                "/docs/getting-started/",
            ),
        )
        .into_view()
}

fn examples_page() -> View {
    el("div")
        .class("examples-page")
        .child(
            el("section")
                .class("examples-hero")
                .child(kicker("INTERACTIVE EXAMPLES / ORDINARY STATIC CSS"))
                .child(
                    el("h1")
                        .child("Build the")
                        .child(" ")
                        .child(el("span").child("impossible to ignore.")),
                )
                .child(
                    el("p").child(
                        "Three product surfaces. One generated PliegoCSS class graph. Change the mode, interact with the component, and inspect the exact authored recipe and emitted selector.",
                    ),
                )
                .child(
                    el("nav")
                        .class("example-switcher")
                        .attr("aria-label", "Select example")
                        .child(example_button("commerce", "01", "Commerce", true))
                        .child(example_button("operations", "02", "Operations", false))
                        .child(example_button("editorial", "03", "Editorial", false)),
                ),
        )
        .child(
            el("section")
                .class("example-stage-shell")
                .attr("data-example-lab", "")
                .child(example_commerce())
                .child(example_operations())
                .child(example_editorial())
                .child(
                    el("aside")
                        .class("example-inspector")
                        .child(
                            el("div")
                        .class("example-inspector-head")
                        .child(el("span").child("LIVE INSPECTOR"))
                                .child(
                                    el("span")
                                        .attr("data-example-state", "")
                                        .attr("aria-live", "polite")
                                        .child("commerce / default"),
                                ),
                        )
                        .child(
                            el("div")
                                .class("example-inspector-block")
                                .child(el("span").child("AUTHORED RECIPE"))
                                .child(
                                    el("pre").child(
                                        el("code")
                                            .attr("data-example-recipe", "")
                                            .child("Loading compiler corpus…"),
                                    ),
                                ),
                        )
                        .child(
                            el("div")
                                .class("example-inspector-block")
                                .child(el("span").child("GENERATED IDENTITY"))
                                .child(
                                    el("dl")
                                        .child(metric("Class", "—"))
                                        .child(metric("Style ID", "—"))
                                        .child(metric("Corpus", "—")),
                                ),
                        )
                        .child(
                            el("div")
                                .class("example-inspector-block")
                                .child(el("span").child("EMITTED SELECTOR"))
                                .child(
                                    el("pre").child(
                                        el("code").attr("data-example-css", "").child("Loading…"),
                                    ),
                                ),
                        ),
                ),
        )
        .child(
            el("section")
                .class("examples-contract section-shell")
                .child(kicker("WHAT THESE DEMOS PROVE"))
                .child(
                    el("div")
                        .class("examples-contract-grid")
                        .child(
                            el("article")
                                .child(el("span").child("01"))
                                .child(el("h2").child("The interaction is real."))
                                .child(el("p").child("Buttons update product state, not a decorative animation loop. Every example remains usable with a keyboard and reduced motion.")),
                        )
                        .child(
                            el("article")
                                .child(el("span").child("02"))
                                .child(el("h2").child("The CSS is real."))
                                .child(el("p").child("The inspector resolves each example state to an exact class generated by the repository’s compiler during the site build.")),
                        )
                        .child(
                            el("article")
                                .child(el("span").child("03"))
                                .child(el("h2").child("The boundary is honest."))
                                .child(el("p").child("The browser chooses from a bounded, hash-bound corpus. It does not ship or impersonate the PliegoCSS compiler.")),
                        ),
                ),
        )
        .child(
            closing(
                "Turn the proof into your interface.",
                "Start with the laboratory, then compile the actual source, tokens, policies, and topology in your repository.",
                "Open the laboratory",
                "/playground/",
            ),
        )
        .into_view()
}

fn example_button(id: &str, index: &str, label: &str, selected: bool) -> Element {
    el("button")
        .attr("type", "button")
        .attr("data-example-select", id)
        .attr("aria-pressed", if selected { "true" } else { "false" })
        .child(el("span").child(index))
        .child(label)
}

fn example_commerce() -> Element {
    el("article")
        .class("example-panel example-commerce is-active")
        .attr("data-example-panel", "commerce")
        .child(
            el("div")
                .class("commerce-gallery")
                .child(
                    el("div")
                        .class("commerce-object")
                        .attr("aria-hidden", "true")
                        .child(el("span").class("object-plane plane-a"))
                        .child(el("span").class("object-plane plane-b"))
                        .child(el("span").class("object-spine"))
                        .child(el("span").class("object-signal")),
                )
                .child(
                    el("div")
                        .class("commerce-thumbs")
                        .child(example_thumb("front", true))
                        .child(example_thumb("fold", false))
                        .child(example_thumb("detail", false)),
                ),
        )
        .child(
            el("div")
                .class("commerce-copy")
                .child(el("p").class("example-overline").child("PLIEGO OBJECT / SERIES 01"))
                .child(el("h2").child("The Cascade Folio"))
                .child(el("p").child("A numbered study in folds, rails, and deterministic blue. Built as a product interaction rather than a static hero card."))
                .child(el("p").class("commerce-price").child("$128"))
                .child(
                    el("div")
                        .class("commerce-variants")
                        .child(example_choice("carbon", "Carbon", true))
                        .child(example_choice("cobalt", "Cobalt", false))
                        .child(example_choice("paper", "Paper", false)),
                )
                .child(
                    el("button")
                        .class("commerce-add")
                        .attr("type", "button")
                        .attr("data-commerce-add", "")
                        .child("Add folio"),
                )
                .child(
                    el("p")
                        .class("commerce-status")
                        .attr("data-commerce-status", "")
                        .attr("aria-live", "polite")
                        .child("Ready to compile the order."),
                ),
        )
}

fn example_thumb(id: &str, selected: bool) -> Element {
    el("button")
        .attr("type", "button")
        .attr("data-commerce-view", id)
        .attr("aria-pressed", if selected { "true" } else { "false" })
        .child(id)
}

fn example_choice(id: &str, label: &str, selected: bool) -> Element {
    el("button")
        .attr("type", "button")
        .attr("data-commerce-variant", id)
        .attr("aria-pressed", if selected { "true" } else { "false" })
        .child(label)
}

fn example_operations() -> Element {
    el("article")
        .class("example-panel example-operations")
        .attr("data-example-panel", "operations")
        .attr("hidden", "")
        .child(
            el("aside")
                .class("ops-rail")
                .attr("aria-label", "Operations view")
                .child(el("span").class("ops-mark").child("P/"))
                .child(ops_view_button("overview", "Overview", true))
                .child(ops_view_button("builds", "Builds", false))
                .child(ops_view_button("evidence", "Evidence", false))
                .child(ops_view_button("policies", "Policies", false)),
        )
        .child(
            el("div")
                .class("ops-canvas")
                .child(
                    el("header")
                        .child(
                            el("div")
                                .child(
                                    el("p")
                                        .class("example-overline")
                                        .child("CONTROL ROOM / PROJECT PLIEGO"),
                                )
                                .child(
                                    el("h2").attr("data-ops-title", "").child("Build authority"),
                                ),
                        )
                        .child(
                            el("button")
                                .attr("type", "button")
                                .attr("data-ops-pulse", "")
                                .child("Replay evidence"),
                        ),
                )
                .child(
                    el("div")
                        .class("ops-metrics")
                        .child(ops_metric("CSS", "64.8", "KiB corpus"))
                        .child(ops_metric("Styles", "144", "reachable"))
                        .child(ops_metric("Drift", "0", "bytes"))
                        .child(ops_metric("Gate", "PASS", "local")),
                )
                .child(
                    el("div")
                        .class("ops-grid")
                        .child(
                            el("section")
                                .class("ops-trace")
                                .child(
                                    el("p")
                                        .attr("data-ops-trace-label", "")
                                        .child("SEVEN-DAY ARTIFACT TRACE"),
                                )
                                .child(
                                    el("div")
                                        .class("ops-bars")
                                        .attr("aria-label", "Seven deterministic build sizes")
                                        .child(el("i").attr("style", "--bar:62%"))
                                        .child(el("i").attr("style", "--bar:71%"))
                                        .child(el("i").attr("style", "--bar:66%"))
                                        .child(el("i").attr("style", "--bar:82%"))
                                        .child(el("i").attr("style", "--bar:77%"))
                                        .child(el("i").attr("style", "--bar:91%"))
                                        .child(el("i").attr("style", "--bar:91%")),
                                ),
                        )
                        .child(
                            el("section")
                                .class("ops-feed")
                                .child(
                                    el("p")
                                        .attr("data-ops-feed-label", "")
                                        .child("LATEST EVIDENCE"),
                                )
                                .child(ops_event("14:32:08", "manifest", "hash matched"))
                                .child(ops_event("14:32:06", "browser", "route replayed"))
                                .child(ops_event("14:31:59", "policy", "0 findings")),
                        ),
                )
                .child(
                    el("p")
                        .class("sr-only")
                        .attr("data-ops-announcer", "")
                        .attr("aria-live", "polite")
                        .child("Overview selected."),
                ),
        )
}

fn ops_view_button(id: &str, label: &str, selected: bool) -> Element {
    let mut button = el("button")
        .attr("type", "button")
        .attr("data-ops-view", id)
        .attr("aria-pressed", if selected { "true" } else { "false" })
        .child(label);
    if selected {
        button = button.class("is-active");
    }
    button
}

fn ops_metric(term: &str, value: &str, detail: &str) -> Element {
    el("div")
        .child(el("span").child(term))
        .child(el("strong").child(value))
        .child(el("small").child(detail))
}

fn ops_event(time: &str, kind: &str, outcome: &str) -> Element {
    el("div")
        .child(el("time").child(time))
        .child(el("strong").child(kind))
        .child(el("span").child(outcome))
}

fn example_editorial() -> Element {
    el("article")
        .class("example-panel example-editorial")
        .attr("data-example-panel", "editorial")
        .attr("hidden", "")
        .child(
            el("div")
                .class("editorial-cover")
                .attr("aria-hidden", "true")
                .child(el("span").class("editorial-number").child("01"))
                .child(el("span").class("editorial-rail rail-one"))
                .child(el("span").class("editorial-rail rail-two"))
                .child(el("span").class("editorial-fold")),
        )
        .child(
            el("div")
                .class("editorial-copy")
                .child(el("p").class("example-overline").child("FIELD NOTE / CASCADE SYSTEMS"))
                .child(el("h2").child("Interfaces should remember why."))
                .child(el("p").class("editorial-deck").child("A technical essay about semantic identity, ordinary CSS, and the evidence required to keep refactoring authority intact."))
                .child(
                    el("div")
                        .class("editorial-meta")
                        .child(el("span").child("12 MIN READ"))
                        .child(el("span").child("ISSUE 01"))
                        .child(el("span").child("JUL 2026")),
                )
                .child(
                    el("button")
                        .class("editorial-read")
                        .attr("type", "button")
                        .attr("data-editorial-expand", "")
                        .attr("aria-expanded", "false")
                        .child("Open the field note"),
                )
                .child(
                    el("div")
                        .class("editorial-excerpt")
                        .attr("data-editorial-excerpt", "")
                        .attr("hidden", "")
                        .child(el("p").child("A utility string is not the product. The product is the chain that lets a person move from authored intent to a physical declaration and back without folklore.")),
                ),
        )
}

fn explanation_explorer() -> Element {
    el("section")
        .class("explanation-explorer")
        .attr("data-explanation", "")
        .attr("aria-labelledby", "explanation-title")
        .child(
            el("div")
                .class("explanation-heading section-shell")
                .child(kicker("SEMANTIC INTENT → PHYSICAL EFFECT"))
                .child(
                    el("h2")
                        .id("explanation-title")
                        .child("Trace the fold, utility by utility."),
                )
                .child(
                    el("div")
                        .class("explanation-tabs")
                        .attr("role", "tablist")
                        .child(explanation_button("composed-effect", "Ring + shadow", true))
                        .child(explanation_button(
                            "responsive-grid",
                            "Responsive grid",
                            false,
                        ))
                        .child(explanation_button("action", "Action state", false)),
                ),
        )
        .child(
            el("div")
                .class("explanation-stage")
                .child(
                    el("div")
                        .class("explanation-source")
                        .child(el("span").child("AUTHORED"))
                        .child(
                            el("pre").child(
                                el("code")
                                    .attr("data-explanation-source", "")
                                    .child("Loading…"),
                            ),
                        ),
                )
                .child(
                    el("div")
                        .class("explanation-fold")
                        .attr("aria-hidden", "true")
                        .child(el("canvas").attr("data-layer-canvas", "")),
                )
                .child(
                    el("div")
                        .class("explanation-output")
                        .child(el("span").child("EMITTED"))
                        .child(
                            el("pre").child(
                                el("code")
                                    .attr("data-explanation-css", "")
                                    .child("Loading…"),
                            ),
                        ),
                ),
        )
        .child(
            el("ol")
                .class("utility-lineage")
                .attr("data-explanation-utilities", "")
                .child(el("li").child("Loading utility lineage…")),
        )
}

fn explanation_button(id: &str, label: &str, selected: bool) -> Element {
    el("button")
        .attr("type", "button")
        .attr("role", "tab")
        .attr("data-explanation-id", id)
        .attr("aria-selected", if selected { "true" } else { "false" })
        .child(label)
}

fn catalog_explorer() -> Element {
    el("section")
        .class("catalog-explorer section-shell")
        .attr("data-catalog-explorer", "")
        .attr("aria-labelledby", "explorer-title")
        .child(
            el("div")
                .class("catalog-explorer-head")
                .child(
                    el("div").child(kicker("GENERATED REFERENCE")).child(
                        el("h2")
                            .id("explorer-title")
                            .child("Find the exact utility surface."),
                    ),
                )
                .child(
                    el("label")
                        .class("catalog-search")
                        .child(el("span").class("sr-only").child("Search utilities"))
                        .child(
                            el("input")
                                .attr("type", "search")
                                .attr("placeholder", "Search pattern, domain, or behavior…")
                                .attr("data-catalog-search", ""),
                        ),
                ),
        )
        .child(
            el("div")
                .class("catalog-filters")
                .attr("data-catalog-filters", "")
                .child(catalog_filter("all", "All", true))
                .child(catalog_filter("fixed", "Fixed", false))
                .child(catalog_filter("parameterized", "Parameterized", false))
                .child(catalog_filter("arbitrary-property", "Arbitrary", false)),
        )
        .child(
            el("div")
                .class("catalog-results")
                .attr("data-catalog-results", "")
                .child(
                    el("p")
                        .class("catalog-loading")
                        .child("Loading generated catalog…"),
                ),
        )
}

fn catalog_filter(id: &str, label: &str, selected: bool) -> Element {
    el("button")
        .attr("type", "button")
        .attr("data-catalog-filter", id)
        .attr("aria-pressed", if selected { "true" } else { "false" })
        .child(label)
}

fn utility_catalog_page() -> View {
    el("div")
        .class("utility-reference page-shell")
        .child(page_intro(
            "UTILITY REFERENCE / GENERATED",
            "One source of truth for every utility form.",
            "This catalog is emitted from the same Rust definitions used by parsing, lowering, completions, diagnostics, examples, and documentation.",
        ))
        .child(
            el("div")
                .class("catalog-reference-facts")
                .child(metric("Utility forms", "60"))
                .child(metric("Theme tokens", "51"))
                .child(metric("Breakpoints", "3"))
                .child(metric("Schema", "3")),
        )
        .child(catalog_explorer())
        .into_view()
}

fn evidence() -> Element {
    let panel_class = format!("evidence-card {}", PANEL_STYLE.class_name());
    el("section")
        .class("evidence-section section-shell")
        .attr("aria-labelledby", "evidence-title")
        .child(
            el("div")
                .class("section-heading")
                .child(kicker("NO UNLABELED GREEN"))
                .child(el("h2").id("evidence-title").child("Evidence keeps its authority.")),
        )
        .child(
            el("figure")
                .class("evidence-art")
                .attr("data-reveal", "")
                .child(brand_picture(
                    "evidence-archive",
                    "Four archival folios connected by one cyan lineage thread to a cobalt artifact",
                    "evidence-art__picture",
                ))
                .child(
                    el("figcaption")
                        .child(el("span").child("SOURCE → EVIDENCE → AUTHORITY"))
                        .child("No artifact inherits more authority than its evidence."),
                ),
        )
        .child(
            el("div")
                .class("evidence-layout")
                .child(
                    el("div")
                        .class(panel_class.clone())
                        .attr("data-reveal", "")
                        .child(el("span").class("evidence-state measured").child("MEASURED"))
                        .child(el("h3").child("Current source, direct execution"))
                        .child(el("p").child("Focused Rust tests, deterministic output, browser computed styles, or registry replay performed on the identified source.")),
                )
                .child(
                    el("div")
                        .class(panel_class.clone())
                        .attr("data-reveal", "")
                        .child(el("span").class("evidence-state inherited").child("INHERITED"))
                        .child(el("h3").child("Valid history, limited authority"))
                        .child(el("p").child("Useful context from an earlier commit that cannot green-light changed source.")),
                )
                .child(
                    el("div")
                        .class(panel_class)
                        .attr("data-reveal", "")
                        .child(el("span").class("evidence-state pending").child("PENDING"))
                        .child(el("h3").child("Required, not yet performed"))
                        .child(el("p").child("Visible blockers stay blockers. No documentation sentence turns them into a pass.")),
                ),
        )
}

fn docs_index() -> View {
    let mut directory = el("div").class("docs-directory");
    for category in doc_categories() {
        let mut group = el("section")
            .class("docs-category")
            .attr("data-doc-search-group", "")
            .child(
                el("div")
                    .class("docs-category-heading")
                    .child(el("span").child(*category))
                    .child(el("span").child(format!(
                            "{:02} entries",
                            DOCS.iter()
                                .filter(|document| document.category == *category)
                                .count()
                        ))),
            );
        for document in DOCS
            .iter()
            .filter(|document| document.category == *category)
        {
            group = group.child(doc_directory_item(document));
        }
        directory = directory.child(group);
    }
    el("div")
        .class("docs-landing page-shell")
        .child(page_intro("DOCUMENTATION", "Use the smallest surface that solves the problem.", "Start by auditing standard CSS. Typed authoring, manifests, repair, and migration are layered capabilities—not entry requirements."))
        .child(
            el("label")
                .class("doc-search")
                .child(el("span").child("SEARCH THE MANUAL"))
                .child(
                    el("input")
                        .attr("type", "search")
                        .attr("placeholder", "Try “migration”, “typed”, or “evidence”")
                        .attr("data-doc-search", ""),
                ),
        )
        .child(directory)
        .into_view()
}

fn doc_categories() -> &'static [&'static str] {
    &[
        "Start",
        "Core",
        "Standard CSS",
        "Evidence",
        "Migration",
        "Tooling",
        "Integrations",
        "Reference",
    ]
}

fn doc_directory_item(document: &DocPage) -> Element {
    el("a")
        .class("doc-directory-item")
        .attr("href", document.route)
        .attr("data-doc-search-item", "")
        .attr(
            "data-search-text",
            format!(
                "{} {} {} {}",
                document.category, document.eyebrow, document.title, document.summary
            ),
        )
        .child(
            el("span")
                .class("doc-index")
                .child(format!("{:02}", list_child_index(document.route))),
        )
        .child(
            el("span")
                .child(el("strong").child(document.eyebrow))
                .child(el("span").child(document.summary)),
        )
        .child(el("span").attr("aria-hidden", "true").child("↗"))
}

fn list_child_index(route: &str) -> usize {
    DOCS.iter()
        .position(|document| document.route == route)
        .map_or(1, |index| index + 1)
}

fn doc_page(document: &DocPage) -> View {
    let mut toc = el("nav")
        .class("doc-toc")
        .attr("aria-label", "On this page")
        .child(el("strong").child("ON THIS PAGE"));
    let mut content = el("article").class("doc-article");
    for section in document.sections {
        toc = toc.child(
            el("a")
                .attr("href", format!("#{}", section.id))
                .child(section.title),
        );
        let mut block = el("section")
            .id(section.id)
            .class("doc-section")
            .child(el("h2").child(section.title))
            .child(el("p").child(section.body));
        if let Some(code) = section.code {
            block = block.child(
                el("div")
                    .class("code-sample")
                    .child(
                        el("button")
                            .class("copy-button")
                            .attr("type", "button")
                            .attr("data-copy", "")
                            .attr("aria-label", "Copy code")
                            .child("COPY"),
                    )
                    .child(el("pre").child(el("code").child(code))),
            );
        }
        content = content.child(block);
    }
    el("div")
        .class("doc-page page-shell")
        .child(page_intro(
            document.eyebrow,
            document.title,
            document.summary,
        ))
        .child(
            el("div")
                .class("doc-layout")
                .child(docs_sidebar(document.route))
                .child(content)
                .child(toc),
        )
        .into_view()
}

fn docs_sidebar(active_route: &str) -> Element {
    let mut sidebar = el("nav")
        .class("docs-sidebar")
        .attr("aria-label", "Documentation");
    for category in doc_categories() {
        let mut group = el("div")
            .class("docs-sidebar-group")
            .child(el("strong").child(*category));
        for document in DOCS
            .iter()
            .filter(|document| document.category == *category)
        {
            let mut link = el("a").attr("href", document.route);
            if document.route == active_route {
                link = link.class("is-active").attr("aria-current", "page");
            }
            group = group.child(link.child(document.eyebrow));
        }
        if *category == "Reference" {
            group = group.child(
                el("a")
                    .attr("href", "/docs/utilities/")
                    .child("Generated utility catalog"),
            );
        }
        sidebar = sidebar.child(group);
    }
    sidebar
}

fn benchmarks() -> View {
    el("div")
        .class("benchmarks-page page-shell")
        .child(page_intro("BENCHMARKS", "Performance claims need a fixture, a method, and a source commit.", "PliegoCSS compares payload, fresh-process compilation, Rust check cost, pruning, migration quality, and browser behavior without collapsing unlike workloads into one number."))
        .child(
            el("div")
                .class("benchmark-hero")
                .child(
                    el("div")
                        .child(el("p").class("big-metric").child("104 / 104"))
                        .child(el("p").child("real migration roles identified with 0 false positives and 0 false negatives in the reviewed corpus")),
                )
                .child(
                    el("div")
                        .child(el("p").class("big-metric").child("50 / 50"))
                        .child(el("p").child("WSL DTCG watch repetitions kept the last valid group")),
                ),
        )
        .child(
            el("section")
                .class("benchmark-notice")
                .child(kicker("CURRENT RELEASE LIMIT"))
                .child(el("h2").child("Historical timing snapshots are being superseded."))
                .child(el("p").child("The prior snapshot commit is not recoverable from repository history. Current timings remain blocked until regenerated from the final clean, reachable source commit. Payload and correctness gates continue to run independently.")),
        )
        .child(
            el("div")
                .class("benchmark-links")
                .child(repo_link("Methodology", "docs/benchmarks/methodology.md"))
                .child(repo_link("Tailwind competitive matrix", "docs/benchmarks/tailwind-v4-competitive-matrix.md"))
                .child(repo_link("Reachability pruning", "docs/benchmarks/reachability-pruning.md"))
                .child(repo_link("Browser validation", "docs/benchmarks/browser-validation.md")),
        )
        .into_view()
}

fn brand() -> View {
    el("div")
        .class("brand-page page-shell")
        .child(page_intro("BRAND SYSTEM", "The cascade has a fold.", "One authored rail becomes one deterministic projection. The mark captures that transition without borrowing a letterform."))
        .child(
            el("section")
                .class("brand-showcase")
                .child(
                    el("div")
                        .class("brand-symbol-stage")
                        .child(
                            el("img")
                                .attr("src", "/brand/pliegocss-symbol-reversed.svg")
                                .attr("width", "256")
                                .attr("height", "256")
                                .attr("alt", "PliegoCSS folded cascade symbol"),
                        ),
                )
                .child(
                    el("div")
                        .class("brand-rules")
                        .child(kicker("CASCADE FOLD / 64 × 64"))
                        .child(el("h2").child("Authored input. Deterministic projection."))
                        .child(el("p").child("Use clear space equal to one stroke width. Never rotate, round, skew, outline, shadow, or recolor individual segments."))
                        .child(action_link("Open the complete brandbook", "https://github.com/celiumsai/pliegocss/blob/main/brand/BRANDBOOK.md")),
                ),
        )
        .child(
            el("section")
                .class("palette")
                .child(color_swatch("Carbon", "#070B14", "carbon"))
                .child(color_swatch("Paper", "#F4F7FB", "paper"))
                .child(color_swatch("Cobalt", "#1D4ED8", "cobalt"))
                .child(color_swatch("Cyan", "#22D3EE", "cyan"))
                .child(color_swatch("Coral", "#F97368", "coral"))
                .child(color_swatch("Amber", "#F59E0B", "amber")),
        )
        .child(
            el("section")
                .class("brand-image-system")
                .child(
                    el("div")
                        .class("brand-image-system__heading")
                        .child(kicker("GPT IMAGE 2 / CANONICAL MASTERS"))
                        .child(el("h2").child("Material makes the compiler legible."))
                        .child(el("p").child("Six reviewed masters turn source, transformation, lineage, and evidence into one tactile visual language. Every master is hash-bound to its generation receipt.")),
                )
                .child(
                    el("div")
                        .class("brand-image-system__lead")
                        .child(
                            el("figure")
                                .child(brand_picture(
                                    "semantic-fold",
                                    "Engineered paper enters a precise fold and emerges as aligned cobalt layers",
                                    "brand-image",
                                ))
                                .child(el("figcaption").child("Semantic fold / compiler narrative")),
                        )
                        .child(
                            el("figure")
                                .child(brand_picture(
                                    "evidence-archive",
                                    "Archival evidence folios joined to one compiled cobalt artifact",
                                    "brand-image",
                                ))
                                .child(el("figcaption").child("Evidence archive / provenance")),
                        ),
                )
                .child(
                    el("div")
                        .class("brand-image-system__materials")
                        .child(
                            el("figure")
                                .child(brand_picture(
                                    "material-study-carbon",
                                    "A square fold in carbon engineered paper with one cobalt edge",
                                    "brand-image",
                                ))
                                .child(el("figcaption").child("Carbon / authored source")),
                        )
                        .child(
                            el("figure")
                                .child(brand_picture(
                                    "material-study-paper",
                                    "Paper-white drafting film pierced by one cyan lineage filament",
                                    "brand-image",
                                ))
                                .child(el("figcaption").child("Paper / visible evidence")),
                        )
                        .child(
                            el("figure")
                                .child(brand_picture(
                                    "material-study-cobalt",
                                    "Cobalt artifact plate aligned over a carbon semantic grid",
                                    "brand-image",
                                ))
                                .child(el("figcaption").child("Cobalt / compiled artifact")),
                        ),
                ),
        )
        .into_view()
}

fn changelog() -> View {
    el("div")
        .class("changelog-page page-shell")
        .child(page_intro("CHANGELOG", "Release truth, not launch theater.", "The current candidate is available as a GitHub prerelease. Crates and the final 0.1.0 release remain unpublished."))
        .child(
            el("article")
                .class("release-entry")
                .child(el("div").class("release-date").child("2026 / 07 / 20"))
                .child(
                    el("div")
                        .child(kicker("V0.1.0-RC.1"))
                        .child(el("h2").child("First public-preview release candidate"))
                        .child(el("p").child("Standards-first audit and transform, typed Rust styles, themes and DTCG tokens, deterministic artifacts, reversible migration, editor clients, and a native PliegoRS boundary."))
                        .child(
                            el("a")
                                .class("text-link")
                                .attr("href", "https://github.com/celiumsai/pliegocss/releases/tag/v0.1.0-rc.1")
                                .child("Read the GitHub release ↗"),
                        ),
                ),
        )
        .child(
            el("p")
                .class("source-note")
                .child("The repository")
                .child(" "),
        )
        .child(repo_link("CHANGELOG.md", "CHANGELOG.md"))
        .into_view()
}

fn security() -> View {
    el("div")
        .class("security-page page-shell")
        .child(page_intro("SECURITY", "Report privately. Reproduce minimally.", "Security-sensitive surfaces include parsing, source discovery, repair, migration, publication locks, manifests, receipts, LSP process execution, and generated artifacts."))
        .child(
            el("div")
                .class("security-grid")
                .child(
                    el("section")
                        .child(kicker("DISCLOSURE"))
                        .child(el("h2").child("Do not open a public issue."))
                        .child(el("p").child("Use GitHub private vulnerability reporting or email hello@pliegocss.dev with the affected version, minimal reproduction, impact, prerequisites, and mitigation."))
                        .child(
                            el("a")
                                .class("text-link")
                                .attr("href", "mailto:hello@pliegocss.dev?subject=SECURITY%3A%20PliegoCSS")
                                .child("hello@pliegocss.dev ↗"),
                        ),
                )
                .child(
                    el("section")
                        .child(kicker("SUPPORTED"))
                        .child(el("h2").child("Latest candidate + main"))
                        .child(el("p").child("Before 1.0, fixes target the latest prerelease and the default branch. Response goals are not an SLA.")),
                ),
        )
        .child(repo_link("Read the complete security policy", "SECURITY.md"))
        .into_view()
}

fn legal_title(slug: &str) -> &'static str {
    match slug {
        "terms" => "Terms — PliegoCSS",
        "privacy" => "Privacy — PliegoCSS",
        "cookies" => "Cookies — PliegoCSS",
        "acceptable-use" => "Acceptable use — PliegoCSS",
        _ => "Legal — PliegoCSS",
    }
}

fn legal_hub() -> View {
    let entries = [
        (
            "/legal/terms/",
            "Terms",
            "Rules for the website, documentation, source, packages, and prerelease artifacts.",
        ),
        (
            "/legal/privacy/",
            "Privacy",
            "What this static site, correspondence, and infrastructure providers may process.",
        ),
        (
            "/legal/cookies/",
            "Cookies and storage",
            "No advertising cookies, cross-site trackers, or browser storage are used by this build.",
        ),
        (
            "/legal/acceptable-use/",
            "Acceptable use",
            "Boundaries for source, binaries, project identity, infrastructure, and security reports.",
        ),
    ];
    let mut register = el("div").class("legal-register");
    for (href, title, summary) in entries {
        register = register.child(
            el("a")
                .attr("href", href)
                .child(el("span").class("legal-register__mark").child("POLICY"))
                .child(el("h2").child(title))
                .child(el("p").child(summary))
                .child(el("span").class("legal-register__arrow").child("↗")),
        );
    }
    el("div")
        .class("legal-page page-shell")
        .child(page_intro(
            "LEGAL / CURRENT",
            "Plain language. Narrow scope.",
            "These current documents cover the public PliegoCSS website, documentation, source, packages, prerelease artifacts, and official project identity.",
        ))
        .child(
            el("p")
                .class("legal-effective")
                .child("Effective 2026-07-20 · English and Spanish have equal authority."),
        )
        .child(register)
        .into_view()
}

fn legal_document(slug: &str) -> View {
    let (title, intro, sections): (&str, &str, &[(&str, &str, &str)]) = match slug {
        "terms" => (
            "Terms",
            "These terms govern access to the PliegoCSS website and official public project materials.",
            &[
                (
                    "01",
                    "Current status",
                    "PliegoCSS 0.1.0-rc.1 is public-preview software. Crates are not yet published to crates.io, APIs may change before 1.0, and the release-readiness record remains the authority for promotion.",
                ),
                (
                    "02",
                    "License and notices",
                    "Source and packages identified by an SPDX notice are licensed under Apache-2.0. Brand assets, fonts, generated images, and third-party files remain subject to their accompanying licenses, notices, and trademark policy.",
                ),
                (
                    "03",
                    "No warranty",
                    "The software and website are provided without warranties to the extent permitted by law, as stated in the Apache-2.0 license. Benchmarks and evidence are scoped to the identified source and environment.",
                ),
                (
                    "04",
                    "Project identity",
                    "PliegoCSS, its name, and its logos are trademarks of Celiums Solutions LLC. The software license does not grant permission to imply endorsement, affiliation, or an official distribution.",
                ),
                (
                    "05",
                    "Changes and contact",
                    "Material changes are published on this register before they take effect. Questions may be sent to hello@pliegocss.dev.",
                ),
            ],
        ),
        "privacy" => (
            "Privacy",
            "The current static website does not create accounts, accept payments, run advertising, or build behavioral profiles.",
            &[
                (
                    "01",
                    "Browser data",
                    "This build does not set cookies or write to localStorage or sessionStorage. Language is represented by the URL, so the selected English or Spanish route can be bookmarked without a browser identifier.",
                ),
                (
                    "02",
                    "Email and reports",
                    "Messages sent to hello@pliegocss.dev or through GitHub private vulnerability reporting are processed to answer the request, investigate the report, preserve necessary correspondence, and protect the project.",
                ),
                (
                    "03",
                    "Infrastructure",
                    "Hosting, DNS, repository, and security providers may process standard request, abuse-prevention, and access logs under their own terms. PliegoCSS does not sell those records or use them for advertising.",
                ),
                (
                    "04",
                    "Public contributions",
                    "Issues, discussions, commits, and pull requests submitted to a public repository are intentionally public and are processed by the repository provider under its terms.",
                ),
                (
                    "05",
                    "Requests and contact",
                    "For a privacy question about correspondence controlled by Celiums Solutions LLC, contact hello@pliegocss.dev with enough context to identify the relevant exchange. Do not send unrelated credentials or sensitive data.",
                ),
            ],
        ),
        "cookies" => (
            "Cookies and storage",
            "The current PliegoCSS website uses no advertising cookies, cross-site tracking cookies, browser identifiers, or consent-requiring analytics.",
            &[
                (
                    "01",
                    "Current browser storage",
                    "The site does not write cookies, localStorage, or sessionStorage. Interactive laboratories operate in memory and reset when the document is reloaded.",
                ),
                (
                    "02",
                    "Language",
                    "English routes live at the root and Spanish routes under /es/. Changing language navigates to the equivalent route and does not store a preference.",
                ),
                (
                    "03",
                    "Provider controls",
                    "Infrastructure providers may use strictly necessary security or delivery mechanisms under their own policies. PliegoCSS does not add advertising or cross-site tracking on top of them.",
                ),
                (
                    "04",
                    "Future changes",
                    "Any material introduction of analytics, client storage, or tracking must be documented here before activation and evaluated for required consent.",
                ),
            ],
        ),
        "acceptable-use" => (
            "Acceptable use",
            "Use PliegoCSS materials without harming people, systems, users, or the integrity of the project.",
            &[
                (
                    "01",
                    "Security",
                    "Do not probe private infrastructure, bypass access controls, disrupt availability, collect data without authorization, publish unpatched exploits, or test systems you do not own or have permission to assess.",
                ),
                (
                    "02",
                    "Artifacts",
                    "Do not distribute altered binaries, packages, receipts, manifests, evidence, or website captures as official PliegoCSS artifacts. Preserve provenance and clearly label forks and modifications.",
                ),
                (
                    "03",
                    "Identity",
                    "Do not impersonate PliegoCSS, Celiums Solutions LLC, maintainers, security contacts, or official release channels. Follow the repository trademark policy for nominative use.",
                ),
                (
                    "04",
                    "Reports",
                    "Good-faith reports with minimal reproducible evidence are welcome through GitHub private vulnerability reporting or hello@pliegocss.dev. The security policy does not authorize unlawful or destructive testing.",
                ),
            ],
        ),
        _ => ("Legal", "Unsupported policy.", &[]),
    };

    let mut body = el("div").class("legal-document__body");
    for (number, heading, text) in sections {
        body = body.child(
            el("section")
                .child(el("span").class("legal-section-number").child(*number))
                .child(el("h2").child(*heading))
                .child(el("p").child(*text)),
        );
    }
    el("div")
        .class("legal-page legal-document page-shell")
        .child(page_intro("LEGAL / 2026-07-20", title, intro))
        .child(
            el("p")
                .class("legal-effective")
                .child("Effective 2026-07-20 · Current website and prerelease scope."),
        )
        .child(body)
        .child(
            el("nav")
                .class("legal-document__nav")
                .attr("aria-label", "Legal documents")
                .child(el("a").attr("href", "/legal/").child("Legal register"))
                .child(el("a").attr("href", "/legal/terms/").child("Terms"))
                .child(el("a").attr("href", "/legal/privacy/").child("Privacy"))
                .child(el("a").attr("href", "/legal/cookies/").child("Cookies"))
                .child(
                    el("a")
                        .attr("href", "/legal/acceptable-use/")
                        .child("Acceptable use"),
                ),
        )
        .into_view()
}

fn accessibility() -> View {
    let items = [
        (
            "01",
            "Useful HTML first",
            "Every page ships meaningful server-rendered HTML before client motion or laboratory hydration. Core navigation and documentation remain links, headings, lists, forms, and text.",
        ),
        (
            "02",
            "Keyboard and focus",
            "Interactive controls use native buttons, inputs, and dialogs; visible focus is preserved; the skip link reaches main content; and command search supports keyboard navigation.",
        ),
        (
            "03",
            "Reduced motion",
            "The site follows prefers-reduced-motion, disables smooth scrolling and WebGL decoration, and keeps content visible without scroll-triggered movement.",
        ),
        (
            "04",
            "Language and structure",
            "English and Spanish pages declare their language, provide equivalent URLs, preserve heading hierarchy, and keep the same product and legal scope.",
        ),
        (
            "05",
            "Report a barrier",
            "Email hello@pliegocss.dev with the route, browser or assistive technology, expected behavior, and the barrier encountered. Accessibility feedback is handled as product evidence.",
        ),
    ];
    let mut grid = el("div").class("accessibility-grid");
    for (number, title, body) in items {
        grid = grid.child(
            el("section")
                .child(el("span").child(number))
                .child(el("h2").child(title))
                .child(el("p").child(body)),
        );
    }
    el("div")
        .class("accessibility-page page-shell")
        .child(page_intro(
            "ACCESSIBILITY",
            "The baseline is usable before it is spectacular.",
            "PliegoCSS treats accessibility as an output invariant: semantic HTML, keyboard operation, stable focus, reduced motion, explicit language, and progressive enhancement.",
        ))
        .child(grid)
        .child(
            el("a")
                .class("text-link")
                .attr("href", "mailto:hello@pliegocss.dev?subject=ACCESSIBILITY%3A%20PliegoCSS")
                .child("Report an accessibility barrier ↗"),
        )
        .into_view()
}

fn not_found() -> View {
    el("div")
        .class("not-found page-shell")
        .child(kicker("404 / UNRESOLVED ROUTE"))
        .child(el("h1").child("This fold has no output."))
        .child(el("p").child("The requested path is not part of the current site graph."))
        .child(action_link("Return home", "/"))
        .into_view()
}

fn page_intro(eyebrow: &str, title: &str, summary: &str) -> Element {
    el("header")
        .class("page-intro")
        .child(kicker(eyebrow))
        .child(el("h1").child(title))
        .child(el("p").child(summary))
}

fn kicker(value: &str) -> Element {
    el("p")
        .class(format!("kicker {}", LABEL_STYLE.class_name()))
        .child(value)
}

fn action_link(label: &str, href: &str) -> Element {
    el("a")
        .class(format!("action-link {}", ACTION_STYLE.class_name()))
        .attr("href", href)
        .child(label)
        .child(el("span").attr("aria-hidden", "true").child("↗"))
}

fn metric(term: &str, value: &str) -> Element {
    el("div")
        .child(el("dt").child(term))
        .child(el("dd").child(value))
}

fn lab_control(dimension: &str, label: &str, options: &[(&str, &str)], selected: &str) -> Element {
    let mut group = el("fieldset")
        .class("lab-control")
        .attr("data-lab-control", dimension)
        .child(el("legend").child(label));
    for (value, option_label) in options {
        let mut button = el("button")
            .attr("type", "button")
            .attr("data-lab-option", *value)
            .attr(
                "aria-pressed",
                if *value == selected { "true" } else { "false" },
            )
            .child(*option_label);
        if *value == selected {
            button = button.class("is-selected");
        }
        group = group.child(button);
    }
    group
}

fn viewport_button(id: &str, width: &str, selected: bool) -> Element {
    el("button")
        .attr("type", "button")
        .attr("data-viewport", id)
        .attr("aria-pressed", if selected { "true" } else { "false" })
        .child(el("span").child(id))
        .child(el("small").child(format!("{width}px")))
}

fn output_tab(id: &str, label: &str, selected: bool) -> Element {
    el("button")
        .id(format!("output-tab-{id}"))
        .attr("type", "button")
        .attr("role", "tab")
        .attr("data-output-tab", id)
        .attr("aria-controls", format!("output-panel-{id}"))
        .attr("aria-selected", if selected { "true" } else { "false" })
        .attr("tabindex", if selected { "0" } else { "-1" })
        .child(label)
}

fn conflict_button(id: &str, label: &str, selected: bool) -> Element {
    el("button")
        .id(format!("conflict-tab-{id}"))
        .attr("type", "button")
        .attr("role", "tab")
        .attr("data-conflict", id)
        .attr("aria-controls", "conflict-diagnostic")
        .attr("aria-selected", if selected { "true" } else { "false" })
        .attr("tabindex", if selected { "0" } else { "-1" })
        .child(label)
}

fn step(index: &str, title: &str, body: &str) -> Element {
    el("li")
        .attr("data-cascade-step", "")
        .child(el("span").child(index))
        .child(el("strong").child(title))
        .child(el("small").child(body))
}

fn closing(title: &str, body: &str, action: &str, href: &str) -> Element {
    el("section")
        .class("closing")
        .attr("data-reveal", "")
        .child(kicker("THE OUTPUT IS THE PRODUCT"))
        .child(el("h2").child(title))
        .child(el("p").child(body))
        .child(action_link(action, href))
}

fn repo_link(label: &str, path: &str) -> Element {
    el("a")
        .class("repo-link")
        .attr(
            "href",
            format!("https://github.com/celiumsai/pliegocss/blob/main/{path}"),
        )
        .child(el("span").child(label))
        .child(el("span").attr("aria-hidden", "true").child("↗"))
}

fn color_swatch(name: &str, value: &str, class_name: &str) -> Element {
    el("div")
        .class(format!("swatch {class_name}"))
        .child(el("span").class("swatch-color").attr("aria-hidden", "true"))
        .child(el("strong").child(name))
        .child(el("code").child(value))
}

fn brand_picture(stem: &str, alt: &str, class_name: &str) -> Element {
    el("picture")
        .class(class_name)
        .child(
            el("source")
                .attr("srcset", format!("/media/brand/{stem}.avif"))
                .attr("type", "image/avif"),
        )
        .child(
            el("source")
                .attr("srcset", format!("/media/brand/{stem}.webp"))
                .attr("type", "image/webp"),
        )
        .child(
            el("img")
                .attr("src", format!("/media/brand/{stem}.png"))
                .attr("alt", alt)
                .attr(
                    "loading",
                    if stem == "cascade-chamber" {
                        "eager"
                    } else {
                        "lazy"
                    },
                )
                .attr("decoding", "async"),
        )
}

pub fn sitemap() -> Vec<u8> {
    let mut routes = vec![
        "/",
        "/playground/",
        "/examples/",
        "/docs/",
        "/docs/utilities/",
        "/benchmarks/",
        "/brand/",
        "/changelog/",
        "/security/",
        "/legal/",
        "/legal/terms/",
        "/legal/privacy/",
        "/legal/cookies/",
        "/legal/acceptable-use/",
        "/accessibility/",
    ];
    routes.extend(DOCS.iter().map(|document| document.route));
    routes.sort_unstable();
    routes.dedup();

    let entries = routes
        .into_iter()
        .flat_map(|route| {
            let english = format!("https://pliegocss.dev{route}");
            let spanish = format!(
                "https://pliegocss.dev{}",
                localize_route(Locale::Es, route)
            );
            [
                format!(
                    "<url><loc>{english}</loc><xhtml:link rel=\"alternate\" hreflang=\"en\" href=\"{english}\"/><xhtml:link rel=\"alternate\" hreflang=\"es\" href=\"{spanish}\"/><xhtml:link rel=\"alternate\" hreflang=\"x-default\" href=\"{english}\"/></url>"
                ),
                format!(
                    "<url><loc>{spanish}</loc><xhtml:link rel=\"alternate\" hreflang=\"es\" href=\"{spanish}\"/><xhtml:link rel=\"alternate\" hreflang=\"en\" href=\"{english}\"/><xhtml:link rel=\"alternate\" hreflang=\"x-default\" href=\"{english}\"/></url>"
                ),
            ]
        })
        .collect::<String>();
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\" xmlns:xhtml=\"http://www.w3.org/1999/xhtml\">{entries}</urlset>"
    )
    .into_bytes()
}
