//! Standard Language Server Protocol transport for `PliegoCSS` Rust literals.

#![recursion_limit = "256"]

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::{self, BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::Command;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use pliego_css_parser::{format_style_list, parse_style_list};
use pliego_css_source::{InvocationKind, ScanReport, SourceRange, StyleLiteral, scan_source_named};
use serde_json::{Value, json};

mod project_index;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const MAX_SEMANTIC_CACHE_ITEMS: usize = 4_096;
const MAX_SEMANTIC_CHECKS_PER_DOCUMENT: usize = 256;
const SEMANTIC_DEBOUNCE: Duration = Duration::from_millis(150);

/// Exact process and theme selection used by delegated tooling requests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerConfig {
    /// Exact `pliego-cssc` executable name or path.
    pub compiler: OsString,
    /// Use the deterministic seed theme instead of conventional discovery.
    pub seed: bool,
    /// Optional exact theme configuration path.
    pub config: Option<PathBuf>,
    /// Optional exact generated Project Index path used for source-to-CSS navigation.
    pub project_index: Option<PathBuf>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            compiler: std::env::var_os("PLIEGO_CSSC")
                .unwrap_or_else(|| OsString::from("pliego-cssc")),
            seed: false,
            config: None,
            project_index: None,
        }
    }
}

/// Parses process arguments and serves LSP messages over stdin/stdout.
///
/// # Errors
///
/// Returns an error for invalid arguments, malformed protocol frames, JSON failures, or output I/O.
pub fn run_from_env() -> Result<(), String> {
    let config = parse_args(std::env::args_os().skip(1))?;
    let stdin = io::stdin();
    let stdout = io::stdout();
    serve(BufReader::new(stdin), stdout.lock(), config)
}

fn parse_args(arguments: impl IntoIterator<Item = OsString>) -> Result<ServerConfig, String> {
    let mut config = ServerConfig::default();
    let mut arguments = arguments.into_iter();
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--pliego-cssc") => {
                config.compiler = arguments.next().ok_or("`--pliego-cssc` requires a value")?;
            }
            Some("--seed") if !config.seed && config.config.is_none() => config.seed = true,
            Some("--config") if !config.seed && config.config.is_none() => {
                config.config = Some(PathBuf::from(
                    arguments.next().ok_or("`--config` requires a value")?,
                ));
            }
            Some("--project-index") if config.project_index.is_none() => {
                config.project_index = Some(PathBuf::from(
                    arguments
                        .next()
                        .ok_or("`--project-index` requires a value")?,
                ));
            }
            Some(value) => return Err(format!("unknown or conflicting option `{value}`")),
            None => return Err("arguments must be valid Unicode".into()),
        }
    }
    Ok(config)
}

#[derive(Debug)]
struct Document {
    text: String,
    version: i64,
}

struct Server {
    config: ServerConfig,
    root: PathBuf,
    documents: BTreeMap<String, Document>,
    catalog: Option<Vec<Value>>,
    shutdown: bool,
}

impl Server {
    fn new(config: ServerConfig) -> Self {
        Self {
            config,
            root: PathBuf::from("."),
            documents: BTreeMap::new(),
            catalog: None,
            shutdown: false,
        }
    }

    fn tool(&self, command: &str, extra: &[&str]) -> Result<Value, String> {
        let mut process = Command::new(&self.config.compiler);
        process.arg(command).args(extra).current_dir(&self.root);
        if self.config.seed {
            process.arg("--seed");
        } else if let Some(config) = &self.config.config {
            process.arg("--config").arg(config);
        }
        process.arg("--format").arg("json");
        let output = process
            .output()
            .map_err(|error| format!("cannot run pliego-cssc: {error}"))?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
        }
        serde_json::from_slice(&output.stdout)
            .map_err(|error| format!("invalid pliego-cssc JSON: {error}"))
    }

    fn catalog(&mut self) -> Result<&[Value], String> {
        if self.catalog.is_none() {
            let document = self.tool("catalog", &[])?;
            if document.get("schemaVersion") != Some(&json!(3)) {
                return Err("unsupported catalog schema".into());
            }
            self.catalog = Some(
                document
                    .get("utilities")
                    .and_then(Value::as_array)
                    .ok_or("catalog has no utilities")?
                    .clone(),
            );
        }
        Ok(self.catalog.as_deref().unwrap_or_default())
    }
}

struct SemanticEngine {
    config: ServerConfig,
    cache: BTreeMap<String, Result<Vec<Value>, String>>,
}

impl SemanticEngine {
    fn new(config: ServerConfig) -> Self {
        Self {
            config,
            cache: BTreeMap::new(),
        }
    }

    fn findings(&mut self, root: &PathBuf, style: &str) -> Result<&[Value], String> {
        if !self.cache.contains_key(style) {
            if self.cache.len() >= MAX_SEMANTIC_CACHE_ITEMS {
                self.cache.clear();
            }
            let result = self.run_check(root, style);
            self.cache.insert(style.to_owned(), result);
        }
        self.cache
            .get(style)
            .expect("semantic result inserted")
            .as_deref()
            .map_err(Clone::clone)
    }

