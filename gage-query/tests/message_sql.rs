#![allow(clippy::indexing_slicing)]

mod common;

use common::{col_strings, test_ctx};
use datafusion::arrow::array::{Array, StringArray};

#[tokio::test]
async fn message_table_user_text() {
    let ctx = test_ctx().await;
    let batches = ctx
        .sql(
            "SELECT type, text FROM message WHERE session_id = 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee' AND type = 'user'",
        )
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].num_rows(), 1);
    let texts = col_strings(&batches[0], 1);
    assert_eq!(texts[0], "hello");
}

#[tokio::test]
async fn message_table_rich_session() {
    let ctx = test_ctx().await;
    let batches = ctx
        .sql(
            "SELECT type, subtype, text, timestamp FROM message WHERE session_id = 'bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb' ORDER BY line",
        )
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();
    assert_eq!(batches.len(), 1);
    let batch = &batches[0];
    // Expect: user, assistant(thinking), assistant(text), assistant(tool_use),
    //         user(tool_result), assistant(text) = 6 rows
    // (progress and ai-title entries are excluded)
    assert_eq!(batch.num_rows(), 6);

    let types = col_strings(batch, 0);
    assert_eq!(
        types,
        vec![
            "user",
            "assistant",
            "assistant",
            "assistant",
            "user",
            "assistant"
        ]
    );

    let subtypes: Vec<Option<&str>> = batch
        .column(1)
        .as_any()
        .downcast_ref::<StringArray>()
        .unwrap()
        .iter()
        .collect();
    assert_eq!(
        subtypes,
        vec![
            Some("text"),
            Some("thinking"),
            Some("text"),
            Some("tool_use"),
            Some("tool_result"),
            Some("text"),
        ]
    );
}

#[tokio::test]
async fn message_table_thinking_text() {
    let ctx = test_ctx().await;
    let batches = ctx
        .sql(
            "SELECT text FROM message WHERE session_id = 'bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb' AND subtype = 'thinking'",
        )
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();
    assert_eq!(batches[0].num_rows(), 1);
    let texts = col_strings(&batches[0], 0);
    assert!(texts[0].contains("Read tool"));
}

#[tokio::test]
async fn message_table_tool_use_text() {
    let ctx = test_ctx().await;
    let batches = ctx
        .sql(
            "SELECT text FROM message WHERE session_id = 'bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb' AND subtype = 'tool_use'",
        )
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();
    assert_eq!(batches[0].num_rows(), 1);
    let texts = col_strings(&batches[0], 0);
    assert!(texts[0].contains("Read"));
    assert!(texts[0].contains("file_path"));
    assert!(texts[0].contains("main.rs"));
}

// An explicitly-set empty text block must be captured as "" (not NULL):
// the text was present in the JSON, just empty.
#[tokio::test]
async fn message_table_empty_text_is_not_null() {
    let ctx = test_ctx().await;
    let batches = ctx
        .sql(
            "SELECT text FROM message WHERE session_id = 'cccccccc-cccc-cccc-cccc-cccccccccccc' AND line = 1",
        )
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();
    let col = batches[0]
        .column(0)
        .as_any()
        .downcast_ref::<StringArray>()
        .unwrap();
    assert!(col.is_valid(0), "explicit empty text must not be NULL");
    assert_eq!(col.value(0), "");
}

// A message whose entire body is IDE tags keeps a non-NULL (empty) text
// with the tags split out into ide_tags.
#[tokio::test]
async fn message_table_ide_only_text_is_empty_not_null() {
    let ctx = test_ctx().await;
    let batches = ctx
        .sql(
            "SELECT text, ide_tags FROM message WHERE session_id = 'cccccccc-cccc-cccc-cccc-cccccccccccc' AND line = 2",
        )
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();
    let text = batches[0]
        .column(0)
        .as_any()
        .downcast_ref::<StringArray>()
        .unwrap();
    let ide = batches[0]
        .column(1)
        .as_any()
        .downcast_ref::<StringArray>()
        .unwrap();
    assert!(text.is_valid(0), "IDE-only body must not be NULL");
    assert_eq!(text.value(0), "");
    assert!(ide.is_valid(0));
    assert!(ide.value(0).contains("ide_opened_file"));
}

// With no recognized text content anywhere (only a non-text block), text
// is the empty string — the column is never NULL.
#[tokio::test]
async fn message_table_no_text_content_is_empty() {
    let ctx = test_ctx().await;
    let batches = ctx
        .sql(
            "SELECT text FROM message WHERE session_id = 'cccccccc-cccc-cccc-cccc-cccccccccccc' AND line = 3",
        )
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();
    let col = batches[0]
        .column(0)
        .as_any()
        .downcast_ref::<StringArray>()
        .unwrap();
    assert!(col.is_valid(0), "no text content must be empty, not NULL");
    assert_eq!(col.value(0), "");
}

#[tokio::test]
async fn message_table_excludes_non_messages() {
    let ctx = test_ctx().await;
    let batches = ctx
        .sql(
            "SELECT count(*) as cnt FROM message WHERE session_id = 'bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb'",
        )
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();
    let count = batches[0]
        .column(0)
        .as_any()
        .downcast_ref::<datafusion::arrow::array::Int64Array>()
        .unwrap()
        .value(0);
    assert_eq!(count, 6);
}

#[tokio::test]
async fn message_table_no_match() {
    let ctx = test_ctx().await;
    let batches = ctx
        .sql("SELECT type FROM message WHERE session_id = 'nonexistent'")
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();
    let total_rows: usize = batches.iter().map(|b| b.num_rows()).sum();
    assert_eq!(total_rows, 0);
}
