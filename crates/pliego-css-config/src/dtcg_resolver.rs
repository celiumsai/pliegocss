//! Bounded same-document implementation of DTCG Resolver 2025.10.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Arc;

use serde_json::{Map, Value};

use super::dtcg::{
    DTCG_FORMAT_VERSION, DtcgError, DtcgTheme, MAX_DOCUMENT_BYTES, parse_dtcg_str,
    read_bounded_utf8_document, semantic_document_hash,
};
use super::token_graph::{TokenGraph, TokenGraphAdapter, TokenGraphSource, TokenGraphTheme};

/// Exact resolver profile implemented by `PliegoCSS` 0.1.
///
/// The profile implements the mandatory same-document reference baseline from Resolver 2025.10.
/// File-system and network references are intentionally rejected without performing I/O.
pub const DTCG_RESOLVER_PROFILE: &str = "2025.10/same-document-1";

const MAX_RESOLUTIONS: usize = 256;
const MAX_RESOLVER_DEPTH: usize = 64;
const MAX_GRAPH_SOURCES: usize = 65_536;
const MAX_REFERENCE_VISITS: usize = 262_144;
const MAX_MATERIALIZED_SOURCES: usize = 65_536;
const MAX_EXPANDED_BYTES: usize = 64 * 1024 * 1024;
const MAX_RETAINED_BYTES: usize = 64 * 1024 * 1024;

/// One parsed DTCG Resolver document and all of its validated context permutations.
#[derive(Clone, Debug)]
pub struct DtcgResolver {
    document: Value,
    graph: Arc<TokenGraph>,
    items: Vec<ResolverItem>,
    permutations: Vec<ResolvedPermutation>,
}

impl DtcgResolver {
    /// Returns the preserved resolver document.
    #[must_use]
    pub const fn document(&self) -> &Value {
        &self.document
    }

    /// Returns the canonical graph containing every validated resolver permutation.
    #[must_use]
    pub fn graph(&self) -> &TokenGraph {
        &self.graph
    }

    /// Returns the exact number of context permutations validated at load time.
    #[must_use]
    pub fn resolution_count(&self) -> usize {
        self.permutations.len()
    }

    /// Selects one resolved theme with closed, case-insensitive modifier inputs.
    ///
    /// Missing inputs use the modifier default when one exists. Unknown modifiers, invalid
    /// contexts, missing required inputs, and duplicate keys after case folding fail closed.
    ///
    /// # Errors
    ///
    /// Returns [`DtcgError`] when `inputs` do not select exactly one validated permutation.
    pub fn resolve(&self, inputs: &BTreeMap<String, String>) -> Result<DtcgTheme, DtcgError> {
        let selections = validate_inputs(&self.items, inputs)?;
        let permutation = self
            .permutations
            .iter()
            .find(|permutation| permutation.selections == selections)
            .ok_or_else(|| resolver_invalid("inputs", "validated permutation is absent"))?;
        Ok(permutation
            .theme
            .clone()
            .with_resolution(Arc::clone(&self.graph), selections))
    }

    /// Selects one resolved theme from an ordered input stream without losing duplicate keys.
    ///
    /// This is the preferred adapter surface for repeated CLI flags and declarative build inputs.
    /// Exact duplicates and duplicates after case folding fail before selection.
    ///
    /// # Errors
    ///
    /// Returns [`DtcgError`] for duplicate or otherwise invalid modifier inputs.
    pub fn resolve_entries<I, K, V>(&self, inputs: I) -> Result<DtcgTheme, DtcgError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        let mut mapped = BTreeMap::new();
        for (name, context) in inputs {
            let name = name.into();
            if mapped.insert(name.clone(), context.into()).is_some() {
                return Err(resolver_invalid(
                    "inputs",
                    format!("modifier `{name}` is duplicated"),
                ));
            }
        }
        self.resolve(&mapped)
    }
}

/// Parses and validates a bundled DTCG Resolver 2025.10 document.
///
/// Every possible context permutation is composed and passed through the DTCG format bridge before
/// this function succeeds. This catches cycles and invalid values even in non-default branches.
///
/// # Errors
///
/// Returns [`DtcgError`] for malformed JSON, unsupported versions or references, invalid resolver
/// structure, excessive permutations, or any invalid resolved DTCG theme.
pub fn parse_dtcg_resolver_str(source: &str) -> Result<DtcgResolver, DtcgError> {
    if source.len() > MAX_DOCUMENT_BYTES {
        return Err(DtcgError::Limit("resolver document exceeds 16 MiB"));
    }
    let document = serde_json::from_str(source).map_err(DtcgError::Parse)?;
    build_resolver(document)
}