    fn run_check(&self, root: &PathBuf, style: &str) -> Result<Vec<Value>, String> {
        let mut process = Command::new(&self.config.compiler);
        process
            .arg("--diagnostic-format")
            .arg("json")
            .arg("check")
            .arg("--style")
            .arg(style)
            .current_dir(root);
        if self.config.seed {
            process.arg("--seed");
        } else if let Some(config) = &self.config.config {
            process.arg("--config").arg(config);
        }
        let output = process
            .output()
            .map_err(|error| format!("cannot run pliego-cssc check: {error}"))?;
        if output.status.success() {
            if !output.stderr.is_empty() {
                return Err("successful pliego-cssc check wrote stderr".into());
            }
            return Ok(Vec::new());
        }
        let document: Value = serde_json::from_slice(&output.stderr)
            .map_err(|error| format!("invalid pliego-cssc diagnostic JSON: {error}"))?;
        if document.get("schemaVersion") != Some(&json!(1))
            || document.get("command") != Some(&json!("check"))
        {
            return Err("unsupported pliego-cssc diagnostic schema".into());
        }
        let diagnostics = document
            .get("diagnostics")
            .and_then(Value::as_array)
            .ok_or("pliego-cssc diagnostic document has no diagnostics")?;
        if diagnostics.is_empty() {
            return Err("failed pliego-cssc check returned no diagnostics".into());
        }
        Ok(diagnostics.clone())
    }
}

struct SemanticJob {
    uri: String,
    version: i64,
    text: String,
    root: PathBuf,
}

struct SemanticResult {
    uri: String,
    version: i64,
    diagnostics: Vec<Value>,
}

struct PendingSemantic {
    due: Instant,
    job: SemanticJob,
}

enum ProtocolEvent {
    Input(Result<Option<Value>, String>),
    Semantic(SemanticResult),
}

/// Serves framed JSON-RPC messages until `exit` or end-of-stream.
///
/// # Errors
///
/// Returns an error when an input frame or output write is invalid.
pub fn serve(
    mut input: impl BufRead + Send,
    mut output: impl Write,
    config: ServerConfig,
) -> Result<(), String> {
    let mut server = Server::new(config.clone());
    let mut pending = BTreeMap::<String, PendingSemantic>::new();
    let (event_sender, events) = mpsc::channel::<ProtocolEvent>();
    let (job_sender, jobs) = mpsc::channel::<SemanticJob>();
    spawn_semantic_worker(config, jobs, event_sender.clone());

    thread::scope(|scope| -> Result<(), String> {
        let input_sender = event_sender.clone();
        scope.spawn(move || {
            loop {
                let result = read_message(&mut input);
                let stop = match &result {
                    Ok(Some(message)) => {
                        message.get("method").and_then(Value::as_str) == Some("exit")
                    }
                    Ok(None) | Err(_) => true,
                };
                if input_sender.send(ProtocolEvent::Input(result)).is_err() || stop {
                    break;
                }
            }
        });
        drop(event_sender);

        loop {
            match receive_event(&events, &pending)? {
                Some(ProtocolEvent::Input(result)) => {
                    let Some(message) = result? else { break };
                    let method = message.get("method").and_then(Value::as_str).unwrap_or("");
                    let id = message.get("id").cloned();
                    let params = message.get("params").cloned().unwrap_or(Value::Null);
                    if method == "exit" {
                        break;
                    }
                    if let Some(id) = id {
                        let response = match handle_request(&mut server, method, &params) {
                            Ok(result) => json!({"jsonrpc":"2.0","id":id,"result":result}),
                            Err(error) => json!({
                                "jsonrpc":"2.0","id":id,
                                "error":{"code":-32602,"message":error}
                            }),
                        };
                        write_message(&mut output, &response)?;
                    } else {
                        if let Some(notification) =
                            handle_notification(&mut server, method, &params)?
                        {
                            write_message(&mut output, &notification)?;
                        }
                        update_semantic_schedule(&server, method, &params, &mut pending)?;
                    }
                }
                Some(ProtocolEvent::Semantic(result)) => {
                    if let Some(document) = server.documents.get(&result.uri) {
                        if document.version == result.version {
                            let mut diagnostics = local_diagnostics(&result.uri, &document.text);
                            diagnostics.extend(result.diagnostics);
                            write_message(
                                &mut output,
                                &diagnostic_message(&result.uri, document.version, &diagnostics),
                            )?;
                        }
                    }
                }
                None => dispatch_due_jobs(&job_sender, &mut pending)?,
            }
        }
        drop(job_sender);
        Ok(())
    })
}

fn spawn_semantic_worker(
    config: ServerConfig,
    jobs: Receiver<SemanticJob>,
    events: Sender<ProtocolEvent>,
) {
    thread::spawn(move || {
        let mut engine = SemanticEngine::new(config);
        while let Ok(job) = jobs.recv() {
            let diagnostics = semantic_document_diagnostics(&mut engine, &job);
            if events
                .send(ProtocolEvent::Semantic(SemanticResult {
                    uri: job.uri,
                    version: job.version,
                    diagnostics,
                }))
                .is_err()
            {
                break;
            }
        }
    });
}

