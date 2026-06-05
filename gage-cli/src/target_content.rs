//! Render the content a `NoteTarget` refers to, for `gage note show -t`.
//!
//! For session-scoped targets, returns a pre-wrapped string: the locator
//! followed by a blank line followed by the target's content, sized to
//! `width`. Non-session targets render a placeholder.

use std::io;

use datafusion::arrow::array::{Array, StringArray};
use datafusion::prelude::SessionContext;
use gage_claude::entry::split_ide_tags;
use gage_db::target::{NoteTarget, SessionTarget};
use gage_query::tables::entry_text;
use serde_json::Value;

const DIM_ITALIC: &str = "\x1b[2;3m";
const RESET_DIM_ITALIC: &str = "\x1b[22;23m";

/// Render the full target cell — locator + blank line + content, all
/// wrapped to `width`. Called by `gage note show -t`.
pub async fn render_target_cell(
    ctx: &SessionContext,
    target: &NoteTarget,
    width: usize,
) -> io::Result<String> {
    let locator = textwrap::fill(&target.to_uri(), width);
    let content = render_content(ctx, target, width).await?;
    Ok(format!("{locator}\n{content}"))
}

async fn render_content(
    ctx: &SessionContext,
    target: &NoteTarget,
    width: usize,
) -> io::Result<String> {
    match target {
        NoteTarget::Scan(_) | NoteTarget::Project(_) => {
            Ok(format!("{DIM_ITALIC}{}{RESET_DIM_ITALIC}", target.to_uri()))
        }
        NoteTarget::Session(t) => render_session_content(ctx, t, width).await,
    }
}

async fn render_session_content(
    ctx: &SessionContext,
    target: &SessionTarget,
    width: usize,
) -> io::Result<String> {
    let session_id = &target.session_id;
    let Some(line) = target.line else {
        return Ok(format!("{DIM_ITALIC}Whole session{RESET_DIM_ITALIC}"));
    };
    let entry = query_raw_entry(ctx, session_id, line).await?;
    let text = entry_text(&entry)
        .map(|t| split_ide_tags(&t).0)
        .unwrap_or_default();
    Ok(textwrap::fill(&text, width))
}

async fn query_raw_entry(ctx: &SessionContext, session_id: &str, line: u32) -> io::Result<Value> {
    let sql = format!("SELECT raw FROM entry WHERE session_id = '{session_id}' AND line = {line}");
    let df = ctx.sql(&sql).await.map_err(io::Error::other)?;
    let batches = df.collect().await.map_err(io::Error::other)?;
    for batch in &batches {
        let col = batch
            .column(0)
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        if col.len() > 0 {
            let raw = col.value(0);
            return serde_json::from_str(raw).map_err(io::Error::other);
        }
    }
    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("line {line} not found in session {session_id}"),
    ))
}