/// Reads and validates a bundled DTCG Resolver 2025.10 document from `path`.
///
/// # Errors
///
/// Returns [`DtcgError`] when the file cannot be read or does not satisfy the bounded resolver
/// contract. The path must name a regular, non-link UTF-8 file no larger than 16 MiB.
pub fn parse_dtcg_resolver_path(path: impl AsRef<Path>) -> Result<DtcgResolver, DtcgError> {
    let path = path.as_ref();
    let source = read_bounded_utf8_document(path, "resolver document exceeds 16 MiB")?;
    parse_dtcg_resolver_str(&source)
}

#[derive(Clone, Debug)]
struct ResolverSet {
    name: String,
    sources: Vec<Value>,
}

#[derive(Clone, Debug)]
struct ResolverModifier {
    name: String,
    contexts: BTreeMap<String, Vec<Value>>,
    default: Option<String>,
}

#[derive(Clone, Debug)]
enum ResolverItem {
    Set(ResolverSet),
    Modifier(ResolverModifier),
}

impl ResolverItem {
    fn name(&self) -> &str {
        match self {
            Self::Set(set) => &set.name,
            Self::Modifier(modifier) => &modifier.name,
        }
    }
}

#[derive(Clone, Debug)]
struct ResolvedPermutation {
    selections: BTreeMap<String, String>,
    theme: DtcgTheme,
}

#[derive(Default)]
struct ExpansionBudget {
    reference_visits: usize,
    materialized_sources: usize,
    expanded_bytes: usize,
}

impl ExpansionBudget {
    fn visit_reference(&mut self) -> Result<(), DtcgError> {
        add_limited(
            &mut self.reference_visits,
            1,
            MAX_REFERENCE_VISITS,
            "resolver performs more than 262,144 reference visits",
        )
    }

    fn clone_target(&mut self, value: &Value) -> Result<Value, DtcgError> {
        self.add_value_bytes(value)?;
        Ok(value.clone())
    }

    fn materialize(&mut self, value: &Value) -> Result<(), DtcgError> {
        add_limited(
            &mut self.materialized_sources,
            1,
            MAX_MATERIALIZED_SOURCES,
            "resolver materializes more than 65,536 token sources",
        )?;
        self.add_value_bytes(value)
    }

    fn add_value_bytes(&mut self, value: &Value) -> Result<(), DtcgError> {
        let bytes = serde_json::to_vec(value).map_err(DtcgError::Serialize)?;
        add_limited(
            &mut self.expanded_bytes,
            bytes.len(),
            MAX_EXPANDED_BYTES,
            "resolver expansion exceeds 64 MiB",
        )
    }
}

#[derive(Default)]
struct RetentionBudget {
    sources: usize,
    bytes: usize,
}

impl RetentionBudget {
    fn add_theme(&mut self, theme: &DtcgTheme) -> Result<(), DtcgError> {
        add_limited(
            &mut self.sources,
            theme.graph().sources.len(),
            MAX_GRAPH_SOURCES,
            "combined resolver graph exceeds 65,536 sources",
        )?;
        let graph_bytes = theme
            .graph()
            .to_canonical_json()
            .map_err(DtcgError::Graph)?;
        let document_bytes = serde_json::to_vec(theme.document()).map_err(DtcgError::Serialize)?;
        add_limited(
            &mut self.bytes,
            graph_bytes.len(),
            MAX_RETAINED_BYTES,
            "retained resolver permutations exceed 64 MiB",
        )?;
        add_limited(
            &mut self.bytes,
            document_bytes.len(),
            MAX_RETAINED_BYTES,
            "retained resolver permutations exceed 64 MiB",
        )
    }
}

fn add_limited(
    total: &mut usize,
    additional: usize,
    limit: usize,
    reason: &'static str,
) -> Result<(), DtcgError> {
    *total = total
        .checked_add(additional)
        .filter(|total| *total <= limit)
        .ok_or(DtcgError::Limit(reason))?;
    Ok(())
}

fn build_resolver(document: Value) -> Result<DtcgResolver, DtcgError> {
    let root = document
        .as_object()
        .ok_or_else(|| resolver_invalid("$", "resolver root must be a JSON object"))?;
    validate_root(root)?;
    let sets = parse_root_sets(root)?;
    let root_modifiers = parse_root_modifiers(root)?;
    let mut structure_budget = ExpansionBudget::default();
    validate_declared_sources(&document, &sets, &root_modifiers, &mut structure_budget)?;
    let items = parse_resolution_order(root, &document, &mut structure_budget)?;
    let selections = enumerate_selections(&items)?;
    let mut permutations = Vec::with_capacity(selections.len());
    let mut expansion_budget = ExpansionBudget::default();
    let mut retention_budget = RetentionBudget::default();
    for selection in selections {
        let theme = resolve_permutation(&document, &items, &selection, &mut expansion_budget)
            .map_err(|error| contextualize_resolution(error, &selection))?;
        retention_budget.add_theme(&theme)?;
        permutations.push(ResolvedPermutation {
            selections: selection,
            theme,
        });
    }
    let graph = Arc::new(combine_graphs(&document, &permutations)?);
    Ok(DtcgResolver {
        document,
        graph,
        items,
        permutations,
    })
}

