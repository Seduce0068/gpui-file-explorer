// SPDX-License-Identifier: AGPL-3.0-or-later
//! Back / forward navigation history.

use std::path::{Path, PathBuf};

/// Default upper bound of the back stack. Shared by [`History::new`] and
/// [`History::default`] so the two constructors cannot drift apart.
///
/// `pub(crate)` so `settings` can assert that its persisted
/// `MAX_HISTORY_ENTRIES` cap stays equal to this runtime limit: the two are
/// declared independently and a drift would silently truncate or overfill a
/// restored session (V09).
pub(crate) const DEFAULT_MAX_BACK: usize = 200;

/// Drop the oldest elements so at most `keep` remain.
///
/// `Vec::truncate` keeps the *front* of a vector. That is wrong for the
/// oldest-first back stack: it would throw away exactly the recent paths the
/// user wants to navigate back to. Keeping the tail is the rule here and at
/// the settings boundary, so both go through this helper and cannot drift.
pub(crate) fn retain_last_n<T>(items: &mut Vec<T>, keep: usize) {
    let len = items.len();
    if len > keep {
        items.drain(..len - keep);
    }
}

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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct History {
    back: Vec<PathBuf>,
    forward: Vec<PathBuf>,
    max_back: usize,
}

/// A lossless, order-preserving view of both navigation stacks.
///
/// `back` is oldest-first (the last entry is the next [`History::go_back`]
/// target) and `forward` is next-first (the first entry is the next
/// [`History::go_forward`] target). This is the same convention used by the
/// persisted `TabSession` fields, so a snapshot can be written to — and
/// restored from — settings without probing navigation side effects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistorySnapshot {
    /// Oldest-first; the last entry is the next back target.
    pub back: Vec<PathBuf>,
    /// Next-first; the first entry is the next forward target.
    pub forward: Vec<PathBuf>,
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

impl History {
    pub fn new() -> Self {
        Self {
            back: Vec::new(),
            forward: Vec::new(),
            max_back: DEFAULT_MAX_BACK,
        }
    }

    /// Copy both navigation stacks without changing this history.
    pub fn snapshot(&self) -> HistorySnapshot {
        HistorySnapshot {
            back: self.back.clone(),
            // The internal stack is last-in-first-out; expose it next-first.
            forward: self.forward.iter().rev().cloned().collect(),
        }
    }

