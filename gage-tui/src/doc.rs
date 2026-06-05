//! Document model for the TUI.
//!
//! `Document` owns the session content. `Session` and `Entry` both wrap a
//! parsed JSON value (the source of truth for rendering) and expose accessors
//! plus a `yaml()` serializer. The two follow the same pattern so the body
//! pane renders them through the same highlighter path.

use serde_json::Value;

pub struct Document {
    pub session: Session,
    pub entries: Vec<Entry>,
}

pub struct Session {
    pub id: String,
    pub value: Value,
}

impl Session {
    pub fn yaml(&self) -> String {
        serde_yml::to_string(&self.value).expect("Value is always YAML serializable")
    }
}

pub struct Entry {
    pub value: Value,
}

impl Entry {
    pub fn entry_type(&self) -> &str {
        self.value.get("type").and_then(Value::as_str).unwrap_or("")
    }

    /// Outline label — the subtype when meaningful (e.g. `tool_use`,
    /// `thinking`, `tool_result`, `meta`), otherwise the raw type. Mirrors
    /// the labeling used by `gage eval view`.
    pub fn label(&self) -> &str {
        match gage_claude::entry::entry_subtype(&self.value) {
            Some("text") | None => self.entry_type(),
            Some(sub) => sub,
        }
    }

    pub fn message(&self) -> Option<&Value> {
        self.value.get("message")
    }

    pub fn yaml(&self) -> String {
        serde_yml::to_string(&self.value).expect("Value is always YAML serializable")
    }
}