fn receive_event(
    events: &Receiver<ProtocolEvent>,
    pending: &BTreeMap<String, PendingSemantic>,
) -> Result<Option<ProtocolEvent>, String> {
    let Some(due) = pending.values().map(|item| item.due).min() else {
        return events
            .recv()
            .map(Some)
            .map_err(|_| "protocol event loop closed".into());
    };
    match events.recv_timeout(due.saturating_duration_since(Instant::now())) {
        Ok(event) => Ok(Some(event)),
        Err(RecvTimeoutError::Timeout) => Ok(None),
        Err(RecvTimeoutError::Disconnected) => Err("protocol event loop closed".into()),
    }
}

fn dispatch_due_jobs(
    jobs: &Sender<SemanticJob>,
    pending: &mut BTreeMap<String, PendingSemantic>,
) -> Result<(), String> {
    let now = Instant::now();
    let due = pending
        .iter()
        .filter(|(_, item)| item.due <= now)
        .map(|(uri, _)| uri.clone())
        .collect::<Vec<_>>();
    for uri in due {
        let item = pending
            .remove(&uri)
            .ok_or("pending semantic job disappeared")?;
        jobs.send(item.job)
            .map_err(|_| "semantic worker closed".to_owned())?;
    }
    Ok(())
}

fn update_semantic_schedule(
    server: &Server,
    method: &str,
    params: &Value,
    pending: &mut BTreeMap<String, PendingSemantic>,
) -> Result<(), String> {
    if matches!(method, "textDocument/didOpen" | "textDocument/didChange") {
        let uri = string_field(
            params.get("textDocument").ok_or("missing textDocument")?,
            "uri",
        )?;
        let document = server.documents.get(uri).ok_or("document is not open")?;
        pending.insert(
            uri.to_owned(),
            PendingSemantic {
                due: Instant::now() + SEMANTIC_DEBOUNCE,
                job: SemanticJob {
                    uri: uri.to_owned(),
                    version: document.version,
                    text: document.text.clone(),
                    root: server.root.clone(),
                },
            },
        );
    } else if method == "textDocument/didClose" {
        let uri = string_field(
            params.get("textDocument").ok_or("missing textDocument")?,
            "uri",
        )?;
        pending.remove(uri);
    }
    Ok(())
}

fn handle_request(server: &mut Server, method: &str, params: &Value) -> Result<Value, String> {
    if server.shutdown && method != "shutdown" {
        return Err("server is shutting down".into());
    }
    match method {
        "initialize" => {
            if let Some(uri) = params.get("rootUri").and_then(Value::as_str) {
                server.root = file_uri_path(uri)?;
            }
            Ok(json!({
                "capabilities": {
                    "positionEncoding": "utf-16",
                    "textDocumentSync": {"openClose":true,"change":1},
                    "completionProvider": {"triggerCharacters":["-",":","["]},
                    "hoverProvider": true,
                    "definitionProvider": server.config.project_index.is_some(),
                    "documentFormattingProvider": true
                },
                "serverInfo":{"name":"pliego-css-lsp","version":VERSION}
            }))
        }
        "shutdown" => {
            server.shutdown = true;
            Ok(Value::Null)
        }
        "textDocument/completion" => completion(server, params),
        "textDocument/hover" => hover(server, params),
        "textDocument/definition" => definition(server, params),
        "textDocument/formatting" => formatting(server, params),
        _ => Err(format!("unsupported method `{method}`")),
    }
}

fn handle_notification(
    server: &mut Server,
    method: &str,
    params: &Value,
) -> Result<Option<Value>, String> {
    match method {
        "textDocument/didOpen" => {
            let document = params.get("textDocument").ok_or("missing textDocument")?;
            let uri = string_field(document, "uri")?.to_owned();
            server.documents.insert(
                uri.clone(),
                Document {
                    text: string_field(document, "text")?.to_owned(),
                    version: document.get("version").and_then(Value::as_i64).unwrap_or(0),
                },
            );
            Ok(Some(diagnostic_notification(server, &uri)?))
        }
        "textDocument/didChange" => {
            let identifier = params.get("textDocument").ok_or("missing textDocument")?;
            let uri = string_field(identifier, "uri")?.to_owned();
            let version = identifier
                .get("version")
                .and_then(Value::as_i64)
                .ok_or("missing document version")?;
            let changes = params
                .get("contentChanges")
                .and_then(Value::as_array)
                .ok_or("missing contentChanges")?;
            if changes.len() != 1 || changes[0].get("range").is_some() {
                return Err("full synchronization requires one range-free change".into());
            }
            let document = server
                .documents
                .get_mut(&uri)
                .ok_or("document is not open")?;
            if version <= document.version {
                return Err("document version did not increase".into());
            }
            string_field(&changes[0], "text")?.clone_into(&mut document.text);
            document.version = version;
            Ok(Some(diagnostic_notification(server, &uri)?))
        }
        "textDocument/didClose" => {
            let uri = string_field(
                params.get("textDocument").ok_or("missing textDocument")?,
                "uri",
            )?;
            server.documents.remove(uri);
            Ok(Some(json!({
                "jsonrpc":"2.0","method":"textDocument/publishDiagnostics",
                "params":{"uri":uri,"diagnostics":[]}
            })))
        }
        _ => Ok(None),
    }
}

fn diagnostic_notification(server: &Server, uri: &str) -> Result<Value, String> {
    let document = server.documents.get(uri).ok_or("document is not open")?;
    Ok(diagnostic_message(
        uri,
        document.version,
        &local_diagnostics(uri, &document.text),
    ))
}

