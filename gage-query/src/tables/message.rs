use std::any::Any;
use std::fmt::{self, Formatter};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use chrono::DateTime;
use datafusion::arrow::array::{Int64Builder, StringBuilder, TimestampMillisecondBuilder};
use datafusion::arrow::compute::SortOptions;
use datafusion::arrow::datatypes::{DataType, Field, Schema, SchemaRef, TimeUnit};
use datafusion::arrow::record_batch::RecordBatch;
use datafusion::catalog::Session;
use datafusion::datasource::{TableProvider, TableType};
use datafusion::error::Result;
use datafusion::execution::context::TaskContext;
use datafusion::physical_expr::expressions::col;
use datafusion::physical_expr::{EquivalenceProperties, PhysicalSortExpr};
use datafusion::physical_plan::execution_plan::{Boundedness, EmissionType};
use datafusion::physical_plan::memory::MemoryStream;
use datafusion::physical_plan::{
    DisplayAs, DisplayFormatType, ExecutionPlan, Partitioning, PlanProperties,
    SendableRecordBatchStream,
};
use datafusion::prelude::*;
use gage_claude::entry::{entry_attachment_blocks, entry_subtype, split_ide_tags};
use gage_claude::session::{SessionInfo, SessionListBuilder};
use gage_claude::session_reader::SessionReader;

use super::SessionSource;
use crate::filter::{self, IdFilter};

fn message_schema() -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("session_id", DataType::Utf8, false),
        Field::new("line", DataType::Int64, false),
        Field::new("uuid", DataType::Utf8, true),
        Field::new("type", DataType::Utf8, false),
        Field::new("subtype", DataType::Utf8, true),
        Field::new("text", DataType::Utf8, false),
        Field::new(
            "timestamp",
            DataType::Timestamp(TimeUnit::Millisecond, Some("UTC".into())),
            true,
        ),
        Field::new("attachments", DataType::Utf8, true),
        Field::new("ide_tags", DataType::Utf8, true),
        Field::new("raw", DataType::Utf8, false),
    ]))
}

#[derive(Debug, Clone)]
pub struct MessageTable {
    source: SessionSource,
    schema: SchemaRef,
}

impl MessageTable {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            source: SessionSource::Root(root.into()),
            schema: message_schema(),
        }
    }

    /// Build a `MessageTable` scoped to one session file.
    pub fn with_session(session_id: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            source: SessionSource::SingleSession {
                session_id: session_id.into(),
                path: path.into(),
            },
            schema: message_schema(),
        }
    }
}

#[async_trait]
impl TableProvider for MessageTable {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn schema(&self) -> SchemaRef {
        self.schema.clone()
    }

    fn table_type(&self) -> TableType {
        TableType::Base
    }

    fn supports_filters_pushdown(
        &self,
        filters: &[&Expr],
    ) -> Result<Vec<datafusion::logical_expr::TableProviderFilterPushDown>> {
        let support = filters
            .iter()
            .map(|f| filter::pushdown(f, "session_id"))
            .collect();
        Ok(support)
    }

    async fn scan(
        &self,
        _state: &dyn Session,
        projection: Option<&Vec<usize>>,
        filters: &[Expr],
        limit: Option<usize>,
    ) -> Result<Arc<dyn ExecutionPlan>> {
        let session_id_filter = IdFilter::new(filters, "session_id")?;
        let projected_schema = match projection {
            Some(indices) => Arc::new(self.schema.project(indices)?),
            None => self.schema.clone(),
        };
        Ok(Arc::new(MessageExec::new(
            self.schema.clone(),
            projected_schema,
            self.source.clone(),
            projection.cloned(),
            session_id_filter,
            limit,
        )))
    }
}

#[derive(Debug, Clone)]
struct MessageExec {
    source: SessionSource,
    full_schema: SchemaRef,
    projected_schema: SchemaRef,
    projection: Option<Vec<usize>>,
    session_id_filter: Option<IdFilter>,
    limit: Option<usize>,
    properties: PlanProperties,
}