fn validate_root(root: &Map<String, Value>) -> Result<(), DtcgError> {
    validate_fields(
        root,
        &[
            "$defs",
            "$schema",
            "description",
            "modifiers",
            "name",
            "resolutionOrder",
            "sets",
            "version",
        ],
        "$",
    )?;
    let version = required_string(root, "version", "$")?;
    if version != DTCG_FORMAT_VERSION {
        return Err(resolver_invalid(
            "$.version",
            format!("expected resolver version `{DTCG_FORMAT_VERSION}`"),
        ));
    }
    for key in ["name", "description", "$schema"] {
        optional_string(root, key, "$")?;
    }
    for key in ["sets", "modifiers", "$defs"] {
        if root.get(key).is_some_and(|value| !value.is_object()) {
            return Err(resolver_invalid(
                format!("$.{key}"),
                "value must be an object",
            ));
        }
    }
    let order = root
        .get("resolutionOrder")
        .and_then(Value::as_array)
        .ok_or_else(|| resolver_invalid("$.resolutionOrder", "required array is missing"))?;
    if order.is_empty() {
        return Err(resolver_invalid(
            "$.resolutionOrder",
            "at least one set or modifier is required",
        ));
    }
    if order.len() > MAX_MATERIALIZED_SOURCES {
        return Err(DtcgError::Limit(
            "resolutionOrder contains more than 65,536 items",
        ));
    }
    Ok(())
}

fn parse_root_sets(root: &Map<String, Value>) -> Result<BTreeMap<String, ResolverSet>, DtcgError> {
    let mut output = BTreeMap::new();
    let Some(sets) = root.get("sets").and_then(Value::as_object) else {
        return Ok(output);
    };
    for (name, value) in sets {
        validate_name(name, &format!("$.sets.{name}"))?;
        output.insert(name.clone(), parse_set(name, value, false)?);
    }
    Ok(output)
}

fn parse_root_modifiers(
    root: &Map<String, Value>,
) -> Result<BTreeMap<String, ResolverModifier>, DtcgError> {
    let mut output = BTreeMap::new();
    let mut folded_names = BTreeSet::new();
    let Some(modifiers) = root.get("modifiers").and_then(Value::as_object) else {
        return Ok(output);
    };
    for (name, value) in modifiers {
        validate_name(name, &format!("$.modifiers.{name}"))?;
        if !folded_names.insert(fold(name)) {
            return Err(resolver_invalid(
                "$.modifiers",
                format!("modifier name `{name}` collides after case folding"),
            ));
        }
        output.insert(name.clone(), parse_modifier(name, value, false)?);
    }
    Ok(output)
}

fn parse_resolution_order(
    root: &Map<String, Value>,
    document: &Value,
    budget: &mut ExpansionBudget,
) -> Result<Vec<ResolverItem>, DtcgError> {
    let order = root["resolutionOrder"]
        .as_array()
        .expect("root validation guarantees an array");
    let mut items = Vec::with_capacity(order.len());
    let mut names = BTreeSet::new();
    for (index, value) in order.iter().enumerate() {
        let path = format!("$.resolutionOrder[{index}]");
        let object = value
            .as_object()
            .ok_or_else(|| resolver_invalid(&path, "item must be an object"))?;
        let item = if object.contains_key("$ref") {
            parse_referenced_item(object, document, &path, budget)?
        } else {
            parse_inline_item(value, &path)?
        };
        if !names.insert(fold(item.name())) {
            return Err(resolver_invalid(
                &path,
                format!(
                    "resolution item name `{}` is duplicated after case folding",
                    item.name()
                ),
            ));
        }
        items.push(item);
    }
    Ok(items)
}

fn parse_referenced_item(
    object: &Map<String, Value>,
    document: &Value,
    path: &str,
    budget: &mut ExpansionBudget,
) -> Result<ResolverItem, DtcgError> {
    let resolved =
        resolve_reference_object(document, object, ReferenceScope::ResolutionOrder, budget)?;
    let target_kind = direct_definition_target(&resolved.terminal_fragment)?;
    let resolved_object = resolved
        .value
        .as_object()
        .ok_or_else(|| resolver_invalid(path, "reference must resolve to a set or modifier"))?;
    let local_name = resolved_object.get("name").and_then(Value::as_str);
    let (kind, name) = if let Some((kind, name)) = target_kind {
        (kind, local_name.unwrap_or(&name).to_owned())
    } else {
        let kind = required_string(resolved_object, "type", path)?;
        let name = required_string(resolved_object, "name", path)?.to_owned();
        (kind.to_owned(), name)
    };
    match kind.as_str() {
        "set" => Ok(ResolverItem::Set(parse_set(&name, &resolved.value, true)?)),
        "modifier" => Ok(ResolverItem::Modifier(parse_modifier(
            &name,
            &resolved.value,
            true,
        )?)),
        _ => Err(resolver_invalid(
            path,
            "reference must resolve to a `set` or `modifier`",
        )),
    }
}

