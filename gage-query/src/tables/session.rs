use std::any::Any;
use std::fmt::{self, Formatter};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use async_trait::async_trait;
use datafusion::arrow::array::{
    BooleanBuilder, Int64Builder, StringBuilder, TimestampMillisecondBuilder,
};
use datafusion::arrow::compute::SortOptions;
use datafusion::arrow::datatypes::{DataType, Field, Schema, SchemaRef, TimeUnit};
use datafusion::arrow::record_batch::RecordBatch;
use datafusion::catalog::Session;
use datafusion::datasource::{TableProvider, TableType};
use datafusion::error::Result;
use datafusion::execution::context::TaskContext;
use datafusion::logical_expr::TableProviderFilterPushDown;
use datafusion::physical_expr::expressions::col;
use datafusion::physical_expr::{EquivalenceProperties, PhysicalSortExpr};
use datafusion::physical_plan::execution_plan::{Boundedness, EmissionType};
use datafusion::physical_plan::memory::MemoryStream;
use datafusion::physical_plan::{
    DisplayAs, DisplayFormatType, ExecutionPlan, Partitioning, PlanProperties,
    SendableRecordBatchStream,
};
use datafusion::prelude::*;
use gage_claude::entry::{entry_to_text, split_ide_tags};
use gage_claude::session::{SessionInfo, SessionListBuilder};
use gage_claude::session_reader::SessionReader;

use crate::filter::{self, IdFilter};

// Column layout in the merged `session` schema. Cheap columns
// (filled from the directory walk) come first; expensive columns
// (filled by parsing the session JSONL) come after. The expensive
// region starts at `EXPENSIVE_START` and `message_count` lives at
// `COL_MESSAGE_COUNT` — these two indices are load-bearing.
const EXPENSIVE_START: usize = 5; // title is the first expensive column
const COL_MESSAGE_COUNT: usize = 7;
const NUM_COLS: usize = 13;

fn session_schema() -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("project", DataType::Utf8, true),
        Field::new("path", DataType::Utf8, false),
        Field::new(
            "mtime",
            DataType::Timestamp(TimeUnit::Millisecond, Some("UTC".into())),
            false,
        ),
        Field::new("size", DataType::Int64, false),
        Field::new("title", DataType::Utf8, true),
        Field::new("model", DataType::Utf8, true),
        Field::new("message_count", DataType::Int64, false),
        Field::new("input_tokens", DataType::Int64, false),
        Field::new("output_tokens", DataType::Int64, false),
        Field::new("cache_read_input_tokens", DataType::Int64, false),
        Field::new("cache_creation_input_tokens", DataType::Int64, false),
        Field::new("is_empty", DataType::Boolean, false),
    ]))
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ScannedRow {
    pub model: Option<String>,
    pub message_count: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_input_tokens: i64,
    pub cache_creation_input_tokens: i64,
    pub title: Option<String>,
    pub is_empty: bool,
}

/// Fast message count using byte-level matching. No JSON parsing.
fn fast_message_count(path: &Path) -> i64 {
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return 0,
    };
    let reader = BufReader::new(file);
    let mut count: i64 = 0;
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };
        if line.contains("\"type\":\"user\"")
            || line.contains("\"type\": \"user\"")
            || line.contains("\"type\":\"assistant\"")
            || line.contains("\"type\": \"assistant\"")
        {
            count += 1;
        }
    }
    count
}

/// Maximum number of characters of source text to include when
/// deriving a title from the first usable user message. When the
/// source is longer it is truncated and a trailing `...` is appended.
const MAX_TITLE_LEN_FROM_MSG: usize = 60;

/// Derive a session title from a user entry, used as a fallback when
/// no `ai-title` entry is present. Returns `None` when the entry has
/// no usable text (e.g. purely tag-wrapped meta content) so the caller
/// can keep scanning.
fn session_title_from_entry(entry: &serde_json::Value) -> Option<String> {
    let text = entry_to_text(entry);
    let (body, _ide_tags) = split_ide_tags(&text);
    let line_end = body.find('\n').unwrap_or(body.len());
    let line = body[..line_end].trim();
    if line.is_empty() {
        return None;
    }
    Some(truncate_with_ellipsis(line, MAX_TITLE_LEN_FROM_MSG))
}

fn truncate_with_ellipsis(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let truncated: String = s.chars().take(max_chars).collect();
    format!("{}...", truncated.trim_end())
}

