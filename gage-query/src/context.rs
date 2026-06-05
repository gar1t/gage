use std::path::{Path, PathBuf};
use std::sync::Arc;

use datafusion::prelude::{SessionConfig, SessionContext};

use crate::tables::config::ConfigTable;
use crate::tables::entry::EntryTable;
use crate::tables::issue::IssueTable;
use crate::tables::issue_evidence::IssueEvidenceTable;
use crate::tables::message::MessageTable;
use crate::tables::note::NoteTable;
use crate::tables::session::SessionTable;

fn default_root() -> PathBuf {
    if let Ok(dir) = std::env::var("GAGE_PROJECTS_DIR") {
        return PathBuf::from(dir);
    }
    let home = std::env::var("HOME")
        .map(PathBuf::from)
        .expect("HOME environment variable not set");
    home.join(".claude").join("projects")
}

pub async fn create_context_default() -> SessionContext {
    create_context(&default_root()).await
}

pub async fn create_context(root: &Path) -> SessionContext {
    let config = SessionConfig::new().with_information_schema(true);
    let mut ctx = SessionContext::new_with_config(config);
    datafusion_functions_json::register_all(&mut ctx).unwrap();
    ctx.register_table("session", Arc::new(SessionTable::new(root)))
        .unwrap();
    ctx.register_table("entry", Arc::new(EntryTable::new(root)))
        .unwrap();
    ctx.register_table("message", Arc::new(MessageTable::new(root)))
        .unwrap();
    ctx.register_table("note", Arc::new(NoteTable::new()))
        .unwrap();
    ctx.register_table("issue", Arc::new(IssueTable::new()))
        .unwrap();
    ctx.register_table("issue_evidence", Arc::new(IssueEvidenceTable::new()))
        .unwrap();
    // `root` is `<home>/.claude/projects`; recover the home dir for the
    // `config` table. Tests that pass a non-standard `root` (e.g. a
    // bare `testdata/` dir) get an unrelated home — fine as long as
    // they don't query `config`.
    let home = root
        .parent()
        .and_then(|p| p.parent())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| root.to_path_buf());
    ctx.register_table("config", Arc::new(ConfigTable::new(home)))
        .unwrap();
    ctx
}