impl MessageExec {
    fn new(
        full_schema: SchemaRef,
        projected_schema: SchemaRef,
        source: SessionSource,
        projection: Option<Vec<usize>>,
        session_id_filter: Option<IdFilter>,
        limit: Option<usize>,
    ) -> Self {
        let eq_properties = line_ordered_eq_properties(&projected_schema);
        let properties = PlanProperties::new(
            eq_properties,
            Partitioning::UnknownPartitioning(1),
            EmissionType::Incremental,
            Boundedness::Bounded,
        );
        Self {
            source,
            full_schema,
            projected_schema,
            projection,
            session_id_filter,
            limit,
            properties,
        }
    }
}

/// Build `EquivalenceProperties` advertising `[line ASC]` when the
/// projection retains the `line` column. `SessionReader` emits rows
/// top-to-bottom, so this ordering is the natural one. Advertising it
/// lets DataFusion's `EnforceSorting` rule remove a redundant
/// `ORDER BY line` from author SQL at plan time.
fn line_ordered_eq_properties(projected_schema: &SchemaRef) -> EquivalenceProperties {
    match col("line", projected_schema) {
        Ok(line_expr) => {
            let sort = PhysicalSortExpr {
                expr: line_expr,
                options: SortOptions {
                    descending: false,
                    nulls_first: false,
                },
            };
            EquivalenceProperties::new_with_orderings(projected_schema.clone(), [[sort]])
        }
        Err(_) => EquivalenceProperties::new(projected_schema.clone()),
    }
}

impl DisplayAs for MessageExec {
    fn fmt_as(&self, _t: DisplayFormatType, f: &mut Formatter) -> fmt::Result {
        write!(f, "MessageExec")
    }
}

/// Extract the text representation of a raw session entry.
///
/// Extracts text from `message.content` blocks (text, thinking,
/// tool_use, tool_result) and joins them with `\n\n`. Returns `None`
/// for non-message entries or messages with no text content.
///
/// The returned text may contain leading IDE tags — callers that need
/// them separated should pass the result through `split_ide_tags`.
pub fn entry_text(entry: &serde_json::Value) -> Option<String> {
    let type_str = match entry.get("type").and_then(|v| v.as_str()) {
        Some(t @ ("user" | "assistant")) => t,
        _ => return None,
    };

    let content = match entry.get("message").and_then(|m| m.get("content")) {
        Some(serde_json::Value::Array(arr)) => arr.clone(),
        Some(serde_json::Value::String(s)) => {
            vec![serde_json::json!({"type": "text", "text": s})]
        }
        _ => return None,
    };

    let mut texts: Vec<String> = Vec::new();

    for block in &content {
        let block_type = match block.get("type").and_then(|v| v.as_str()) {
            Some(t) => t,
            None => continue,
        };

        match (type_str, block_type) {
            (_, "text") => {
                if let Some(t) = block.get("text").and_then(|v| v.as_str()) {
                    texts.push(t.to_string());
                }
            }
            ("assistant", "thinking") => {
                if let Some(t) = block.get("thinking").and_then(|v| v.as_str()) {
                    texts.push(t.to_string());
                }
            }
            ("assistant", "tool_use") => {
                let name = block.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let input = block
                    .get("input")
                    .and_then(|v| v.as_object())
                    .cloned()
                    .unwrap_or_default();
                texts.push(format_tool_call_text(name, &input));
            }
            ("user", "tool_result") => {
                texts.push(tool_result_text(block));
            }
            _ => {}
        }
    }

    if texts.is_empty() {
        None
    } else {
        Some(texts.join("\n\n"))
    }
}

fn format_tool_call_text(name: &str, input: &serde_json::Map<String, serde_json::Value>) -> String {
    // Build a YAML mapping: tool name as key, arguments as nested map
    let mut mapping = serde_yaml::Mapping::new();
    let args_mapping: serde_yaml::Mapping = input
        .iter()
        .map(|(k, v)| (serde_yaml::Value::String(k.clone()), json_to_yaml(v)))
        .collect();
    mapping.insert(
        serde_yaml::Value::String(name.to_string()),
        serde_yaml::Value::Mapping(args_mapping),
    );
    let yaml = serde_yaml::to_string(&mapping).unwrap_or_default();
    // serde_yaml adds a trailing newline; trim it for the header portion
    yaml.trim_end().to_string()
}

