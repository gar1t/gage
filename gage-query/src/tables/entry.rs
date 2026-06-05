use std::any::Any;
use std::fmt::{self, Formatter};
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::DateTime;
use datafusion::arrow::array::{Int64Builder, StringBuilder, TimestampMillisecondBuilder};
use datafusion::arrow::compute::SortOptions;
use datafusion::arrow::datatypes::{DataType, Field, Schema, SchemaRef, TimeUnit};
use datafusion::arrow::record_batch::RecordBatch;
use datafusion::catalog::Session;
use datafusion::datasource::TableProvider;
use datafusion::datasource::TableType;
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
use gage_claude::session::{SessionInfo, SessionListBuilder};
use gage_claude::session_reader::SessionReader;

use super::SessionSource;
use crate::filter::{self, IdFilter};

fn entry_schema() -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("session_id", DataType::Utf8, false),
        Field::new("line", DataType::Int64, false),
        Field::new("uuid", DataType::Utf8, true),
        Field::new("type", DataType::Utf8, true),
        Field::new(
            "timestamp",
            DataType::Timestamp(TimeUnit::Millisecond, Some("UTC".into())),
            true,
        ),
        Field::new("raw", DataType::Utf8, false),
    ]))
}

#[derive(Debug, Clone)]
pub struct EntryTable {
    source: SessionSource,
    schema: SchemaRef,
}

impl EntryTable {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            source: SessionSource::Root(root.into()),
            schema: entry_schema(),
        }
    }

    /// Build an `EntryTable` scoped to one session file.
    pub fn with_session(session_id: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            source: SessionSource::SingleSession {
                session_id: session_id.into(),
                path: path.into(),
            },
            schema: entry_schema(),
        }
    }
}

#[async_trait]
impl TableProvider for EntryTable {
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
        Ok(Arc::new(EntryExec::new(
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
struct EntryExec {
    source: SessionSource,
    full_schema: SchemaRef,
    projected_schema: SchemaRef,
    projection: Option<Vec<usize>>,
    session_id_filter: Option<IdFilter>,
    limit: Option<usize>,
    properties: PlanProperties,
}

impl EntryExec {
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
/// top-to-bottom by line number, so this ordering is the natural one.
/// Advertising it lets DataFusion's `EnforceSorting` rule remove a
/// redundant `ORDER BY line` from author SQL at plan time.
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

impl DisplayAs for EntryExec {
    fn fmt_as(&self, _t: DisplayFormatType, f: &mut Formatter) -> fmt::Result {
        write!(f, "EntryExec")
    }
}

impl ExecutionPlan for EntryExec {
    fn name(&self) -> &'static str {
        "EntryExec"
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
        Some(Arc::new(EntryExec::new(
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
        let mut session_ids = StringBuilder::new();
        let mut lines_col = Int64Builder::new();
        let mut uuids = StringBuilder::new();
        let mut types = StringBuilder::new();
        let mut timestamps = TimestampMillisecondBuilder::new();
        let mut raws = StringBuilder::new();
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
                    let added = append_entry_rows(
                        &session.id,
                        &session.src,
                        remaining,
                        &mut session_ids,
                        &mut lines_col,
                        &mut uuids,
                        &mut types,
                        &mut timestamps,
                        &mut raws,
                    );
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
                    append_entry_rows(
                        session_id,
                        path,
                        remaining,
                        &mut session_ids,
                        &mut lines_col,
                        &mut uuids,
                        &mut types,
                        &mut timestamps,
                        &mut raws,
                    );
                }
            }
        }

        let batch = RecordBatch::try_new(
            self.full_schema.clone(),
            vec![
                Arc::new(session_ids.finish()),
                Arc::new(lines_col.finish()),
                Arc::new(uuids.finish()),
                Arc::new(types.finish()),
                Arc::new(timestamps.finish().with_timezone("UTC")),
                Arc::new(raws.finish()),
            ],
        )?;

        Ok(Box::pin(MemoryStream::try_new(
            vec![batch],
            self.projected_schema.clone(),
            self.projection.clone(),
        )?))
    }
}

#[allow(clippy::too_many_arguments)]
fn append_entry_rows(
    session_id: &str,
    path: &std::path::Path,
    limit: Option<usize>,
    session_ids: &mut StringBuilder,
    lines_col: &mut Int64Builder,
    uuids: &mut StringBuilder,
    types: &mut StringBuilder,
    timestamps: &mut TimestampMillisecondBuilder,
    raws: &mut StringBuilder,
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
        let (line_num, v) = match result {
            Ok(pair) => pair,
            Err(_) => continue,
        };

        let type_val = v.get("type").and_then(|t| t.as_str()).map(String::from);
        let uuid_val = v.get("uuid").and_then(|u| u.as_str()).map(String::from);
        let ts_ms = match v.get("timestamp").and_then(|t| t.as_str()) {
            Some(s) => match DateTime::parse_from_rfc3339(s) {
                Ok(dt) => Some(dt.timestamp_millis()),
                Err(e) => {
                    tracing::warn!(
                        session_id,
                        line = line_num,
                        timestamp = s,
                        "unparseable entry timestamp: {e}"
                    );
                    None
                }
            },
            None => None,
        };

        session_ids.append_value(session_id);
        lines_col.append_value(line_num as i64);
        match &uuid_val {
            Some(u) => uuids.append_value(u),
            None => uuids.append_null(),
        }
        match &type_val {
            Some(t) => types.append_value(t),
            None => types.append_null(),
        }
        timestamps.append_option(ts_ms);
        raws.append_value(v.to_string());
        count += 1;
    }
    count
}