fn diagnostic_message(uri: &str, version: i64, diagnostics: &[Value]) -> Value {
    json!({
        "jsonrpc":"2.0","method":"textDocument/publishDiagnostics",
        "params":{"uri":uri,"version":version,"diagnostics":diagnostics}
    })
}

fn local_diagnostics(uri: &str, source: &str) -> Vec<Value> {
    let report = match scan_source_named(uri, source) {
        Ok(report) => report,
        Err(error) => {
            return vec![diagnostic(source, error.range, "PCR001", &error.message, 1)];
        }
    };
    let mut values = report
        .diagnostics
        .iter()
        .map(|item| diagnostic(source, item.range, item.code, &item.message, 1))
        .collect::<Vec<_>>();
    visit_literals(&report, |role, literal| {
        match parse_style_list(&literal.value) {
            Ok(parsed) => {
                let expected = format_style_list(&parsed);
                if expected != literal.value {
                    let mut value = diagnostic(
                        source,
                        literal.range,
                        "FMT001",
                        &format!("{role} utility literal should be {expected:?}"),
                        2,
                    );
                    value["data"] = json!({"replacement":expected});
                    values.push(value);
                }
            }
            Err(error) => values.push(diagnostic(
                source,
                literal.range,
                error.code.as_str(),
                &error.message,
                1,
            )),
        }
    });
    values
}

fn semantic_document_diagnostics(engine: &mut SemanticEngine, job: &SemanticJob) -> Vec<Value> {
    let Ok(report) = scan_source_named(&job.uri, &job.text) else {
        return Vec::new();
    };
    let mut values = Vec::new();
    let mut semantic_checks = 0;
    let mut semantic_limit_reported = false;
    visit_literals(&report, |_, literal| {
        if parse_style_list(&literal.value).is_err() {
            return;
        }
        if semantic_checks < MAX_SEMANTIC_CHECKS_PER_DOCUMENT {
            values.extend(semantic_diagnostics(engine, &job.root, &job.text, literal));
            semantic_checks += 1;
        } else if !semantic_limit_reported {
            values.push(diagnostic(
                &job.text,
                literal.range,
                "PCL002",
                "semantic diagnostic literal limit exceeded",
                1,
            ));
            semantic_limit_reported = true;
        }
    });
    values
}

fn semantic_diagnostics(
    engine: &mut SemanticEngine,
    root: &PathBuf,
    source: &str,
    literal: &StyleLiteral,
) -> Vec<Value> {
    let findings = match engine.findings(root, &literal.value) {
        Ok(findings) => findings,
        Err(error) => {
            return vec![diagnostic(
                source,
                literal.range,
                "PCL001",
                &format!("compiler-backed diagnostics unavailable: {error}"),
                1,
            )];
        }
    };
    let mut diagnostics = Vec::with_capacity(findings.len());
    for finding in findings {
        match semantic_diagnostic(source, literal, finding) {
            Ok(diagnostic) => diagnostics.push(diagnostic),
            Err(error) => {
                return vec![diagnostic(
                    source,
                    literal.range,
                    "PCL001",
                    &format!("invalid compiler diagnostic: {error}"),
                    1,
                )];
            }
        }
    }
    diagnostics
}

fn semantic_diagnostic(
    source: &str,
    literal: &StyleLiteral,
    finding: &Value,
) -> Result<Value, String> {
    let code = string_field(finding, "code")?;
    let message = string_field(finding, "message")?;
    let severity = match string_field(finding, "severity")? {
        "error" => 1,
        "warning" => 2,
        "information" => 3,
        "hint" => 4,
        _ => return Err("unsupported severity".into()),
    };
    let origin = finding.get("origin").ok_or("missing origin")?;
    if string_field(origin, "kind")? != "cli"
        || string_field(origin, "label")? != "explicit-style-1"
    {
        return Err("semantic diagnostic has an unexpected origin".into());
    }
    let range =
        if let Some(style_range) = finding.get("styleRange").filter(|value| !value.is_null()) {
            let start = usize::try_from(
                style_range
                    .get("byteStart")
                    .and_then(Value::as_u64)
                    .ok_or("missing style range start")?,
            )
            .map_err(|_| "style range start is too large")?;
            let end = usize::try_from(
                style_range
                    .get("byteEnd")
                    .and_then(Value::as_u64)
                    .ok_or("missing style range end")?,
            )
            .map_err(|_| "style range end is too large")?;
            semantic_source_range(source, literal, start, end)?
        } else {
            source_range_to_lsp(source, literal.range)
        };
    let mut diagnostic = lsp_diagnostic(&range, code, message, severity);
    diagnostic["data"] = json!({
        "category":finding.get("category").cloned().unwrap_or(Value::Null),
        "suggestion":finding.get("suggestion").cloned().unwrap_or(Value::Null),
        "replacement":finding.get("replacement").cloned().unwrap_or(Value::Null)
    });
    Ok(diagnostic)
}

