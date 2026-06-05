use std::future::Future;
use std::pin::Pin;

use arrow::json::ArrayWriter;
use rmcp::{
    ErrorData as McpError, RoleServer, handler::server::router::tool::ToolRoute, model::JsonObject,
    service::RequestContext,
};
use serde_json::json;

use crate::server::GageServer;
use crate::tool::{MAX_DESCRIPTION_BYTES, ToolDef, build_tool_meta, description_byte_len};

pub const TOOL: ToolDef = route;

const MD: &str = include_str!("../../config/tools/Query.md");
const _: () = assert!(
    description_byte_len(MD) <= MAX_DESCRIPTION_BYTES,
    "QueryGage description exceeds Claude Code's 2048-byte cap",
);

/// Maximum serialized byte size for a `query` result. Above this we
/// return a structured size-cap error with remediation hints so the
/// model can narrow the query before the harness-level truncation
/// fires. Sized to stay comfortably under Claude Code's ~25k-token
/// tool-result cap (≈100 KB of JSON).
const RESULT_CAP_BYTES: usize = 60_000;

fn route() -> ToolRoute<GageServer> {
    ToolRoute::new(build_tool_meta(MD), call)
}

fn call(
    server: &GageServer,
    _ctx: RequestContext<RoleServer>,
    params: JsonObject,
) -> Pin<Box<dyn Future<Output = Result<String, McpError>> + Send + '_>> {
    Box::pin(handle(server, params))
}

async fn handle(server: &GageServer, params: JsonObject) -> Result<String, McpError> {
    let sql = params
        .get("sql")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("missing or non-string `sql`", None))?;

    let ctx = server.ctx().await;
    let df = ctx
        .sql(sql)
        .await
        .map_err(|e| McpError::invalid_params(format!("SQL error: {e}"), None))?;
    let batches = df
        .collect()
        .await
        .map_err(|e| McpError::internal_error(format!("query execution error: {e}"), None))?;
    let batches: Vec<_> = batches
        .iter()
        .filter(|b| b.num_rows() > 0)
        .cloned()
        .collect();
    if batches.is_empty() {
        return Ok("[]".to_string());
    }
    let row_count: usize = batches.iter().map(|b| b.num_rows()).sum();
    let mut buf: Vec<u8> = Vec::new();
    let mut writer = ArrayWriter::new(&mut buf);
    for batch in &batches {
        writer.write(batch).map_err(|e| {
            McpError::internal_error(format!("JSON serialization error: {e}"), None)
        })?;
    }
    writer
        .finish()
        .map_err(|e| McpError::internal_error(format!("JSON serialization error: {e}"), None))?;
    if buf.len() > RESULT_CAP_BYTES {
        return Ok(json!({
            "error": "result exceeds size cap",
            "bytes": buf.len(),
            "cap_bytes": RESULT_CAP_BYTES,
            "rows": row_count,
            "suggestion": "Re-run with a smaller LIMIT, paginate with `line > N`, \
                           SELECT substr(raw, 1, 800) instead of raw, \
                           or omit the raw column entirely.",
        })
        .to_string());
    }
    String::from_utf8(buf).map_err(|e| McpError::internal_error(format!("UTF-8 error: {e}"), None))
}
