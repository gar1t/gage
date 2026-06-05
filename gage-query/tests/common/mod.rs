use std::path::{Path, PathBuf};

use datafusion::arrow::array::StringArray;
use datafusion::arrow::record_batch::RecordBatch;
use datafusion::prelude::SessionContext;
use gage_query::create_context;

pub fn testdata() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("testdata")
}

pub async fn test_ctx() -> SessionContext {
    create_context(&testdata()).await
}

#[allow(clippy::indexing_slicing)]
pub fn col_strings(batch: &RecordBatch, idx: usize) -> Vec<String> {
    batch
        .column(idx)
        .as_any()
        .downcast_ref::<StringArray>()
        .unwrap()
        .iter()
        .map(|v| v.unwrap().to_string())
        .collect()
}