fn semantic_source_range(
    source: &str,
    literal: &StyleLiteral,
    start: usize,
    end: usize,
) -> Result<Value, String> {
    if start >= end
        || end > literal.value.len()
        || !literal.value.is_char_boundary(start)
        || !literal.value.is_char_boundary(end)
    {
        return Err("semantic diagnostic style range is invalid".into());
    }
    let token = source
        .get(literal.range.byte_range())
        .ok_or("literal range is outside the source")?;
    let (content_start, content_end) = literal_content(token).ok_or("unsupported literal token")?;
    let absolute_start = literal.range.start.byte + content_start;
    let raw = source
        .get(absolute_start..literal.range.start.byte + content_end)
        .ok_or("literal content is outside the source")?;
    if raw.contains('\\') || raw != literal.value {
        return Ok(source_range_to_lsp(source, literal.range));
    }
    Ok(byte_range_to_lsp(
        source,
        absolute_start + start..absolute_start + end,
    ))
}

fn completion(server: &mut Server, params: &Value) -> Result<Value, String> {
    let (uri, line, character) = text_position(params)?;
    let text = server
        .documents
        .get(uri)
        .ok_or("document is not open")?
        .text
        .clone();
    let context = utility_context(uri, &text, line, character)?
        .ok_or("cursor is not in a utility literal")?;
    let prefix = &context.value[context.word_start..context.cursor];
    let variant_end = prefix.rfind(':').map_or(0, |index| index + 1);
    let variant = &prefix[..variant_end];
    let needle = &prefix[variant_end..];
    let range = byte_range_to_lsp(&text, context.source_word_start..context.source_word_end);
    let mut items = Vec::new();
    for item in server.catalog()? {
        let example = item.get("example").and_then(Value::as_str).unwrap_or("");
        let pattern = item.get("pattern").and_then(Value::as_str).unwrap_or("");
        let match_name = item.get("matchName").and_then(Value::as_str).unwrap_or("");
        if example.is_empty()
            || (!example.starts_with(needle)
                && !pattern.starts_with(needle)
                && !match_name.starts_with(needle))
        {
            continue;
        }
        items.push(json!({
            "label":example,
            "kind":14,
            "detail":pattern,
            "documentation":{"kind":"markdown","value":item.get("summary").and_then(Value::as_str).unwrap_or("")},
            "filterText":format!("{variant}{example}"),
            "textEdit":{"range":range,"newText":format!("{variant}{example}")}
        }));
    }
    Ok(json!({"isIncomplete":false,"items":items}))
}

fn hover(server: &mut Server, params: &Value) -> Result<Value, String> {
    let (uri, line, character) = text_position(params)?;
    let text = &server
        .documents
        .get(uri)
        .ok_or("document is not open")?
        .text;
    let context =
        utility_context(uri, text, line, character)?.ok_or("cursor is not in a utility literal")?;
    let utility = &context.value[context.word_start..context.word_end];
    if utility.is_empty() {
        return Ok(Value::Null);
    }
    let explained = server.tool("explain", &["--style", utility])?;
    if explained.get("schemaVersion") != Some(&json!(2)) {
        return Err("unsupported explain schema".into());
    }
    let item = explained
        .get("utilities")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .ok_or("explain returned no utility")?;
    let summary = item.get("summary").and_then(Value::as_str).unwrap_or("");
    let pattern = item.get("pattern").and_then(Value::as_str).unwrap_or("");
    let css = explained.get("css").and_then(Value::as_str).unwrap_or("");
    Ok(json!({
        "contents":{"kind":"markdown","value":format!("**{utility}** — `{pattern}`\n\n{summary}\n\n```css\n{css}\n```")},
        "range":byte_range_to_lsp(text, context.source_word_start..context.source_word_end)
    }))
}

fn definition(server: &Server, params: &Value) -> Result<Value, String> {
    let index_path = server
        .config
        .project_index
        .as_deref()
        .ok_or("Project Index navigation is not configured")?;
    let (uri, line, character) = text_position(params)?;
    let text = &server
        .documents
        .get(uri)
        .ok_or("document is not open")?
        .text;
    let context =
        utility_context(uri, text, line, character)?.ok_or("cursor is not in a utility literal")?;
    let document_path = file_uri_path(uri)?;
    project_index::definitions(&project_index::NavigationRequest {
        root: &server.root,
        index_path,
        document_path: &document_path,
        document_text: text,
        literal_start: context.literal_start,
        literal_end: context.literal_end,
        origin_range: byte_range_to_lsp(text, context.source_word_start..context.source_word_end),
    })
}

fn formatting(server: &Server, params: &Value) -> Result<Value, String> {
    let uri = string_field(
        params.get("textDocument").ok_or("missing textDocument")?,
        "uri",
    )?;
    let source = &server
        .documents
        .get(uri)
        .ok_or("document is not open")?
        .text;
    let report = scan_source_named(uri, source).map_err(|error| error.to_string())?;
    if !report.diagnostics.is_empty() {
        return Err("unsupported PliegoCSS macro invocation".into());
    }
    let mut edits = Vec::new();
    let mut failure = None;
    visit_literals(&report, |_, literal| {
        let parsed = match parse_style_list(&literal.value) {
            Ok(parsed) => parsed,
            Err(error) => {
                failure = Some(error.to_string());
                return;
            }
        };
        let expected = format_style_list(&parsed);
        if expected != literal.value {
            edits.push(json!({
                "range":source_range_to_lsp(source, literal.range),
                "newText":format!("{expected:?}")
            }));
        }
    });
    if let Some(failure) = failure {
        return Err(failure);
    }
    Ok(Value::Array(edits))
}