/// Whether a session entry carries real conversational content, used
/// to decide whether a session is empty. An assistant turn always
/// counts. A user turn counts only when its message is more than
/// out-of-band tags: a single content string that is empty after
/// stripping IDE/harness tags (the local-command caveat, slash-command
/// echoes like `/exit`, local-command stdout) does not count.
/// Non-message entries (permission-mode, file-history-snapshot, …)
/// never count.
fn entry_has_content(entry: &serde_json::Value) -> bool {
    match entry.get("type").and_then(|t| t.as_str()) {
        Some("assistant") => true,
        Some("user") => match entry.get("message").and_then(|m| m.get("content")) {
            Some(serde_json::Value::String(s)) => !split_ide_tags(s).0.trim().is_empty(),
            Some(serde_json::Value::Array(blocks)) => !blocks.is_empty(),
            _ => false,
        },
        _ => false,
    }
}

/// Scan a single session file and return the expensive-column values
/// plus the number of lines read. Exposed for test use.
pub(crate) fn scan_session(path: &Path) -> (ScannedRow, usize) {
    let mut row = ScannedRow {
        is_empty: true,
        ..Default::default()
    };

    let reader = match SessionReader::open(path) {
        Ok(r) => r,
        Err(_) => return (row, 0),
    };
    let mut lines_read: usize = 0;

    for result in reader {
        let (_line_num, v) = match result {
            Ok(pair) => pair,
            Err(_) => continue,
        };
        lines_read += 1;

        if row.is_empty && entry_has_content(&v) {
            row.is_empty = false;
        }

        let entry_type = v.get("type").and_then(|t| t.as_str()).unwrap_or("");

        match entry_type {
            "user" | "assistant" => {
                row.message_count += 1;

                if entry_type == "assistant" {
                    let msg = v.get("message");

                    if row.model.is_none() {
                        row.model = msg
                            .and_then(|m| m.get("model"))
                            .and_then(|m| m.as_str())
                            .map(String::from);
                    }

                    if let Some(usage) = msg.and_then(|m| m.get("usage")) {
                        row.input_tokens += usage
                            .get("input_tokens")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0);
                        row.output_tokens += usage
                            .get("output_tokens")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0);
                        row.cache_read_input_tokens += usage
                            .get("cache_read_input_tokens")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0);
                        row.cache_creation_input_tokens += usage
                            .get("cache_creation_input_tokens")
                            .and_then(|v| v.as_i64())
                            .unwrap_or(0);
                    }
                } else if entry_type == "user" && row.title.is_none() {
                    row.title = session_title_from_entry(&v);
                }
            }
            "ai-title" => {
                row.title = v.get("aiTitle").and_then(|t| t.as_str()).map(String::from);
            }
            _ => {}
        }
    }

    (row, lines_read)
}

#[derive(Debug, Clone)]
pub struct SessionTable {
    root: PathBuf,
    schema: SchemaRef,
}

impl SessionTable {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            schema: session_schema(),
        }
    }
}

#[async_trait]
impl TableProvider for SessionTable {
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
    ) -> Result<Vec<TableProviderFilterPushDown>> {
        let support = filters.iter().map(|f| filter::pushdown(f, "id")).collect();
        Ok(support)
    }

    async fn scan(
        &self,
        _state: &dyn Session,
        projection: Option<&Vec<usize>>,
        filters: &[Expr],
        limit: Option<usize>,
    ) -> Result<Arc<dyn ExecutionPlan>> {
        let id_filter = IdFilter::new(filters, "id")?;
        let projected_schema = match projection {
            Some(indices) => Arc::new(self.schema.project(indices)?),
            None => self.schema.clone(),
        };
        if tracing::enabled!(tracing::Level::DEBUG) {
            let cols: Vec<&str> = match projection {
                Some(indices) => indices
                    .iter()
                    .map(|&i| self.schema.field(i).name().as_str())
                    .collect(),
                None => self
                    .schema
                    .fields()
                    .iter()
                    .map(|f| f.name().as_str())
                    .collect(),
            };
            tracing::debug!(
                target: "gage_query::session",
                projection = ?cols,
                filters = filters.len(),
                id_filter = ?id_filter,
                limit_pushdown = ?limit,
                "session::scan invoked",
            );
        }
        Ok(Arc::new(SessionExec::new(
            self.schema.clone(),
            projected_schema,
            self.root.clone(),
            projection.cloned(),
            id_filter,
            limit,
        )))
    }
}

#[derive(Debug, Clone)]
struct SessionExec {
    root: PathBuf,
    full_schema: SchemaRef,
    projected_schema: SchemaRef,
    projection: Option<Vec<usize>>,
    id_filter: Option<IdFilter>,
    limit: Option<usize>,
    properties: PlanProperties,
}