    /// Replace both navigation stacks from `snapshot`.
    ///
    /// `back` is read oldest-first and `forward` next-first, matching
    /// [`HistorySnapshot`]. The combined size is bounded by `max_back`: the
    /// forward stack keeps its next entries and the back stack keeps its most
    /// recent entries, preserving the invariant [`History::visit`] maintains
    /// for histories built through navigation.
    pub fn restore(&mut self, snapshot: HistorySnapshot) {
        // `snapshot.forward` is next-first, so the reversed internal stack has
        // the farthest entry first. Drop the excess from that front: the nearest
        // entries (the next go_forward targets) must survive, as documented
        // above and as the back-stack rule below does for recent entries.
        let mut forward: Vec<PathBuf> = snapshot.forward.into_iter().rev().collect();
        retain_last_n(&mut forward, self.max_back);
        let back_budget = self.max_back.saturating_sub(forward.len());
        let mut back = snapshot.back;
        retain_last_n(&mut back, back_budget);
        self.back = back;
        self.forward = forward;
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

    /// Return the directory a back navigation would land on, without touching
    /// either stack. Callers can validate the target (for example with
    /// `Path::is_dir`) before committing the navigation via
    /// [`History::apply`] with [`NavMode::Back`].
    ///
    /// `current` is accepted for symmetry with [`History::go_back`]; the target
    /// itself depends only on the back stack.
    pub fn peek_back(&self, _current: &Path) -> Option<PathBuf> {
        self.back.last().cloned()
    }

    /// Return the directory a forward navigation would land on, without
    /// touching either stack. Callers can validate the target before
    /// committing via [`History::apply`] with [`NavMode::Forward`].
    ///
    /// `current` is accepted for symmetry with [`History::go_forward`]; the
    /// target itself depends only on the forward stack.
    pub fn peek_forward(&self, _current: &Path) -> Option<PathBuf> {
        self.forward.last().cloned()
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

    /// Test-only inspection of the back stack length.
    #[cfg(test)]
    pub fn back_len(&self) -> usize {
        self.back.len()
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
    fn retain_last_n_drops_the_oldest_elements() {
        let mut values: Vec<i32> = (0..5).collect();
        super::retain_last_n(&mut values, 3);
        assert_eq!(values, vec![2, 3, 4], "the recent tail must survive");
        super::retain_last_n(&mut values, 10);
        assert_eq!(values, vec![2, 3, 4], "a larger budget must be a no-op");
        super::retain_last_n(&mut values, 0);
        assert!(values.is_empty());
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

    #[test]
    fn default_matches_new() {
        assert_eq!(History::default(), History::new());
    }

    #[test]
    fn default_has_full_back_capacity() {
        let mut history = History::default();
        for index in 0..super::DEFAULT_MAX_BACK + 1 {
            history.visit(p(&format!("/d{index}")), p(&format!("/d{}", index + 1)));
        }
        assert_eq!(history.back_len(), super::DEFAULT_MAX_BACK);
    }

    #[test]
    fn snapshot_and_restore_preserve_both_stacks() {
        let mut history = History::new();
        history.visit(p("/a"), p("/b"));
        history.visit(p("/b"), p("/c"));
        history.visit(p("/c"), p("/d"));
        history.go_back(p("/d"));
        history.go_back(p("/c"));
        let original = history.clone();

        let snapshot = history.snapshot();
        assert_eq!(snapshot.back, vec![p("/a")]);
        assert_eq!(snapshot.forward, vec![p("/c"), p("/d")]);
        assert_eq!(history, original, "snapshot must not mutate the history");

        let mut restored = History::new();
        restored.restore(snapshot);
        assert_eq!(restored, original, "restore must reproduce both stacks");
    }

    #[test]
    fn snapshot_and_restore_handle_empty_stacks() {
        let snapshot = History::new().snapshot();
        assert!(snapshot.back.is_empty());
        assert!(snapshot.forward.is_empty());
        let mut restored = History::new();
        restored.restore(snapshot);
        assert_eq!(restored, History::new());
    }

    #[test]
    fn restore_keeps_the_bound_and_prefers_the_next_entries() {
        let mut history = History::new();
        history.max_back = 3;
        history.restore(super::HistorySnapshot {
            back: vec![p("/oldest"), p("/a"), p("/b")],
            forward: vec![p("/next"), p("/later"), p("/last")],
        });
        // Forward has priority (3 entries fill the budget), so all back
        // entries are dropped.
        assert_eq!(history.snapshot().back, Vec::<PathBuf>::new());
        assert_eq!(
            history.snapshot().forward,
            vec![p("/next"), p("/later"), p("/last")]
        );

        // With room left, the most recent back entries survive (oldest-first).
        let mut history = History::new();
        history.max_back = 3;
        history.restore(super::HistorySnapshot {
            back: vec![p("/oldest"), p("/a"), p("/b")],
            forward: vec![p("/next")],
        });
        assert_eq!(history.snapshot().back, vec![p("/a"), p("/b")]);
        assert_eq!(history.snapshot().forward, vec![p("/next")]);
    }

    #[test]
    fn restore_truncates_forward_to_the_nearest_entries() {
        let mut history = History::new();
        history.max_back = 3;
        history.restore(super::HistorySnapshot {
            back: vec![p("/base")],
            // snapshot.forward は next-first（先頭が次の go_forward ターゲット）。
            forward: vec![p("/f1"), p("/f2"), p("/f3"), p("/f4"), p("/f5")],
        });
        // 上限 3 を超えた分は遠い方から捨てられ、直近の 3 件が残る。
        assert_eq!(
            history.snapshot().forward,
            vec![p("/f1"), p("/f2"), p("/f3")]
        );
        assert_eq!(history.snapshot().back, Vec::<PathBuf>::new());
        // 残った先頭が実際の go_forward ターゲットになる。
        assert_eq!(history.go_forward(p("/base")), Some(p("/f1")));
        assert_eq!(history.go_forward(p("/f1")), Some(p("/f2")));
    }

    #[test]
    fn peek_back_does_not_mutate() {
        let mut history = History::new();
        history.visit(p("/a"), p("/b"));
        history.visit(p("/b"), p("/c"));
        let snapshot = history.clone();

        assert_eq!(history.peek_back(&p("/c")), Some(p("/b")));
        assert_eq!(history.peek_back(&p("/c")), Some(p("/b")));
        assert_eq!(history, snapshot, "peek_back must not change any stack");

        // The peeked value is exactly what the committed navigation returns.
        assert_eq!(history.go_back(p("/c")), Some(p("/b")));
    }

    #[test]
    fn peek_forward_does_not_mutate() {
        let mut history = History::new();
        history.visit(p("/a"), p("/b"));
        history.visit(p("/b"), p("/c"));
        assert_eq!(history.go_back(p("/c")), Some(p("/b")));
        assert_eq!(history.go_back(p("/b")), Some(p("/a")));
        let snapshot = history.clone();

        assert_eq!(history.peek_forward(&p("/a")), Some(p("/b")));
        assert_eq!(history.peek_forward(&p("/a")), Some(p("/b")));
        assert_eq!(history, snapshot, "peek_forward must not change any stack");

        assert_eq!(history.go_forward(p("/a")), Some(p("/b")));
    }

    #[test]
    fn peek_is_none_when_stack_is_empty() {
        let history = History::new();
        assert_eq!(history.peek_back(&p("/a")), None);
        assert_eq!(history.peek_forward(&p("/a")), None);
        assert_eq!(history, History::new());
    }
}