fn visit_literals(report: &ScanReport, mut visit: impl FnMut(&str, &StyleLiteral)) {
    for invocation in &report.invocations {
        match &invocation.kind {
            InvocationKind::Pc(pc) => visit("pc", &pc.style),
            InvocationKind::Pcx(pcx) => {
                visit("pcx base", &pcx.base);
                for (clause_index, clause) in pcx.clauses.iter().enumerate() {
                    for (branch_index, branch) in clause.branches.iter().enumerate() {
                        visit(
                            &format!(
                                "pcx clause {} branch {}",
                                clause_index + 1,
                                branch_index + 1
                            ),
                            &branch.style,
                        );
                    }
                }
            }
        }
    }
}

struct UtilityContext {
    value: String,
    cursor: usize,
    word_start: usize,
    word_end: usize,
    source_word_start: usize,
    source_word_end: usize,
    literal_start: usize,
    literal_end: usize,
}

fn utility_context(
    uri: &str,
    source: &str,
    line: usize,
    character: usize,
) -> Result<Option<UtilityContext>, String> {
    let cursor = lsp_position_to_byte(source, line, character)?;
    let report = scan_source_named(uri, source).map_err(|error| error.to_string())?;
    let mut found = None;
    visit_literals(&report, |_, literal| {
        if found.is_none() && literal.range.start.byte <= cursor && cursor <= literal.range.end.byte
        {
            found = literal_context(source, literal, cursor);
        }
    });
    Ok(found)
}

fn literal_context(source: &str, literal: &StyleLiteral, cursor: usize) -> Option<UtilityContext> {
    let token = source.get(literal.range.byte_range())?;
    let (content_start, content_end) = literal_content(token)?;
    let absolute_start = literal.range.start.byte.checked_add(content_start)?;
    let absolute_end = literal.range.start.byte.checked_add(content_end)?;
    if cursor < absolute_start || cursor > absolute_end {
        return None;
    }
    let raw_content = source.get(absolute_start..absolute_end)?;
    if raw_content.contains('\\') || raw_content != literal.value {
        return None;
    }
    let decoded_cursor = cursor - absolute_start;
    if !literal.value.is_char_boundary(decoded_cursor) {
        return None;
    }
    let word_start = literal.value[..decoded_cursor]
        .rfind(char::is_whitespace)
        .map_or(0, |index| {
            index
                + literal.value[index..]
                    .chars()
                    .next()
                    .map_or(0, char::len_utf8)
        });
    let word_end = literal.value[decoded_cursor..]
        .find(char::is_whitespace)
        .map_or(literal.value.len(), |index| decoded_cursor + index);
    Some(UtilityContext {
        value: literal.value.clone(),
        cursor: decoded_cursor,
        word_start,
        word_end,
        source_word_start: absolute_start + word_start,
        source_word_end: absolute_start + word_end,
        literal_start: literal.range.start.byte,
        literal_end: literal.range.end.byte,
    })
}

fn literal_content(token: &str) -> Option<(usize, usize)> {
    if token.starts_with('"') && token.ends_with('"') {
        return Some((1, token.len() - 1));
    }
    if !token.starts_with('r') {
        return None;
    }
    let quote = token.find('"')?;
    let hashes = &token[1..quote];
    if !hashes.chars().all(|character| character == '#') {
        return None;
    }
    let suffix = format!("\"{hashes}");
    token.strip_suffix(&suffix)?;
    Some((quote + 1, token.len() - suffix.len()))
}

fn diagnostic(source: &str, range: SourceRange, code: &str, message: &str, severity: u8) -> Value {
    lsp_diagnostic(&source_range_to_lsp(source, range), code, message, severity)
}

fn lsp_diagnostic(range: &Value, code: &str, message: &str, severity: u8) -> Value {
    json!({
        "range":range,
        "severity":severity,"code":code,"source":"pliegocss","message":message
    })
}

fn source_range_to_lsp(source: &str, range: SourceRange) -> Value {
    byte_range_to_lsp(source, range.byte_range())
}

fn byte_range_to_lsp(source: &str, range: std::ops::Range<usize>) -> Value {
    json!({
        "start":byte_to_lsp_position(source, range.start),
        "end":byte_to_lsp_position(source, range.end)
    })
}

fn byte_to_lsp_position(source: &str, byte: usize) -> Value {
    let prefix = source.get(..byte).unwrap_or(source);
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count();
    let line_start = prefix.rfind('\n').map_or(0, |index| index + 1);
    let character = prefix[line_start..].encode_utf16().count();
    json!({"line":line,"character":character})
}

fn lsp_position_to_byte(source: &str, line: usize, character: usize) -> Result<usize, String> {
    let line_start = if line == 0 {
        0
    } else {
        source
            .match_indices('\n')
            .nth(line - 1)
            .map(|(index, _)| index + 1)
            .ok_or("line is outside the document")?
    };
    let line_end = source[line_start..]
        .find('\n')
        .map_or(source.len(), |index| line_start + index);
    let mut units = 0;
    for (offset, scalar) in source[line_start..line_end].char_indices() {
        if units == character {
            return Ok(line_start + offset);
        }
        units += scalar.len_utf16();
        if units > character {
            return Err("position splits a UTF-16 surrogate pair".into());
        }
    }
    if units == character {
        Ok(line_end)
    } else {
        Err("character is outside the line".into())
    }
}