fn parse_inline_item(value: &Value, path: &str) -> Result<ResolverItem, DtcgError> {
    let object = value
        .as_object()
        .expect("resolution item object was checked by caller");
    let kind = required_string(object, "type", path)?;
    let name = required_string(object, "name", path)?;
    validate_name(name, path)?;
    match kind {
        "set" => Ok(ResolverItem::Set(parse_set(name, value, true)?)),
        "modifier" => Ok(ResolverItem::Modifier(parse_modifier(name, value, true)?)),
        _ => Err(resolver_invalid(path, "`type` must be `set` or `modifier`")),
    }
}

fn parse_set(name: &str, value: &Value, ordered: bool) -> Result<ResolverSet, DtcgError> {
    let path = format!("set `{name}`");
    let object = value
        .as_object()
        .ok_or_else(|| resolver_invalid(&path, "set must be an object"))?;
    let allowed = if ordered {
        &["$extensions", "description", "name", "sources", "type"][..]
    } else {
        &["$extensions", "description", "sources"][..]
    };
    validate_fields(object, allowed, &path)?;
    validate_description_and_extensions(object, &path)?;
    if ordered {
        validate_optional_item_identity(object, "set", &path)?;
    }
    let sources = object
        .get("sources")
        .and_then(Value::as_array)
        .ok_or_else(|| resolver_invalid(&path, "set requires a `sources` array"))?;
    if sources.is_empty() {
        return Err(resolver_invalid(&path, "set `sources` must not be empty"));
    }
    Ok(ResolverSet {
        name: name.to_owned(),
        sources: sources.clone(),
    })
}

fn parse_modifier(name: &str, value: &Value, ordered: bool) -> Result<ResolverModifier, DtcgError> {
    let path = format!("modifier `{name}`");
    let object = value
        .as_object()
        .ok_or_else(|| resolver_invalid(&path, "modifier must be an object"))?;
    let allowed = if ordered {
        &[
            "$extensions",
            "contexts",
            "default",
            "description",
            "name",
            "type",
        ][..]
    } else {
        &["$extensions", "contexts", "default", "description"][..]
    };
    validate_fields(object, allowed, &path)?;
    validate_description_and_extensions(object, &path)?;
    if ordered {
        validate_optional_item_identity(object, "modifier", &path)?;
    }
    let contexts = object
        .get("contexts")
        .and_then(Value::as_object)
        .ok_or_else(|| resolver_invalid(&path, "modifier requires a `contexts` object"))?;
    if contexts.len() < 2 {
        return Err(resolver_invalid(
            &path,
            "modifier requires at least two contexts in this profile",
        ));
    }
    let mut parsed_contexts = BTreeMap::new();
    let mut folded_contexts = BTreeSet::new();
    for (context, sources) in contexts {
        validate_name(context, &format!("{path}.contexts.{context}"))?;
        if !folded_contexts.insert(fold(context)) {
            return Err(resolver_invalid(
                &path,
                format!("context `{context}` collides after case folding"),
            ));
        }
        parsed_contexts.insert(
            context.clone(),
            parse_context_sources(sources, &format!("{path}.contexts.{context}"))?,
        );
    }
    let default = optional_string(object, "default", &path)?.map(str::to_owned);
    if default
        .as_ref()
        .is_some_and(|default| !parsed_contexts.contains_key(default))
    {
        return Err(resolver_invalid(
            &path,
            "modifier default must exactly match one context",
        ));
    }
    Ok(ResolverModifier {
        name: name.to_owned(),
        contexts: parsed_contexts,
        default,
    })
}

fn parse_context_sources(value: &Value, path: &str) -> Result<Vec<Value>, DtcgError> {
    value
        .as_array()
        .cloned()
        .ok_or_else(|| resolver_invalid(path, "context must be a source array"))
}

