#![allow(clippy::indexing_slicing)]

mod common;

use common::{col_strings, test_ctx};

#[tokio::test]
async fn entry_table_returns_rows() {
    let ctx = test_ctx().await;
    let batches = ctx
        .sql("SELECT session_id, type, uuid FROM entry ORDER BY session_id, uuid")
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();
    let total_rows: usize = batches.iter().map(|b| b.num_rows()).sum();
    assert_eq!(total_rows, 14);
}

#[tokio::test]
async fn entry_table_with_session_filter() {
    let ctx = test_ctx().await;
    let batches = ctx
        .sql(
            "SELECT type, uuid FROM entry WHERE session_id = 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee'",
        )
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].num_rows(), 2);

    let types = col_strings(&batches[0], 0);
    assert_eq!(types[0], "user");
    assert_eq!(types[1], "assistant");
}

#[tokio::test]
async fn entry_table_group_by_type() {
    let ctx = test_ctx().await;
    let batches = ctx
        .sql(
            "SELECT type, count(*) as cnt FROM entry WHERE session_id = 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee' GROUP BY type ORDER BY type",
        )
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();
    assert_eq!(batches.len(), 1);
    let types = col_strings(&batches[0], 0);
    assert_eq!(types, vec!["assistant", "user"]);
}

#[tokio::test]
async fn entry_table_raw_contains_json() {
    let ctx = test_ctx().await;
    let batches = ctx
        .sql(
            "SELECT raw FROM entry WHERE session_id = 'aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee' LIMIT 1",
        )
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();
    let raws = col_strings(&batches[0], 0);
    assert!(raws[0].starts_with('{'));
}

#[tokio::test]
async fn entry_table_no_match() {
    let ctx = test_ctx().await;
    let batches = ctx
        .sql("SELECT type FROM entry WHERE session_id = 'nonexistent'")
        .await
        .unwrap()
        .collect()
        .await
        .unwrap();
    let total_rows: usize = batches.iter().map(|b| b.num_rows()).sum();
    assert_eq!(total_rows, 0);
}
