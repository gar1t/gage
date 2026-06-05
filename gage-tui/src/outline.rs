//! Outline tree state + visible-row projection.
//!
//! `Outline` owns the tree *shape* (what's expanded, what's visible). Row
//! content — labels for the session, entry titles — is composed by the
//! renderer from the `Document`, keyed off `RowKind::Entry { index }`. This
//! split keeps Outline a pure UI structure with no per-entry data duplication.

pub struct Outline {
    entry_count: usize,
    session_expanded: bool,
    visible: Vec<Row>,
}

pub struct Row {
    pub level: usize,
    pub has_children: bool,
    pub expanded: bool,
    pub kind: RowKind,
}

pub enum RowKind {
    Session,
    Entry { index: usize },
}

/// Outcome of a `Left`-style collapse. The caller decides what to do with the
/// selection — collapse may invalidate descendants; "no children to collapse"
/// promotes the gesture into a move-to-parent.
pub enum CollapseOutcome {
    /// Subtree collapsed. The caller should clamp selection if it pointed
    /// into the collapsed subtree.
    Collapsed,
    /// No collapse possible here; caller should select this row instead.
    SelectParent(usize),
    /// Nothing to do (no children, no parent).
    None,
}

impl Outline {
    pub fn new(entry_count: usize) -> Self {
        let mut o = Self {
            entry_count,
            session_expanded: true,
            visible: Vec::new(),
        };
        o.rebuild();
        o
    }

    pub fn rows(&self) -> &[Row] {
        &self.visible
    }

    pub fn row(&self, idx: usize) -> Option<&Row> {
        self.visible.get(idx)
    }

    pub fn len(&self) -> usize {
        self.visible.len()
    }

    /// Toggle the selected row's expansion. Returns true if anything changed.
    pub fn toggle(&mut self, idx: usize) -> bool {
        let Some(row) = self.visible.get(idx) else {
            return false;
        };
        if !row.has_children {
            return false;
        }
        let expanded = row.expanded;
        self.set_expanded(idx, !expanded);
        true
    }

    /// Expand the selected row if it has children and is currently collapsed.
    pub fn expand(&mut self, idx: usize) -> bool {
        let Some(row) = self.visible.get(idx) else {
            return false;
        };
        if !row.has_children || row.expanded {
            return false;
        }
        self.set_expanded(idx, true);
        true
    }

    /// Collapse the selected row, or — when it has no children to collapse —
    /// instruct the caller to move selection to the parent.
    pub fn collapse(&mut self, idx: usize) -> CollapseOutcome {
        let Some(row) = self.visible.get(idx) else {
            return CollapseOutcome::None;
        };
        if row.has_children && row.expanded {
            self.set_expanded(idx, false);
            return CollapseOutcome::Collapsed;
        }
        if row.level > 1 {
            return CollapseOutcome::SelectParent(self.parent_of(idx));
        }
        CollapseOutcome::None
    }

    fn set_expanded(&mut self, idx: usize, expanded: bool) {
        let Some(row) = self.visible.get(idx) else {
            return;
        };
        match row.kind {
            RowKind::Session => {
                self.session_expanded = expanded;
                self.rebuild();
            }
            RowKind::Entry { .. } => {}
        }
    }

    fn parent_of(&self, idx: usize) -> usize {
        let Some(row) = self.visible.get(idx) else {
            return 0;
        };
        let parent_level = row.level.saturating_sub(1);
        for j in (0..idx).rev() {
            if let Some(r) = self.visible.get(j)
                && r.level <= parent_level
            {
                return j;
            }
        }
        0
    }

    fn rebuild(&mut self) {
        let mut rows = Vec::with_capacity(1 + self.entry_count);
        rows.push(Row {
            level: 1,
            has_children: self.entry_count > 0,
            expanded: self.session_expanded,
            kind: RowKind::Session,
        });
        if self.session_expanded {
            for i in 0..self.entry_count {
                rows.push(Row {
                    level: 2,
                    has_children: false,
                    expanded: false,
                    kind: RowKind::Entry { index: i },
                });
            }
        }
        self.visible = rows;
    }
}
