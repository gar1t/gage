pub mod config;
pub mod entry;
pub mod issue;
pub mod issue_evidence;
pub mod message;
pub mod note;
pub mod session;

pub use config::ConfigTable;
pub use entry::EntryTable;
pub use issue::IssueTable;
pub use issue_evidence::IssueEvidenceTable;
pub use message::{MessageTable, entry_text};
pub use note::NoteTable;
pub use session::SessionTable;

use std::path::PathBuf;

/// Where a session-row table provider (`EntryTable`, `MessageTable`)
/// finds its data.
///
/// `Root` walks every session under a Claude Code projects directory
/// — the global `gage query` use case. `SingleSession` reads exactly
/// one session file, which is the per-session scanner context built by
/// `gage-scan`'s runner.
#[derive(Debug, Clone)]
pub(super) enum SessionSource {
    Root(PathBuf),
    SingleSession { session_id: String, path: PathBuf },
}