fn validate_declared_sources(
    document: &Value,
    sets: &BTreeMap<String, ResolverSet>,
    modifiers: &BTreeMap<String, ResolverModifier>,
    budget: &mut ExpansionBudget,
) -> Result<(), DtcgError> {
    for (name, set) in sets {
        drop(expand_sources(
            document,
            &set.sources,
            &mut Vec::new(),
            0,
            budget,
            &format!("$.sets.{name}.sources"),
        )?);
    }
    for (modifier_name, modifier) in modifiers {
        for (context, sources) in &modifier.contexts {
            drop(expand_sources(
                document,
                sources,
                &mut Vec::new(),
                0,
                budget,
                &format!("$.modifiers.{modifier_name}.contexts.{context}"),
            )?);
        }
    }
    Ok(())
}

fn enumerate_selections(
    items: &[ResolverItem],
) -> Result<Vec<BTreeMap<String, String>>, DtcgError> {
    let modifiers = items
        .iter()
        .filter_map(|item| match item {
            ResolverItem::Set(_) => None,
            ResolverItem::Modifier(modifier) => Some(modifier),
        })
        .collect::<Vec<_>>();
    let count = modifiers.iter().try_fold(1_usize, |count, modifier| {
        count.checked_mul(modifier.contexts.len())
    });
    if count.is_none_or(|count| count > MAX_RESOLUTIONS) {
        return Err(DtcgError::Limit(
            "resolver produces more than 256 context permutations",
        ));
    }
    let mut selections = vec![BTreeMap::new()];
    for modifier in modifiers {
        let mut expanded = Vec::with_capacity(selections.len() * modifier.contexts.len());
        for selection in selections {
            for context in modifier.contexts.keys() {
                let mut next = selection.clone();
                next.insert(modifier.name.clone(), context.clone());
                expanded.push(next);
            }
        }
        selections = expanded;
    }
    Ok(selections)
}

fn resolve_permutation(
    document: &Value,
    items: &[ResolverItem],
    selections: &BTreeMap<String, String>,
    budget: &mut ExpansionBudget,
) -> Result<DtcgTheme, DtcgError> {
    let mut tokens = Map::new();
    for (index, item) in items.iter().enumerate() {
        let (sources, source_path) = match item {
            ResolverItem::Set(set) => (
                &set.sources,
                format!("$.resolutionOrder[{index}].set[{}]", set.name),
            ),
            ResolverItem::Modifier(modifier) => {
                let context = &selections[&modifier.name];
                (
                    &modifier.contexts[context],
                    format!(
                        "$.resolutionOrder[{index}].modifier[{}={context}]",
                        modifier.name
                    ),
                )
            }
        };
        for source in expand_sources(document, sources, &mut Vec::new(), 0, budget, &source_path)? {
            let object = source.as_object().ok_or_else(|| {
                resolver_invalid(&source_path, "token source must resolve to a JSON object")
            })?;
            merge_token_groups(&mut tokens, object);
        }
    }
    let source = serde_json::to_string(&Value::Object(tokens)).map_err(DtcgError::Serialize)?;
    parse_dtcg_str(&source)
}

fn expand_sources(
    document: &Value,
    sources: &[Value],
    active_sets: &mut Vec<String>,
    depth: usize,
    budget: &mut ExpansionBudget,
    path: &str,
) -> Result<Vec<Value>, DtcgError> {
    if depth > MAX_RESOLVER_DEPTH {
        return Err(DtcgError::Limit(
            "resolver reference nesting exceeds 64 levels",
        ));
    }
    let mut output = Vec::new();
    for (index, source) in sources.iter().enumerate() {
        let source_path = format!("{path}[{index}]");
        let Some(object) = source.as_object() else {
            return Err(resolver_invalid(
                source_path,
                "source must be a JSON object",
            ));
        };
        if !object.contains_key("$ref") {
            budget.materialize(source)?;
            output.push(source.clone());
            continue;
        }
        let reference = reference_string(object, &source_path)?;
        let fragment = normalized_fragment(reference, ReferenceScope::Source)?;
        let canonical_reference = format!("#{fragment}");
        if active_sets.contains(&canonical_reference) {
            let mut chain = active_sets.clone();
            chain.push(canonical_reference);
            return Err(resolver_invalid(
                source_path,
                format!("circular reference: {}", chain.join(" -> ")),
            ));
        }
        active_sets.push(canonical_reference);
        let resolved = resolve_reference_object(document, object, ReferenceScope::Source, budget)?;
        if let Some(set) = referenced_set(&resolved, &source_path)? {
            output.extend(expand_sources(
                document,
                &set.sources,
                active_sets,
                depth + 1,
                budget,
                &format!("{source_path}->set[{}]", set.name),
            )?);
        } else {
            budget.materialize(&resolved.value)?;
            output.push(resolved.value);
        }
        active_sets.pop();
    }
    Ok(output)
}