fn json_to_yaml(v: &serde_json::Value) -> serde_yaml::Value {
    match v {
        serde_json::Value::Null => serde_yaml::Value::Null,
        serde_json::Value::Bool(b) => serde_yaml::Value::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                serde_yaml::Value::Number(i.into())
            } else if let Some(f) = n.as_f64() {
                serde_yaml::Value::Number(serde_yaml::Number::from(f))
            } else {
                serde_yaml::Value::String(n.to_string())
            }
        }
        serde_json::Value::String(s) => serde_yaml::Value::String(s.clone()),
        serde_json::Value::Array(arr) => {
            serde_yaml::Value::Sequence(arr.iter().map(json_to_yaml).collect())
        }
        serde_json::Value::Object(map) => {
            let m: serde_yaml::Mapping = map
                .iter()
                .map(|(k, v)| (serde_yaml::Value::String(k.clone()), json_to_yaml(v)))
                .collect();
            serde_yaml::Value::Mapping(m)
        }
    }
}

struct MessageBuilders {
    session_ids: StringBuilder,
    lines: Int64Builder,
    uuids: StringBuilder,
    types: StringBuilder,
    subtypes: StringBuilder,
    texts: StringBuilder,
    timestamps: TimestampMillisecondBuilder,
    attachments: StringBuilder,
    ide_tags: StringBuilder,
    raws: StringBuilder,
}

impl MessageBuilders {
    fn new() -> Self {
        Self {
            session_ids: StringBuilder::new(),
            lines: Int64Builder::new(),
            uuids: StringBuilder::new(),
            types: StringBuilder::new(),
            subtypes: StringBuilder::new(),
            texts: StringBuilder::new(),
            timestamps: TimestampMillisecondBuilder::new(),
            attachments: StringBuilder::new(),
            ide_tags: StringBuilder::new(),
            raws: StringBuilder::new(),
        }
    }

    fn finish(mut self, schema: SchemaRef) -> datafusion::error::Result<RecordBatch> {
        Ok(RecordBatch::try_new(
            schema,
            vec![
                Arc::new(self.session_ids.finish()),
                Arc::new(self.lines.finish()),
                Arc::new(self.uuids.finish()),
                Arc::new(self.types.finish()),
                Arc::new(self.subtypes.finish()),
                Arc::new(self.texts.finish()),
                Arc::new(self.timestamps.finish().with_timezone("UTC")),
                Arc::new(self.attachments.finish()),
                Arc::new(self.ide_tags.finish()),
                Arc::new(self.raws.finish()),
            ],
        )?)
    }
}

/// Extract tool_result text from a content block's `content` field,
/// which may be a string or an array of content items.
fn tool_result_text(block: &serde_json::Value) -> String {
    match block.get("content") {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Array(arr)) => {
            let mut parts = Vec::new();
            for item in arr {
                if let Some(t) = item.get("text").and_then(|v| v.as_str()) {
                    parts.push(t);
                }
            }
            parts.join("\n")
        }
        _ => String::new(),
    }
}