impl SessionExec {
    fn new(
        full_schema: SchemaRef,
        projected_schema: SchemaRef,
        root: PathBuf,
        projection: Option<Vec<usize>>,
        id_filter: Option<IdFilter>,
        limit: Option<usize>,
    ) -> Self {
        let eq_properties = mtime_desc_eq_properties(&projected_schema);
        let properties = PlanProperties::new(
            eq_properties,
            Partitioning::UnknownPartitioning(1),
            EmissionType::Incremental,
            Boundedness::Bounded,
        );
        Self {
            root,
            full_schema,
            projected_schema,
            projection,
            id_filter,
            limit,
            properties,
        }
    }

    /// True if the requested projection only touches cheap columns
    /// (those filled from the directory walk).
    fn projection_is_cheap_only(&self) -> bool {
        match &self.projection {
            Some(indices) => indices.iter().all(|&i| i < EXPENSIVE_START),
            None => false,
        }
    }

    /// True if the projection only needs `message_count` (and any
    /// cheap columns) — lets us skip JSON parsing in favor of the
    /// byte-level line counter.
    fn projection_is_message_count_only(&self) -> bool {
        match &self.projection {
            Some(indices) => indices
                .iter()
                .all(|&i| i < EXPENSIVE_START || i == COL_MESSAGE_COUNT),
            None => false,
        }
    }
}

/// Build `EquivalenceProperties` advertising `[mtime DESC]` when the
/// projection retains the `mtime` column. `SessionListBuilder::build`
/// already sorts by mtime descending, so this is the natural order.
/// Advertising it lets DataFusion push `LIMIT N` past `ORDER BY mtime
/// DESC`, which we honor by capping the directory walk at N entries.
fn mtime_desc_eq_properties(projected_schema: &SchemaRef) -> EquivalenceProperties {
    match col("mtime", projected_schema) {
        Ok(mtime_expr) => {
            // SQL `ORDER BY mtime DESC` in DataFusion defaults to NULLS
            // FIRST; advertise the same so EnforceSorting can elide the
            // SortExec and LimitPushdown can pass `fetch` to scan().
            let sort = PhysicalSortExpr {
                expr: mtime_expr,
                options: SortOptions {
                    descending: true,
                    nulls_first: true,
                },
            };
            EquivalenceProperties::new_with_orderings(projected_schema.clone(), [[sort]])
        }
        Err(_) => EquivalenceProperties::new(projected_schema.clone()),
    }
}

impl DisplayAs for SessionExec {
    fn fmt_as(&self, _t: DisplayFormatType, f: &mut Formatter) -> fmt::Result {
        write!(f, "SessionExec")
    }
}

impl ExecutionPlan for SessionExec {
    fn name(&self) -> &'static str {
        "SessionExec"
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

    /// Report the current fetch (limit) so DataFusion's LimitPushdown
    /// rule knows what's already plumbed through.
    fn fetch(&self) -> Option<usize> {
        self.limit
    }

    /// Accept a fetch from DataFusion's LimitPushdown. Combined with
    /// our advertised `[mtime DESC]` ordering, this is what lets
    /// `ORDER BY mtime DESC LIMIT N` open at most N session files.
    fn with_fetch(&self, limit: Option<usize>) -> Option<Arc<dyn ExecutionPlan>> {
        Some(Arc::new(SessionExec::new(
            self.full_schema.clone(),
            self.projected_schema.clone(),
            self.root.clone(),
            self.projection.clone(),
            self.id_filter.clone(),
            limit,
        )))
    }