fn referenced_set(
    resolved: &ResolvedReference,
    path: &str,
) -> Result<Option<ResolverSet>, DtcgError> {
    if let Some((kind, name)) = direct_definition_target(&resolved.terminal_fragment)? {
        if kind == "set" {
            return parse_set(&name, &resolved.value, true).map(Some);
        }
        return Ok(None);
    }
    let Some(object) = resolved.value.as_object() else {
        return Ok(None);
    };
    if object.get("type").and_then(Value::as_str) != Some("set") {
        return Ok(None);
    }
    let name = required_string(object, "name", path)?;
    parse_set(name, &resolved.value, true).map(Some)
}

#[derive(Clone, Copy)]
enum ReferenceScope {
    ResolutionOrder,
    Source,
}

struct ResolvedReference {
    value: Value,
    terminal_fragment: String,
}

fn resolve_reference_object(
    document: &Value,
    object: &Map<String, Value>,
    scope: ReferenceScope,
    budget: &mut ExpansionBudget,
) -> Result<ResolvedReference, DtcgError> {
    resolve_reference_chain(document, object, scope, &mut Vec::new(), 0, budget)
}

fn resolve_reference_chain(
    document: &Value,
    object: &Map<String, Value>,
    scope: ReferenceScope,
    active: &mut Vec<String>,
    depth: usize,
    budget: &mut ExpansionBudget,
) -> Result<ResolvedReference, DtcgError> {
    if depth > MAX_RESOLVER_DEPTH {
        return Err(DtcgError::Limit(
            "resolver reference nesting exceeds 64 levels",
        ));
    }
    let reference = reference_string(object, "$ref")?;
    let fragment = normalized_fragment(reference, scope)?;
    let canonical_reference = format!("#{fragment}");
    budget.visit_reference()?;
    if active.contains(&canonical_reference) {
        let mut chain = active.clone();
        chain.push(canonical_reference);
        return Err(resolver_invalid(
            reference,
            format!("circular reference: {}", chain.join(" -> ")),
        ));
    }
    active.push(canonical_reference.clone());
    let target = document
        .pointer(&fragment)
        .ok_or_else(|| resolver_invalid(reference, "JSON Pointer target does not exist"))?;
    let target = budget.clone_target(target)?;
    let mut resolved = if let Some(target_object) = target.as_object() {
        if target_object.contains_key("$ref") {
            resolve_reference_chain(document, target_object, scope, active, depth + 1, budget)?
        } else {
            ResolvedReference {
                value: target,
                terminal_fragment: fragment,
            }
        }
    } else {
        ResolvedReference {
            value: target,
            terminal_fragment: fragment,
        }
    };
    active.pop();
    let overrides = object
        .iter()
        .filter(|(key, _)| key.as_str() != "$ref")
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect::<Map<_, _>>();
    if overrides.is_empty() {
        return Ok(resolved);
    }
    let target = resolved.value.as_object_mut().ok_or_else(|| {
        resolver_invalid(
            reference,
            "local overrides require an object reference target",
        )
    })?;
    for (key, value) in overrides {
        target.insert(key, value);
    }
    budget.add_value_bytes(&resolved.value)?;
    Ok(resolved)
}

fn normalized_fragment(reference: &str, scope: ReferenceScope) -> Result<String, DtcgError> {
    let Some(encoded) = reference.strip_prefix('#') else {
        return Err(resolver_invalid(
            reference,
            "external filesystem and network references are unsupported by this profile",
        ));
    };
    let fragment = decode_uri_fragment(encoded)?;
    if !fragment.is_empty() && !fragment.starts_with('/') {
        return Err(resolver_invalid(
            reference,
            "same-document reference must be `#` or begin with `#/`",
        ));
    }
    if fragment == "/resolutionOrder" || fragment.starts_with("/resolutionOrder/") {
        return Err(resolver_invalid(
            reference,
            "references into `resolutionOrder` are forbidden",
        ));
    }
    if matches!(scope, ReferenceScope::Source)
        && (fragment == "/modifiers" || fragment.starts_with("/modifiers/"))
    {
        return Err(resolver_invalid(
            reference,
            "only `resolutionOrder` may reference a modifier",
        ));
    }
    if !fragment.is_empty() {
        for segment in fragment[1..].split('/') {
            decode_pointer_segment(segment)?;
        }
    }
    Ok(fragment)
}

fn direct_definition_target(fragment: &str) -> Result<Option<(String, String)>, DtcgError> {
    let Some(fragment) = fragment.strip_prefix('/') else {
        return Ok(None);
    };
    let mut parts = fragment.split('/');
    let Some(collection) = parts.next() else {
        return Ok(None);
    };
    let Some(name) = parts.next() else {
        return Ok(None);
    };
    if parts.next().is_some() || !matches!(collection, "sets" | "modifiers") {
        return Ok(None);
    }
    let name = decode_pointer_segment(name)?;
    let kind = if collection == "sets" {
        "set"
    } else {
        "modifier"
    };
    Ok(Some((kind.to_owned(), name)))
}