/// Process a single session file into message rows (one row per entry).
///
/// Text content blocks are joined with `\n\n` into the `text` column.
/// Everything else (thinking, tool_use, tool_result, image) goes into
/// the `attachments` column as a JSON array.
fn process_session(
    session_id: &str,
    path: &Path,
    limit: Option<usize>,
    b: &mut MessageBuilders,
) -> usize {
    let reader = match SessionReader::open(path) {
        Ok(r) => r,
        Err(_) => return 0,
    };

    let mut count = 0;
    for result in reader {
        if limit.is_some_and(|l| count >= l) {
            break;
        }
        let (line_num, entry) = match result {
            Ok(pair) => pair,
            Err(_) => continue,
        };

        let type_str = match entry.get("type").and_then(|v| v.as_str()) {
            Some(t @ ("user" | "assistant")) => t,
            _ => continue,
        };

        let entry_uuid = entry.get("uuid").and_then(|v| v.as_str()).map(String::from);
        let ts_ms = match entry.get("timestamp").and_then(|v| v.as_str()) {
            Some(s) => match DateTime::parse_from_rfc3339(s) {
                Ok(dt) => Some(dt.timestamp_millis()),
                Err(e) => {
                    tracing::warn!(
                        session_id,
                        line = line_num,
                        timestamp = s,
                        "unparseable message timestamp: {e}"
                    );
                    None
                }
            },
            None => None,
        };

        // Skip entries whose `message.content` is absent or neither an
        // array nor a string — preserving the prior table-builder
        // behavior of dropping malformed shapes.
        match entry.get("message").and_then(|m| m.get("content")) {
            Some(serde_json::Value::Array(_)) | Some(serde_json::Value::String(_)) => {}
            _ => continue,
        }

        let subtype = entry_subtype(&entry);

        let mut attachments: Vec<serde_json::Value> = Vec::new();
        for block in entry_attachment_blocks(&entry) {
            let content_index = entry
                .pointer("/message/content")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.iter().position(|b| std::ptr::eq(b, block)))
                .unwrap_or(0);
            let mut att = block.clone();
            if let Some(obj) = att.as_object_mut() {
                obj.insert(
                    "ref".to_string(),
                    serde_json::json!([line_num, content_index]),
                );
            }
            attachments.push(att);
        }

        let joined = entry_text(&entry).unwrap_or_default();
        let (text, ide_tags) = split_ide_tags(&joined);

        let attachments_json = if attachments.is_empty() {
            None
        } else {
            Some(serde_json::Value::Array(attachments).to_string())
        };

        b.session_ids.append_value(session_id);
        b.lines.append_value(line_num as i64);
        match &entry_uuid {
            Some(v) => b.uuids.append_value(v),
            None => b.uuids.append_null(),
        }
        b.types.append_value(type_str);
        match subtype {
            Some(v) => b.subtypes.append_value(v),
            None => b.subtypes.append_null(),
        };
        b.texts.append_value(&text);
        b.timestamps.append_option(ts_ms);
        match &attachments_json {
            Some(v) => b.attachments.append_value(v),
            None => b.attachments.append_null(),
        }
        match &ide_tags {
            Some(v) => b.ide_tags.append_value(v),
            None => b.ide_tags.append_null(),
        }
        b.raws.append_value(entry.to_string());
        count += 1;
    }
    count
}

impl ExecutionPlan for MessageExec {
    fn name(&self) -> &'static str {
        "MessageExec"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn properties(&self) -> &PlanProperties {
        &self.properties
    }

    fn children(&self) -> Vec<&Arc<dyn ExecutionPlan>> {
        vec![]
    }

    fn with_new_children(
        self: Arc<Self>,
        _children: Vec<Arc<dyn ExecutionPlan>>,
    ) -> Result<Arc<dyn ExecutionPlan>> {
        Ok(self)
    }

    fn fetch(&self) -> Option<usize> {
        self.limit
    }

    fn with_fetch(&self, limit: Option<usize>) -> Option<Arc<dyn ExecutionPlan>> {
        Some(Arc::new(MessageExec::new(
            self.full_schema.clone(),
            self.projected_schema.clone(),
            self.source.clone(),
            self.projection.clone(),
            self.session_id_filter.clone(),
            limit,
        )))
    }

    fn execute(
        &self,
        _partition: usize,
        _context: Arc<TaskContext>,
    ) -> Result<SendableRecordBatchStream> {
        let mut builders = MessageBuilders::new();
        let mut remaining = self.limit;

        match &self.source {
            SessionSource::Root(root) => {
                let sessions = SessionListBuilder::new().root(root).build();
                let sessions: Vec<SessionInfo> = match &self.session_id_filter {
                    Some(f) => f.retain(sessions, |s| s.id.as_str())?,
                    None => sessions.into_iter().collect(),
                };
                for session in &sessions {
                    if remaining == Some(0) {
                        break;
                    }
                    let added =
                        process_session(&session.id, &session.src, remaining, &mut builders);
                    if let Some(ref mut r) = remaining {
                        *r = r.saturating_sub(added);
                    }
                }
            }
            SessionSource::SingleSession { session_id, path } => {
                let keep = match &self.session_id_filter {
                    Some(f) => f.matches(session_id)?,
                    None => true,
                };
                if keep {
                    process_session(session_id, path, remaining, &mut builders);
                }
            }
        }

        let batch = builders.finish(self.full_schema.clone())?;

        Ok(Box::pin(MemoryStream::try_new(
            vec![batch],
            self.projected_schema.clone(),
            self.projection.clone(),
        )?))
    }
}
