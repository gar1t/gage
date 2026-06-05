#![allow(clippy::indexing_slicing)]

mod common;

use common::{col_strings, test_ctx};
use datafusion::arrow::array::{Array, StringArray};

#[tokio::test]
async fn session_table_returns_rows() {
    let ctx = test_ctx().await;
    let batches = ctx
        .sql("SELECT id, project FROM session ORDER BY id")
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();
    assert_eq!(batches.len(), 1);
    let batch = &batches[0];
    assert_eq!(batch.num_rows(), 4);

    let ids = col_strings(batch, 0);
    assert_eq!(ids[0], "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee");
    assert_eq!(ids[1], "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb");
    assert_eq!(ids[2], "cccccccc-cccc-cccc-cccc-cccccccccccc");
    assert_eq!(ids[3], "ff000000-1111-2222-3333-444444444444");

    let projects = col_strings(batch, 1);
    assert_eq!(projects[0], "-home-test-project");
    assert_eq!(projects[1], "-home-test-project");
    assert_eq!(projects[2], "-home-test-project");
    assert_eq!(projects[3], "-home-test-other");
}

#[tokio::test]
async fn session_table_order_by_and_limit() {
    let ctx = test_ctx().await;
    let batches = ctx
        .sql("SELECT id FROM session ORDER BY mtime DESC LIMIT 1")
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].num_rows(), 1);
}

#[tokio::test]
async fn session_table_where_like() {
    let ctx = test_ctx().await;
    let batches = ctx
        .sql("SELECT id FROM session WHERE id LIKE 'aaaa%'")
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();
    assert_eq!(batches.len(), 1);
    let ids = col_strings(&batches[0], 0);
    assert_eq!(ids, vec!["aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"]);
}

#[tokio::test]
async fn session_table_where_no_match() {
    let ctx = test_ctx().await;
    let batches = ctx
        .sql("SELECT id FROM session WHERE id = 'nonexistent'")
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();
    let total_rows: usize = batches.iter().map(|b| b.num_rows()).sum();
    assert_eq!(total_rows, 0);
}

#[tokio::test]
async fn session_table_projection() {
    let ctx = test_ctx().await;
    let batches = ctx
        .sql("SELECT size FROM session LIMIT 1")
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();
    assert_eq!(batches[0].schema().fields().len(), 1);
    assert_eq!(batches[0].schema().field(0).name(), "size");
}

#[tokio::test]
async fn session_table_full_scan() {
    let ctx = test_ctx().await;
    let batches = ctx
        .sql(
            "SELECT id, model, message_count, input_tokens, output_tokens, \
             cache_read_input_tokens, cache_creation_input_tokens, title \
             FROM session WHERE id = 'bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb'",
        )
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();
    assert_eq!(batches.len(), 1);
    let batch = &batches[0];
    assert_eq!(batch.num_rows(), 1);

    let session_id = col_strings(batch, 0);
    assert_eq!(session_id[0], "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb");

    let model = col_strings(batch, 1);
    assert_eq!(model[0], "claude-sonnet-4-20250514");

    let msg_count = batch
        .column(2)
        .as_any()
        .downcast_ref::<datafusion::arrow::array::Int64Array>()
        .unwrap()
        .value(0);
    assert_eq!(msg_count, 6);

    let input_tok = batch
        .column(3)
        .as_any()
        .downcast_ref::<datafusion::arrow::array::Int64Array>()
        .unwrap()
        .value(0);
    assert_eq!(input_tok, 500);

    let output_tok = batch
        .column(4)
        .as_any()
        .downcast_ref::<datafusion::arrow::array::Int64Array>()
        .unwrap()
        .value(0);
    assert_eq!(output_tok, 90);

    let cache_read = batch
        .column(5)
        .as_any()
        .downcast_ref::<datafusion::arrow::array::Int64Array>()
        .unwrap()
        .value(0);
    assert_eq!(cache_read, 250);

    let cache_creation = batch
        .column(6)
        .as_any()
        .downcast_ref::<datafusion::arrow::array::Int64Array>()
        .unwrap()
        .value(0);
    assert_eq!(cache_creation, 10);

    let title = col_strings(batch, 7);
    assert_eq!(title[0], "Read and explain main.rs");
}

#[tokio::test]
async fn session_table_title_projection() {
    let ctx = test_ctx().await;
    let batches = ctx
        .sql("SELECT id, title FROM session ORDER BY id")
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();
    assert_eq!(batches.len(), 1);
    let batch = &batches[0];
    assert_eq!(batch.num_rows(), 4);
    assert_eq!(batch.schema().fields().len(), 2);

    let titles: Vec<Option<&str>> = batch
        .column(1)
        .as_any()
        .downcast_ref::<StringArray>()
        .unwrap()
        .iter()
        .collect();
    // Sessions without an `ai-title` entry fall back to the first
    // user message ("hello" and "test"). The edge-case session's only
    // user message is IDE-tag-only, so it has no derivable title.
    assert_eq!(titles[0], Some("hello"));
    assert_eq!(titles[1], Some("Read and explain main.rs"));
    assert_eq!(titles[2], None);
    assert_eq!(titles[3], Some("test"));
}

#[tokio::test]
async fn session_table_no_assistant_entries() {
    let ctx = test_ctx().await;
    let batches = ctx
        .sql(
            "SELECT model, message_count, input_tokens \
             FROM session WHERE id = 'ff000000-1111-2222-3333-444444444444'",
        )
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();
    assert_eq!(batches[0].num_rows(), 1);

    let model_arr = batches[0]
        .column(0)
        .as_any()
        .downcast_ref::<StringArray>()
        .unwrap();
    assert!(model_arr.is_null(0));

    let msg_count = batches[0]
        .column(1)
        .as_any()
        .downcast_ref::<datafusion::arrow::array::Int64Array>()
        .unwrap()
        .value(0);
    assert_eq!(msg_count, 1);

    let input_tok = batches[0]
        .column(2)
        .as_any()
        .downcast_ref::<datafusion::arrow::array::Int64Array>()
        .unwrap()
        .value(0);
    assert_eq!(input_tok, 0);
}