fn text_position(params: &Value) -> Result<(&str, usize, usize), String> {
    let uri = string_field(
        params.get("textDocument").ok_or("missing textDocument")?,
        "uri",
    )?;
    let position = params.get("position").ok_or("missing position")?;
    let line = usize::try_from(
        position
            .get("line")
            .and_then(Value::as_u64)
            .ok_or("missing line")?,
    )
    .map_err(|_| "line is too large")?;
    let character = usize::try_from(
        position
            .get("character")
            .and_then(Value::as_u64)
            .ok_or("missing character")?,
    )
    .map_err(|_| "character is too large")?;
    Ok((uri, line, character))
}

fn string_field<'a>(value: &'a Value, name: &str) -> Result<&'a str, String> {
    value
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("missing {name}"))
}

fn file_uri_path(uri: &str) -> Result<PathBuf, String> {
    let encoded = uri
        .strip_prefix("file://")
        .ok_or("rootUri must use file:")?;
    let decoded = percent_decode(encoded)?;
    #[cfg(windows)]
    let decoded = decoded
        .strip_prefix('/')
        .unwrap_or(&decoded)
        .replace('/', "\\");
    Ok(PathBuf::from(decoded))
}

fn percent_decode(value: &str) -> Result<String, String> {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let pair = bytes
                .get(index + 1..index + 3)
                .ok_or("truncated URI escape")?;
            let pair = std::str::from_utf8(pair).map_err(|_| "invalid URI escape")?;
            output.push(u8::from_str_radix(pair, 16).map_err(|_| "invalid URI escape")?);
            index += 3;
        } else {
            output.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(output).map_err(|_| "URI is not UTF-8".into())
}

fn read_message(input: &mut impl BufRead) -> Result<Option<Value>, String> {
    let mut length = None;
    loop {
        let mut line = String::new();
        if input
            .read_line(&mut line)
            .map_err(|error| error.to_string())?
            == 0
        {
            return if length.is_none() {
                Ok(None)
            } else {
                Err("truncated headers".into())
            };
        }
        if line == "\r\n" || line == "\n" {
            break;
        }
        if let Some(value) = line
            .trim_end()
            .strip_prefix("Content-Length:")
            .map(str::trim)
        {
            length = Some(
                value
                    .parse::<usize>()
                    .map_err(|_| "invalid Content-Length")?,
            );
        }
    }
    let length = length.ok_or("missing Content-Length")?;
    if length > 16 * 1024 * 1024 {
        return Err("message exceeds 16 MiB".into());
    }
    let mut bytes = vec![0; length];
    input
        .read_exact(&mut bytes)
        .map_err(|error| error.to_string())?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|error| error.to_string())
}

