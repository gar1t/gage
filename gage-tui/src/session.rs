//! Load the session row and entry rows for a session via `gage-query`.

use std::error::Error;

use datafusion::arrow::array::{Array, RecordBatch, StringArray};
use datafusion::arrow::json::ArrayWriter;
use datafusion::prelude::SessionContext;
use serde_json::Value;

use crate::doc::{Document, Entry, Session};

pub async fn load(session_id: &str) -> Result<Document, Box<dyn Error>> {
    let ctx = gage_query::create_context_default().await;
    let session = load_session(&ctx, session_id).await?;
    let entries = load_entries(&ctx, session_id).await?;
    Ok(Document { session, entries })
}

async fn load_session(ctx: &SessionContext, session_id: &str) -> Result<Session, Box<dyn Error>> {
    let sql = format!(
        "SELECT * FROM session WHERE id = '{}'",
        session_id.replace('\'', "''")
    );
    let batches = ctx.sql(&sql).await?.collect().await?;
    let value = batches
        .iter()
        .find(|b| b.num_rows() > 0)
        .map(first_row_as_value)
        .transpose()?
        .unwrap_or(Value::Null);
    Ok(Session {
        id: session_id.to_string(),
        value,
    })
}

/// Serialize the first row of `batch` as a JSON object via arrow's
/// `ArrayWriter`, then parse it back into a `Value`. The roundtrip is the
/// path of least resistance — `ArrayWriter` is already the canonical
/// arrow-to-JSON conversion in this workspace.
fn first_row_as_value(batch: &RecordBatch) -> Result<Value, Box<dyn Error>> {
    let row = batch.slice(0, 1);
    let mut buf: Vec<u8> = Vec::new();
    let mut writer = ArrayWriter::new(&mut buf);
    writer.write(&row)?;
    writer.finish()?;
    let arr: Vec<Value> = serde_json::from_slice(&buf)?;
    Ok(arr.into_iter().next().unwrap_or(Value::Null))
}

async fn load_entries(
    ctx: &SessionContext,
    session_id: &str,
) -> Result<Vec<Entry>, Box<dyn Error>> {
    let sql = format!(
        "SELECT raw FROM entry WHERE session_id = '{}' ORDER BY line",
        session_id.replace('\'', "''")
    );
    let batches = ctx.sql(&sql).await?.collect().await?;
    let mut entries = Vec::new();
    for batch in &batches {
        let raws = batch
            .column(0)
            .as_any()
            .downcast_ref::<StringArray>()
            .expect("entry.raw is a non-null Utf8 column");
        for i in 0..batch.num_rows() {
            let value = serde_json::from_str(raws.value(i))?;
            entries.push(Entry { value });
        }
    }
    Ok(entries)
}
