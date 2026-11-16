// SPDX-License-Identifier: AGPL-3.0-or-later
//! Undo for the two operations that can be reversed **safely**:
//!
//! - **rename**: rename the new name back to the old one,
//! - **trash**: restore the trashed items from the system trash.
//!
//! Copy and move are deliberately *not* undoable: undo would have to delete
//! files that the user may have already modified or moved away, which violates
//! the "never delete data on the user's behalf" rule. The UI says so
//! explicitly instead of pretending to undo them.
//!
//! The undo stack keeps at most [`MAX_UNDO`] entries; recording a new entry
//! clears the redo stack. This module is GPUI-free so the stack can be unit
//! tested headlessly.

use std::path::PathBuf;

/// Maximum number of undoable operations kept in memory.
pub const MAX_UNDO: usize = 20;

/// One reversible operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UndoAction {
    /// A rename from `from` to `to`; undo renames `to` back to `from`.
    Rename { from: PathBuf, to: PathBuf },
    /// Items moved to the system trash; undo restores them.
    ///
    /// Each entry pairs the original path with the trash item id, so undo can
    /// find the exact item even if the trash changed meanwhile.
    Trash { items: Vec<(PathBuf, String)> },
}

impl UndoAction {
    /// Short label used in the status bar.
    pub fn label(&self) -> &'static str {
        match self {
            UndoAction::Rename { .. } => "名前変更",
            UndoAction::Trash { .. } => "ゴミ箱へ移動",
        }
    }

    /// Number of items the operation affected.
    pub fn len(&self) -> usize {
        match self {
            UndoAction::Rename { .. } => 1,
            UndoAction::Trash { items } => items.len(),
        }
    }
}

/// Bounded undo/redo stack.
#[derive(Debug, Default, Clone)]
pub struct UndoStack {
    undo: Vec<UndoAction>,
    redo: Vec<UndoAction>,
}

impl UndoStack {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a finished operation, clearing the redo stack.
    pub fn push(&mut self, action: UndoAction) {
        self.redo.clear();
        self.undo.push(action);
        if self.undo.len() > MAX_UNDO {
            self.undo.remove(0);
        }
    }

    /// Take the most recent action to undo.
    pub fn pop_undo(&mut self) -> Option<UndoAction> {
        self.undo.pop()
    }

    /// Put an action back after a successful undo (so it can be redone).
    pub fn push_redo(&mut self, action: UndoAction) {
        self.redo.push(action);
    }

    /// Take the most recent action to redo.
    pub fn pop_redo(&mut self) -> Option<UndoAction> {
        self.redo.pop()
    }

    /// Put an action back after a successful redo.
    pub fn push_undo(&mut self, action: UndoAction) {
        self.undo.push(action);
        if self.undo.len() > MAX_UNDO {
            self.undo.remove(0);
        }
    }

    /// Snapshot of the undo stack, newest first (for the history overlay).
    pub fn undo_actions(&self) -> Vec<UndoAction> {
        self.undo.iter().rev().cloned().collect()
    }

    /// Snapshot of the redo stack, newest first (for the history overlay).
    pub fn redo_actions(&self) -> Vec<UndoAction> {
        self.redo.iter().rev().cloned().collect()
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }
}

#[cfg(test)]
mod tests {
    use super::{UndoAction, UndoStack, MAX_UNDO};
    use std::path::PathBuf;

    fn p(value: &str) -> PathBuf {
        PathBuf::from(value)
    }

    fn rename(from: &str, to: &str) -> UndoAction {
        UndoAction::Rename {
            from: p(from),
            to: p(to),
        }
    }

    #[test]
    fn stack_is_lifo_and_tracks_redo() {
        let mut stack = UndoStack::new();
        stack.push(rename("/a", "/b"));
        stack.push(rename("/b", "/c"));
        assert!(stack.can_undo());
        assert!(!stack.can_redo());

        let action = stack.pop_undo().expect("undo");
        assert_eq!(action, rename("/b", "/c"));
        stack.push_redo(action);
        assert!(stack.can_redo());

        let redone = stack.pop_redo().expect("redo");
        assert_eq!(redone, rename("/b", "/c"));
        stack.push_undo(redone);
        assert_eq!(stack.undo_len(), 2);
    }

    #[test]
    fn new_action_clears_redo() {
        let mut stack = UndoStack::new();
        stack.push(rename("/a", "/b"));
        let action = stack.pop_undo().unwrap();
        stack.push_redo(action);
        assert!(stack.can_redo());
        stack.push(rename("/x", "/y"));
        assert!(!stack.can_redo());
    }

    #[test]
    fn stack_is_bounded() {
        let mut stack = UndoStack::new();
        for index in 0..(MAX_UNDO + 5) {
            stack.push(rename(&format!("/old-{index}"), &format!("/new-{index}")));
        }
        assert_eq!(stack.undo_len(), MAX_UNDO);
        // The newest entry survives; the oldest were dropped.
        let newest = stack.pop_undo().unwrap();
        assert_eq!(
            newest,
            rename(
                &format!("/old-{}", MAX_UNDO + 4),
                &format!("/new-{}", MAX_UNDO + 4)
            )
        );
    }

    #[test]
    fn push_undo_after_redo_is_also_bounded() {
        let mut stack = UndoStack::new();
        for index in 0..(MAX_UNDO + 3) {
            stack.push_undo(rename(&format!("/a-{index}"), &format!("/b-{index}")));
        }
        assert_eq!(stack.undo_len(), MAX_UNDO);
        let newest = stack.pop_undo().unwrap();
        assert_eq!(
            newest,
            rename(
                &format!("/a-{}", MAX_UNDO + 2),
                &format!("/b-{}", MAX_UNDO + 2)
            )
        );
    }

    #[test]
    fn trash_action_reports_item_count() {
        let action = UndoAction::Trash {
            items: vec![
                (p("/a.txt"), "id-1".to_string()),
                (p("/b.txt"), "id-2".to_string()),
            ],
        };
        assert_eq!(action.len(), 2);
        assert_eq!(action.label(), "ゴミ箱へ移動");
        assert_eq!(rename("/a", "/b").len(), 1);
        assert_eq!(rename("/a", "/b").label(), "名前変更");
    }

    #[test]
    fn snapshots_list_newest_first() {
        let mut stack = UndoStack::new();
        stack.push(rename("/a", "/b"));
        stack.push(rename("/b", "/c"));
        let undo = stack.undo_actions();
        assert_eq!(undo.len(), 2);
        assert_eq!(undo[0], rename("/b", "/c"));
        assert_eq!(undo[1], rename("/a", "/b"));

        let action = stack.pop_undo().unwrap();
        stack.push_redo(action);
        let redo = stack.redo_actions();
        assert_eq!(redo.len(), 1);
        assert_eq!(redo[0], rename("/b", "/c"));
        assert!(stack.undo_actions().len() == 1);
    }

    #[test]
    fn empty_stack_takes_nothing() {
        let mut stack = UndoStack::new();
        assert!(stack.pop_undo().is_none());
        assert!(stack.pop_redo().is_none());
        assert!(!stack.can_undo());
        assert!(!stack.can_redo());
    }
}