fn write_message(output: &mut impl Write, value: &Value) -> Result<(), String> {
    let bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    write!(output, "Content-Length: {}\r\n\r\n", bytes.len()).map_err(|error| error.to_string())?;
    output
        .write_all(&bytes)
        .map_err(|error| error.to_string())?;
    output.flush().map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_positions_round_trip_astral_scalars() {
        let source = "fn x() { let _ = \"😀 flex\"; }\n";
        let byte = source.find("flex").unwrap();
        let position = byte_to_lsp_position(source, byte);
        assert_eq!(position, json!({"line":0,"character":21}));
        assert_eq!(lsp_position_to_byte(source, 0, 21).unwrap(), byte);
        assert!(lsp_position_to_byte(source, 0, 19).is_err());
    }

    #[test]
    fn literal_context_is_exact_and_fails_closed_for_escapes() {
        let source = "fn x(){let _=pc!(r#\"flex  gap-4\"#);}";
        let report = scan_source_named("x.rs", source).unwrap();
        let literal = match &report.invocations[0].kind {
            InvocationKind::Pc(pc) => &pc.style,
            InvocationKind::Pcx(_) => unreachable!(),
        };
        let cursor = source.find("gap").unwrap() + 2;
        let context = literal_context(source, literal, cursor).unwrap();
        assert_eq!(
            &context.value[context.word_start..context.word_end],
            "gap-4"
        );

        let escaped = "fn x(){let _=pc!(\"flex\\tgap-4\");}";
        let report = scan_source_named("x.rs", escaped).unwrap();
        let literal = match &report.invocations[0].kind {
            InvocationKind::Pc(pc) => &pc.style,
            InvocationKind::Pcx(_) => unreachable!(),
        };
        assert!(literal_context(escaped, literal, escaped.find("gap").unwrap()).is_none());
    }

    #[test]
    fn semantic_ranges_are_exact_and_escaped_literals_fail_to_the_whole_token() {
        let finding = json!({
            "code":"PCS001","category":"style","severity":"error",
            "message":"unknown utility `unknown`","suggestion":null,
            "origin":{"kind":"cli","label":"explicit-style-1"},
            "range":null,"styleRange":{"byteStart":5,"byteEnd":12},"replacement":null
        });
        let source = "fn x(){let _=pc!(\"flex unknown\");}";
        let report = scan_source_named("x.rs", source).unwrap();
        let literal = match &report.invocations[0].kind {
            InvocationKind::Pc(pc) => &pc.style,
            InvocationKind::Pcx(_) => unreachable!(),
        };
        let exact = semantic_diagnostic(source, literal, &finding).unwrap();
        assert_eq!(
            exact["range"],
            byte_range_to_lsp(
                source,
                source.find("unknown").unwrap()..source.find("unknown").unwrap() + 7
            )
        );

        let escaped = "fn x(){let _=pc!(\"flex\\tunknown\");}";
        let report = scan_source_named("x.rs", escaped).unwrap();
        let literal = match &report.invocations[0].kind {
            InvocationKind::Pc(pc) => &pc.style,
            InvocationKind::Pcx(_) => unreachable!(),
        };
        let whole = semantic_diagnostic(escaped, literal, &finding).unwrap();
        assert_eq!(whole["range"], source_range_to_lsp(escaped, literal.range));
    }

    #[test]
    fn framing_round_trips_json_rpc() {
        let message = json!({"jsonrpc":"2.0","id":1,"method":"shutdown"});
        let mut bytes = Vec::new();
        write_message(&mut bytes, &message).unwrap();
        let mut input = BufReader::new(bytes.as_slice());
        assert_eq!(read_message(&mut input).unwrap(), Some(message));
        assert_eq!(read_message(&mut input).unwrap(), None);
    }

    #[test]
    fn percent_decodes_file_paths() {
        assert_eq!(
            percent_decode("/tmp/Pliego%20CSS").unwrap(),
            "/tmp/Pliego CSS"
        );
        assert!(percent_decode("%ZZ").is_err());
    }

    #[test]
    fn completion_replaces_the_complete_existing_utility() {
        let uri = "file:///workspace/view.rs";
        let text = "fn view(){let _=pc!(\"flex gap-4\");}";
        let mut server = Server::new(ServerConfig::default());
        server.documents.insert(
            uri.into(),
            Document {
                text: text.into(),
                version: 1,
            },
        );
        server.catalog = Some(vec![json!({
            "example":"gap-4","pattern":"gap-{space}","matchName":"gap",
            "summary":"Set gap."
        })]);
        let result = completion(
            &mut server,
            &json!({
                "textDocument":{"uri":uri},
                "position":{"line":0,"character":text.find("gap").unwrap() + 3}
            }),
        )
        .unwrap();
        let edit = &result["items"][0]["textEdit"];
        assert_eq!(edit["newText"], "gap-4");
        assert_eq!(edit["range"]["start"]["character"], 26);
        assert_eq!(edit["range"]["end"]["character"], 31);
    }

    #[test]
    fn formatting_returns_whole_literal_edits_without_mutating_the_buffer() {
        let uri = "file:///workspace/view.rs";
        let text = "fn view(){let _=pc!(\" flex   gap-4 \");}";
        let mut server = Server::new(ServerConfig::default());
        server.documents.insert(
            uri.into(),
            Document {
                text: text.into(),
                version: 1,
            },
        );
        let result = formatting(
            &server,
            &json!({"textDocument":{"uri":uri},"options":{"tabSize":4,"insertSpaces":true}}),
        )
        .unwrap();
        assert_eq!(result[0]["newText"], "\"flex gap-4\"");
        assert_eq!(server.documents[uri].text, text);
    }

    #[test]
    fn stdio_session_initializes_publishes_formats_and_shuts_down() {
        let uri = "file:///workspace/view.rs";
        let mut input = Vec::new();
        for message in [
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
            json!({
                "jsonrpc":"2.0","method":"textDocument/didOpen","params":{"textDocument":{
                    "uri":uri,"languageId":"rust","version":1,
                    "text":"fn view(){let _=pc!(\" flex   gap-4 \");}"
                }}
            }),
            json!({
                "jsonrpc":"2.0","id":2,"method":"textDocument/formatting","params":{
                    "textDocument":{"uri":uri},"options":{"tabSize":4,"insertSpaces":true}
                }
            }),
            json!({"jsonrpc":"2.0","id":3,"method":"shutdown","params":null}),
            json!({"jsonrpc":"2.0","method":"exit","params":null}),
        ] {
            write_message(&mut input, &message).unwrap();
        }
        let mut output = Vec::new();
        serve(
            BufReader::new(input.as_slice()),
            &mut output,
            ServerConfig::default(),
        )
        .unwrap();
        let mut output = BufReader::new(output.as_slice());
        let initialize = read_message(&mut output).unwrap().unwrap();
        let published = read_message(&mut output).unwrap().unwrap();
        let formatted = read_message(&mut output).unwrap().unwrap();
        let shutdown = read_message(&mut output).unwrap().unwrap();
        assert_eq!(
            initialize["result"]["capabilities"]["positionEncoding"],
            "utf-16"
        );
        assert_eq!(published["params"]["diagnostics"][0]["code"], "FMT001");
        assert_eq!(formatted["result"][0]["newText"], "\"flex gap-4\"");
        assert!(shutdown["result"].is_null());
        assert!(read_message(&mut output).unwrap().is_none());
    }
}