fn decode_uri_fragment(fragment: &str) -> Result<String, DtcgError> {
    let bytes = fragment.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'%' {
            output.push(bytes[index]);
            index += 1;
            continue;
        }
        let high = bytes.get(index + 1).and_then(|byte| hex_digit(*byte));
        let low = bytes.get(index + 2).and_then(|byte| hex_digit(*byte));
        let (Some(high), Some(low)) = (high, low) else {
            return Err(resolver_invalid(
                fragment,
                "invalid percent escape in JSON Pointer fragment",
            ));
        };
        output.push((high << 4) | low);
        index += 3;
    }
    String::from_utf8(output)
        .map_err(|_| resolver_invalid(fragment, "JSON Pointer fragment is not valid UTF-8"))
}

const fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn decode_pointer_segment(segment: &str) -> Result<String, DtcgError> {
    let mut output = String::with_capacity(segment.len());
    let mut characters = segment.chars();
    while let Some(character) = characters.next() {
        if character != '~' {
            output.push(character);
            continue;
        }
        match characters.next() {
            Some('0') => output.push('~'),
            Some('1') => output.push('/'),
            _ => {
                return Err(resolver_invalid(
                    segment,
                    "invalid JSON Pointer escape sequence",
                ));
            }
        }
    }
    Ok(output)
}

fn merge_token_groups(target: &mut Map<String, Value>, overlay: &Map<String, Value>) {
    for (key, value) in overlay {
        if key.starts_with('$') {
            target.insert(key.clone(), value.clone());
            continue;
        }
        let merge_children = target
            .get(key)
            .and_then(Value::as_object)
            .is_some_and(|left| {
                value
                    .as_object()
                    .is_some_and(|right| !is_token(left) && !is_token(right))
            });
        if merge_children {
            let left = target[key]
                .as_object_mut()
                .expect("merge predicate guarantees an object");
            let right = value
                .as_object()
                .expect("merge predicate guarantees an object");
            merge_token_groups(left, right);
        } else {
            target.insert(key.clone(), value.clone());
        }
    }
}

fn contextualize_resolution(error: DtcgError, selections: &BTreeMap<String, String>) -> DtcgError {
    let label = if selections.is_empty() {
        "default".to_owned()
    } else {
        selections
            .iter()
            .map(|(name, context)| format!("{name}={context}"))
            .collect::<Vec<_>>()
            .join(",")
    };
    match error {
        DtcgError::Invalid { path, reason } => DtcgError::Invalid {
            path: format!("resolution[{label}].{path}"),
            reason,
        },
        error => error,
    }
}

fn is_token(object: &Map<String, Value>) -> bool {
    object.contains_key("$value") || object.contains_key("$ref")
}

