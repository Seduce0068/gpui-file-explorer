// SPDX-License-Identifier: AGPL-3.0-or-later
//! Back / forward navigation history.

use std::path::PathBuf;

/// Intent of a navigation (DESIGN.md §5.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavMode {
    /// Ordinary navigation; records `current` on the back stack.
    Push,
    /// Move to the previous entry.
    Back,
    /// Move to the next entry.
    Forward,
    /// Move to a parent directory (same history semantics as `Push`).
    Up,
    /// Change location without touching the history stacks.
    Replace,
}

/// Browser-style navigation history with a bounded back stack.
#[derive(Debug, Default, Clone)]
pub struct History {
    back: Vec<PathBuf>,
    forward: Vec<PathBuf>,
    max_back: usize,
}

impl History {
    pub fn new() -> Self {
        Self {
            back: Vec::new(),
            forward: Vec::new(),
            max_back: 200,
        }
    }

    /// Record a navigation from `current` to `next`.
    pub fn visit(&mut self, current: PathBuf, next: PathBuf) {
        if current == next {
            return;
        }
        self.back.push(current);
        if self.back.len() > self.max_back {
            self.back.remove(0);
        }
        self.forward.clear();
    }

    /// Leave `current` and return the previous directory, if any.
    pub fn go_back(&mut self, current: PathBuf) -> Option<PathBuf> {
        let target = self.back.pop()?;
        self.forward.push(current);
        Some(target)
    }

    /// Leave `current` and return the next directory in the forward stack.
    pub fn go_forward(&mut self, current: PathBuf) -> Option<PathBuf> {
        let target = self.forward.pop()?;
        self.back.push(current);
        Some(target)
    }

    /// Apply a navigation and return the next location. `target` is only used
    /// by `Push`, `Up` and `Replace`; `Back` / `Forward` ignore it.
    pub fn apply(&mut self, current: PathBuf, target: PathBuf, mode: NavMode) -> Option<PathBuf> {
        match mode {
            NavMode::Push | NavMode::Up => {
                self.visit(current, target.clone());
                Some(target)
            }
            NavMode::Back => self.go_back(current),
            NavMode::Forward => self.go_forward(current),
            NavMode::Replace => Some(target),
        }
    }

    pub fn can_go_back(&self) -> bool {
        !self.back.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.forward.is_empty()
    }

    #[allow(dead_code)]
    pub fn back_len(&self) -> usize {
        self.back.len()
    }

    #[allow(dead_code)]
    pub fn forward_len(&self) -> usize {
        self.forward.len()
    }
}

#[cfg(test)]
mod tests {
    // Explicit imports: `use gpui_kit::*` in the crate root would shadow
    // the built-in `#[test]` attribute through a glob import.
    use super::{History, NavMode};
    use std::path::PathBuf;

    fn p(value: &str) -> PathBuf {
        PathBuf::from(value)
    }

    #[test]
    fn starts_empty() {
        let history = History::new();
        assert!(!history.can_go_back());
        assert!(!history.can_go_forward());
    }

    #[test]
    fn back_and_forward_walk_the_stack() {
        let mut history = History::new();
        history.visit(p("/a"), p("/b"));
        history.visit(p("/b"), p("/c"));
        assert!(history.can_go_back());
        assert!(!history.can_go_forward());

        assert_eq!(history.go_back(p("/c")), Some(p("/b")));
        assert_eq!(history.go_back(p("/b")), Some(p("/a")));
        assert_eq!(history.go_back(p("/a")), None);
        assert!(history.can_go_forward());

        assert_eq!(history.go_forward(p("/a")), Some(p("/b")));
        assert_eq!(history.go_forward(p("/b")), Some(p("/c")));
        assert_eq!(history.go_forward(p("/c")), None);
    }

    #[test]
    fn new_navigation_clears_forward_stack() {
        let mut history = History::new();
        history.visit(p("/a"), p("/b"));
        history.visit(p("/b"), p("/c"));
        history.go_back(p("/c"));
        assert!(history.can_go_forward());
        history.visit(p("/b"), p("/x"));
        assert!(!history.can_go_forward());
    }

    #[test]
    fn same_path_is_not_recorded() {
        let mut history = History::new();
        history.visit(p("/a"), p("/a"));
        assert!(!history.can_go_back());
        assert_eq!(history.back_len(), 0);
    }

    #[test]
    fn max_back_is_bounded() {
        let mut history = History::new();
        history.max_back = 2;
        history.visit(p("/a"), p("/b"));
        history.visit(p("/b"), p("/c"));
        history.visit(p("/c"), p("/d"));
        assert_eq!(history.back_len(), 2);
        assert_eq!(history.go_back(p("/d")), Some(p("/c")));
        assert_eq!(history.go_back(p("/c")), Some(p("/b")));
        assert_eq!(history.go_back(p("/b")), None);
    }

    #[test]
    fn apply_covers_all_nav_modes() {
        let mut history = History::new();
        assert_eq!(
            history.apply(p("/a"), p("/b"), NavMode::Push),
            Some(p("/b"))
        );
        assert_eq!(history.back_len(), 1);
        assert_eq!(
            history.apply(p("/b"), PathBuf::new(), NavMode::Back),
            Some(p("/a"))
        );
        assert_eq!(
            history.apply(p("/a"), PathBuf::new(), NavMode::Forward),
            Some(p("/b"))
        );
        assert_eq!(
            history.apply(p("/b"), p("/b/child"), NavMode::Replace),
            Some(p("/b/child"))
        );
        assert_eq!(history.back_len(), 1, "Replace must not change history");
        assert_eq!(
            history.apply(p("/b/child"), p("/"), NavMode::Up),
            Some(p("/"))
        );
        assert_eq!(history.back_len(), 2);
    }
}