    fn execute(
        &self,
        _partition: usize,
        _context: Arc<TaskContext>,
    ) -> Result<SendableRecordBatchStream> {
        let exec_start = std::time::Instant::now();
        let mut builder = SessionListBuilder::new().root(&self.root);
        // Limit pushdown is only safe when there is no extra
        // post-filter that could reject rows. An `id` filter is applied
        // here per row, so limit pushdown is skipped whenever one is set.
        let walk_limit_applied = if self.id_filter.is_none()
            && let Some(n) = self.limit
        {
            builder = builder.limit(n);
            Some(n)
        } else {
            None
        };
        let walk_start = std::time::Instant::now();
        let sessions = builder.build();
        let walk_ms = walk_start.elapsed().as_millis();
        let walked = sessions.len();

        let cheap_only = self.projection_is_cheap_only();
        let count_only = self.projection_is_message_count_only();

        let sessions: Vec<SessionInfo> = match &self.id_filter {
            Some(f) => f.retain(sessions, |s| s.id.as_str())?,
            None => sessions.into_iter().collect(),
        };
        let files: Vec<(String, String, PathBuf, std::time::SystemTime, u64)> = sessions
            .into_iter()
            .map(|s| {
                let project = s.project_name().into_owned();
                (s.id, project, s.src, s.mtime, s.size)
            })
            .collect();

        let path_label = if cheap_only {
            "cheap_only"
        } else if count_only {
            "count_only"
        } else {
            "full_parse"
        };
        tracing::debug!(
            target: "gage_query::session",
            walked,
            after_id_filter = files.len(),
            walk_ms,
            walk_limit_applied = ?walk_limit_applied,
            path = path_label,
            "SessionExec walk complete",
        );

        let len = files.len();
        let mut ids = StringBuilder::with_capacity(len, len * 36);
        let mut projects = StringBuilder::with_capacity(len, len * 32);
        let mut paths = StringBuilder::with_capacity(len, len * 64);
        let mut mtimes = TimestampMillisecondBuilder::with_capacity(len);
        let mut sizes = Int64Builder::with_capacity(len);
        let mut titles = StringBuilder::new();
        let mut models = StringBuilder::new();
        let mut message_counts = Int64Builder::with_capacity(len);
        let mut input_tokens = Int64Builder::with_capacity(len);
        let mut output_tokens = Int64Builder::with_capacity(len);
        let mut cache_read = Int64Builder::with_capacity(len);
        let mut cache_creation = Int64Builder::with_capacity(len);
        let mut is_empty = BooleanBuilder::with_capacity(len);

        for (id, project, path, mtime, size) in &files {
            ids.append_value(id);
            projects.append_value(project);
            paths.append_value(path.to_string_lossy().as_ref());
            let millis = mtime
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as i64;
            mtimes.append_value(millis);
            sizes.append_value(*size as i64);

            if cheap_only {
                titles.append_null();
                models.append_null();
                message_counts.append_value(0);
                input_tokens.append_value(0);
                output_tokens.append_value(0);
                cache_read.append_value(0);
                cache_creation.append_value(0);
                is_empty.append_value(false);
            } else if count_only {
                titles.append_null();
                models.append_null();
                message_counts.append_value(fast_message_count(path));
                input_tokens.append_value(0);
                output_tokens.append_value(0);
                cache_read.append_value(0);
                cache_creation.append_value(0);
                is_empty.append_value(false);
            } else {
                let (row, _) = scan_session(path);
                match &row.title {
                    Some(t) => titles.append_value(t),
                    None => titles.append_null(),
                }
                match &row.model {
                    Some(m) => models.append_value(m),
                    None => models.append_null(),
                }
                message_counts.append_value(row.message_count);
                input_tokens.append_value(row.input_tokens);
                output_tokens.append_value(row.output_tokens);
                cache_read.append_value(row.cache_read_input_tokens);
                cache_creation.append_value(row.cache_creation_input_tokens);
                is_empty.append_value(row.is_empty);
            }
        }

        let mut columns: Vec<Arc<dyn datafusion::arrow::array::Array>> =
            Vec::with_capacity(NUM_COLS);
        columns.push(Arc::new(ids.finish()));
        columns.push(Arc::new(projects.finish()));
        columns.push(Arc::new(paths.finish()));
        columns.push(Arc::new(mtimes.finish().with_timezone("UTC")));
        columns.push(Arc::new(sizes.finish()));
        columns.push(Arc::new(titles.finish()));
        columns.push(Arc::new(models.finish()));
        columns.push(Arc::new(message_counts.finish()));
        columns.push(Arc::new(input_tokens.finish()));
        columns.push(Arc::new(output_tokens.finish()));
        columns.push(Arc::new(cache_read.finish()));
        columns.push(Arc::new(cache_creation.finish()));
        columns.push(Arc::new(is_empty.finish()));

        let batch = RecordBatch::try_new(self.full_schema.clone(), columns)?;

        tracing::debug!(
            target: "gage_query::session",
            rows = batch.num_rows(),
            elapsed_ms = exec_start.elapsed().as_millis(),
            "SessionExec done",
        );

        Ok(Box::pin(MemoryStream::try_new(
            vec![batch],
            self.projected_schema.clone(),
            self.projection.clone(),
        )?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_session_reads_all_lines() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("testdata")
            .join("-home-test-project")
            .join("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb.jsonl");
        let (row, lines_read) = scan_session(&path);
        assert_eq!(row.title.as_deref(), Some("Read and explain main.rs"));
        assert_eq!(row.model.as_deref(), Some("claude-sonnet-4-20250514"));
        assert_eq!(lines_read, 8);
    }
}