fn validate_inputs(
    items: &[ResolverItem],
    inputs: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>, DtcgError> {
    let modifiers = items
        .iter()
        .filter_map(|item| match item {
            ResolverItem::Set(_) => None,
            ResolverItem::Modifier(modifier) => Some(modifier),
        })
        .collect::<Vec<_>>();
    let by_name = modifiers
        .iter()
        .map(|modifier| (fold(&modifier.name), *modifier))
        .collect::<BTreeMap<_, _>>();
    let mut folded_inputs = BTreeMap::new();
    for (name, context) in inputs {
        if folded_inputs
            .insert(fold(name), (name.as_str(), context.as_str()))
            .is_some()
        {
            return Err(resolver_invalid(
                "inputs",
                format!("modifier `{name}` is duplicated after case folding"),
            ));
        }
    }
    for (name, _) in folded_inputs.values() {
        if !by_name.contains_key(&fold(name)) {
            return Err(resolver_invalid(
                "inputs",
                format!("unknown modifier `{name}`"),
            ));
        }
    }
    let mut selections = BTreeMap::new();
    for modifier in modifiers {
        let selected = folded_inputs
            .get(&fold(&modifier.name))
            .map(|(_, context)| *context)
            .or(modifier.default.as_deref())
            .ok_or_else(|| {
                resolver_invalid(
                    "inputs",
                    format!("missing required modifier `{}`", modifier.name),
                )
            })?;
        let folded_context = fold(selected);
        let context = modifier
            .contexts
            .keys()
            .find(|context| fold(context) == folded_context)
            .ok_or_else(|| {
                resolver_invalid(
                    "inputs",
                    format!(
                        "invalid context `{selected}` for modifier `{}`",
                        modifier.name
                    ),
                )
            })?;
        selections.insert(modifier.name.clone(), context.clone());
    }
    Ok(selections)
}

fn combine_graphs(
    document: &Value,
    permutations: &[ResolvedPermutation],
) -> Result<TokenGraph, DtcgError> {
    let mut sources = Vec::new();
    let mut themes = Vec::with_capacity(permutations.len());
    let mut inventory = BTreeMap::new();
    for (index, permutation) in permutations.iter().enumerate() {
        let prefix = format!("resolution.{index:03}.");
        let pointer_prefix = format!("/resolutions/{index:03}");
        for source in &permutation.theme.graph().sources {
            sources.push(prefix_source(source, &prefix, &pointer_prefix));
        }
        if sources.len() > MAX_GRAPH_SOURCES {
            return Err(DtcgError::Limit(
                "combined resolver graph exceeds 65,536 sources",
            ));
        }
        for (token_type, count) in &permutation.theme.graph().dtcg_inventory {
            inventory
                .entry(token_type.clone())
                .and_modify(|maximum: &mut u64| *maximum = (*maximum).max(*count))
                .or_insert(*count);
        }
        let source_theme = &permutation.theme.graph().themes[0];
        let mut tokens = source_theme.tokens.clone();
        for token in &mut tokens {
            if let Some(source) = &mut token.source {
                source.insert_str(0, &prefix);
            }
        }
        themes.push(TokenGraphTheme {
            name: format!("resolution-{index:03}"),
            selections: permutation.selections.clone(),
            theme_id: source_theme.theme_id.clone(),
            dtcg_inventory: source_theme.dtcg_inventory.clone(),
            tokens,
        });
    }
    sources.sort_by(|left, right| left.path.cmp(&right.path));
    let graph = TokenGraph {
        schema_version: 1,
        graph_version: super::TOKEN_GRAPH_VERSION.to_owned(),
        adapter: Some(TokenGraphAdapter {
            name: "dtcg-resolver".to_owned(),
            version: DTCG_RESOLVER_PROFILE.to_owned(),
            source_hash: Some(semantic_document_hash(document)?),
        }),
        sources,
        themes,
        dtcg_inventory: inventory,
    };
    graph.to_canonical_json().map_err(DtcgError::Graph)?;
    Ok(graph)
}

fn prefix_source(
    source: &TokenGraphSource,
    prefix: &str,
    pointer_prefix: &str,
) -> TokenGraphSource {
    let mut source = source.clone();
    source.path.insert_str(0, prefix);
    source.pointer.insert_str(0, pointer_prefix);
    for reference in &mut source.references {
        if let Some(target) = &mut reference.target {
            target.insert_str(0, prefix);
        }
    }
    source
}

fn validate_fields(
    object: &Map<String, Value>,
    allowed: &[&str],
    path: &str,
) -> Result<(), DtcgError> {
    if let Some(key) = object.keys().find(|key| !allowed.contains(&key.as_str())) {
        return Err(resolver_invalid(
            path,
            format!("unknown field `{key}` in closed resolver contract"),
        ));
    }
    Ok(())
}

fn validate_description_and_extensions(
    object: &Map<String, Value>,
    path: &str,
) -> Result<(), DtcgError> {
    optional_string(object, "description", path)?;
    if object
        .get("$extensions")
        .is_some_and(|value| !value.is_object())
    {
        return Err(resolver_invalid(path, "`$extensions` must be an object"));
    }
    Ok(())
}

fn validate_optional_item_identity(
    object: &Map<String, Value>,
    expected: &str,
    path: &str,
) -> Result<(), DtcgError> {
    if let Some(kind) = optional_string(object, "type", path)? {
        if kind != expected {
            return Err(resolver_invalid(
                path,
                format!("item type must remain `{expected}`"),
            ));
        }
    }
    if let Some(name) = optional_string(object, "name", path)? {
        validate_name(name, path)?;
    }
    Ok(())
}

fn required_string<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<&'a str, DtcgError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| resolver_invalid(path, format!("required string field `{key}` is missing")))
}

fn optional_string<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<Option<&'a str>, DtcgError> {
    match object.get(key) {
        None => Ok(None),
        Some(Value::String(value)) => Ok(Some(value)),
        Some(_) => Err(resolver_invalid(
            path,
            format!("field `{key}` must be a string"),
        )),
    }
}

fn reference_string<'a>(object: &'a Map<String, Value>, path: &str) -> Result<&'a str, DtcgError> {
    required_string(object, "$ref", path)
}

fn validate_name(name: &str, path: &str) -> Result<(), DtcgError> {
    if name.is_empty() || name.chars().any(char::is_control) {
        return Err(resolver_invalid(
            path,
            "name must be non-empty and contain no control characters",
        ));
    }
    Ok(())
}

fn fold(value: &str) -> String {
    value.chars().flat_map(char::to_lowercase).collect()
}

fn resolver_invalid(path: impl Into<String>, reason: impl Into<String>) -> DtcgError {
    DtcgError::Invalid {
        path: path.into(),
        reason: reason.into(),
    }
}
