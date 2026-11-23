// SPDX-License-Identifier: AGPL-3.0-or-later
//! Tab state for the explorer: each tab owns a directory and its navigation
//! history. Pure state (no GPUI) so switching/closing rules can be tested.

use std::path::PathBuf;

use crate::history::History;
use crate::settings::MAX_TABS;

/// One tab: a directory plus its back/forward history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tab {
    pub cwd: PathBuf,
    pub history: History,
}

impl Tab {
    pub fn new(cwd: PathBuf) -> Self {
        Self {
            cwd,
            history: History::new(),
        }
    }
}

/// Ordered tabs with one active tab. Invariant: `tabs` is never empty.
#[derive(Debug, Clone)]
pub struct Tabs {
    tabs: Vec<Tab>,
    active: usize,
    /// Recently closed tabs, most recent last. The whole tab (cwd *and*
    /// back/forward history) is kept so Ctrl+Shift+T matches the session
    /// restore path, which also reattaches per-tab history (BE07).
    /// In-memory only.
    closed: Vec<Tab>,
}

impl Tabs {
    pub fn new(cwd: PathBuf) -> Self {
        Self {
            tabs: vec![Tab::new(cwd)],
            active: 0,
            closed: Vec::new(),
        }
    }

    /// Restore tabs from persisted directories. Returns `None` when the list
    /// is empty (callers fall back to a single default tab). The active index
    /// is clamped into range.
    pub fn from_paths(paths: Vec<PathBuf>, active: usize) -> Option<Self> {
        if paths.is_empty() {
            return None;
        }
        let tabs: Vec<Tab> = paths.into_iter().map(Tab::new).collect();
        let active = active.min(tabs.len() - 1);
        Some(Self {
            tabs,
            active,
            closed: Vec::new(),
        })
    }

    pub fn len(&self) -> usize {
        self.tabs.len()
    }

    pub fn active_index(&self) -> usize {
        self.active
    }

    pub fn active(&self) -> &Tab {
        &self.tabs[self.active]
    }

    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    /// Store the current directory/history into the active tab (called after
    /// navigation before switching/closing tabs).
    pub fn sync(&mut self, cwd: PathBuf, history: History) {
        if let Some(tab) = self.tabs.get_mut(self.active) {
            tab.cwd = cwd;
            tab.history = history;
        }
    }

    /// True while another tab may be opened. The runtime cap is a state
    /// invariant, not just a UI hint: every path that creates a tab must check
    /// this (the app routes all four through its `try_new_tab` guard, BE01).
    pub fn can_open_tab(&self) -> bool {
        self.tabs.len() < MAX_TABS
    }

    /// Append a new tab and make it active. Returns its index. Callers must
    /// have checked [`Self::can_open_tab`] first; unguarded callers could push
    /// past `MAX_TABS`.
    pub fn new_tab(&mut self, cwd: PathBuf) -> usize {
        self.tabs.push(Tab::new(cwd));
        self.active = self.tabs.len() - 1;
        self.active
    }

    /// Make `index` active and return a copy of the tab, or `None` when the
    /// index is out of range or already active.
    pub fn activate(&mut self, index: usize) -> Option<Tab> {
        if index >= self.tabs.len() || index == self.active {
            return None;
        }
        self.active = index;
        Some(self.tabs[index].clone())
    }

    /// Close the active tab. Returns the new active tab, or `None` when this is
    /// the last tab (closing it is refused).
    pub fn close_active(&mut self) -> Option<Tab> {
        if self.tabs.len() <= 1 {
            return None;
        }
        let closed = self.tabs.remove(self.active);
        self.closed.push(closed);
        if self.active >= self.tabs.len() {
            self.active = self.tabs.len() - 1;
        }
        Some(self.tabs[self.active].clone())
    }

    /// Reopen the most recently closed tab with its cwd *and* history intact.
    /// Returns the restored tab, or `None` when there is nothing to restore or
    /// the cap is reached. The cap is checked before popping so a refused
    /// restore never discards the closed tab (BE01/BE07).
    pub fn restore_last_closed(&mut self) -> Option<Tab> {
        if !self.can_open_tab() {
            return None;
        }
        let tab = self.closed.pop()?;
        self.tabs.push(tab.clone());
        self.active = self.tabs.len() - 1;
        Some(tab)
    }

    /// Move the active tab one position to the left. Returns `false` when it
    /// is already the first tab.
    pub fn move_active_left(&mut self) -> bool {
        if self.active == 0 {
            return false;
        }
        self.tabs.swap(self.active, self.active - 1);
        self.active -= 1;
        true
    }

    /// Move the active tab one position to the right. Returns `false` when it
    /// is already the last tab.
    pub fn move_active_right(&mut self) -> bool {
        if self.active + 1 >= self.tabs.len() {
            return false;
        }
        self.tabs.swap(self.active, self.active + 1);
        self.active += 1;
        true
    }

    /// Activate the next tab (wrapping around).
    pub fn next(&mut self) -> Tab {
        self.active = (self.active + 1) % self.tabs.len();
        self.tabs[self.active].clone()
    }

    /// Activate the previous tab (wrapping around).
    pub fn prev(&mut self) -> Tab {
        self.active = (self.active + self.tabs.len() - 1) % self.tabs.len();
        self.tabs[self.active].clone()
    }
}

#[cfg(test)]
mod tests {
    use super::Tabs;
    use crate::history::History;
    use std::path::PathBuf;

    fn p(value: &str) -> PathBuf {
        PathBuf::from(value)
    }

    #[test]
    fn starts_with_one_active_tab() {
        let tabs = Tabs::new(p("/a"));
        assert_eq!(tabs.len(), 1);
        assert_eq!(tabs.active_index(), 0);
        assert_eq!(tabs.active().cwd, p("/a"));
    }

    #[test]
    fn sync_updates_only_the_active_tab() {
        let mut tabs = Tabs::new(p("/a"));
        tabs.new_tab(p("/b"));
        let mut history = History::new();
        history.visit(p("/b"), p("/c"));
        let expected = history.snapshot();
        assert_eq!(
            expected.back,
            vec![p("/b")],
            "fixture must record a back entry"
        );
        assert!(expected.forward.is_empty());
        tabs.sync(p("/c"), history);
        assert_eq!(tabs.tabs()[1].cwd, p("/c"));
        assert_eq!(tabs.tabs()[0].cwd, p("/a"));
        // The history must actually be stored in the active tab, not merely
        // used to update the cwd; the other tab keeps its empty history.
        assert_eq!(
            tabs.tabs()[1].history.snapshot(),
            expected,
            "synced history must be saved into the active tab"
        );
        assert_eq!(
            tabs.tabs()[1].history.peek_back(&p("/c")),
            Some(p("/b")),
            "the back navigation must be usable after the sync"
        );
        assert_eq!(
            tabs.tabs()[0].history.snapshot(),
            History::new().snapshot(),
            "inactive tab's history must be untouched"
        );
    }

    #[test]
    fn activate_switches_and_rejects_invalid_or_same() {
        let mut tabs = Tabs::new(p("/a"));
        tabs.new_tab(p("/b"));
        assert!(tabs.activate(1).is_none(), "already active");
        let tab = tabs.activate(0).expect("valid index");
        assert_eq!(tab.cwd, p("/a"));
        assert_eq!(tabs.active_index(), 0);
        assert!(tabs.activate(5).is_none());
    }

    #[test]
    fn closing_the_last_tab_is_refused() {
        let mut tabs = Tabs::new(p("/a"));
        assert!(tabs.close_active().is_none());
        assert_eq!(tabs.len(), 1);
    }

    #[test]
    fn close_active_selects_a_neighbour() {
        let mut tabs = Tabs::new(p("/a"));
        tabs.new_tab(p("/b"));
        tabs.new_tab(p("/c"));
        // active = /c (index 2); closing selects index 1 (/b)
        let next = tabs.close_active().expect("closed");
        assert_eq!(next.cwd, p("/b"));
        assert_eq!(tabs.len(), 2);
        assert_eq!(tabs.active_index(), 1);
    }

    #[test]
    fn close_active_keeps_index_when_closing_a_non_last_tab() {
        let mut tabs = Tabs::new(p("/a"));
        tabs.new_tab(p("/b"));
        tabs.new_tab(p("/c"));
        // active = /a (index 0); closing a non-last tab keeps the index,
        // so the tab that shifts into index 0 (/b) becomes active.
        assert!(tabs.activate(0).is_some(), "make /a active");
        let next = tabs.close_active().expect("closed");
        assert_eq!(next.cwd, p("/b"));
        assert_eq!(tabs.active_index(), 0);
        assert_eq!(tabs.len(), 2);
        let order: Vec<_> = tabs.tabs().iter().map(|tab| tab.cwd.clone()).collect();
        assert_eq!(order, vec![p("/b"), p("/c")]);
    }

    #[test]
    fn restore_last_closed_reopens_most_recent_tab() {
        let mut tabs = Tabs::new(p("/a"));
        tabs.new_tab(p("/b"));
        tabs.new_tab(p("/c"));
        // Close /c then /b; the most recent closed is /b.
        tabs.close_active();
        tabs.close_active();
        assert_eq!(tabs.len(), 1);

        assert_eq!(tabs.restore_last_closed().map(|tab| tab.cwd), Some(p("/b")));
        assert_eq!(tabs.len(), 2);
        assert_eq!(tabs.active().cwd, p("/b"));

        assert_eq!(tabs.restore_last_closed().map(|tab| tab.cwd), Some(p("/c")));
        assert_eq!(tabs.len(), 3);
        assert_eq!(tabs.restore_last_closed(), None);
    }

    #[test]
    fn restore_last_closed_reattaches_back_history() {
        let mut tabs = Tabs::new(p("/a"));
        tabs.new_tab(p("/b"));
        // Give the /b tab a real back entry before closing it.
        let mut history = History::new();
        history.visit(p("/b"), p("/child"));
        tabs.sync(p("/child"), history);

        assert!(tabs.close_active().is_some(), "close /child (/b) tab");
        let restored = tabs
            .restore_last_closed()
            .expect("closed tab must be restored");
        assert_eq!(restored.cwd, p("/child"));
        assert_eq!(
            restored.history.peek_back(&restored.cwd),
            Some(p("/b")),
            "the restored tab must keep the history that go_back() peeks"
        );
        let expected = restored.history.snapshot();
        assert_eq!(tabs.active().history.snapshot(), expected);
    }

    #[test]
    fn can_open_tab_enforces_the_cap_at_the_31_32_boundary() {
        use super::MAX_TABS;

        assert_eq!(MAX_TABS, 32, "boundary fixtures assume the current cap");
        let mut tabs = Tabs::new(p("/0"));
        for index in 1..MAX_TABS - 1 {
            assert!(
                tabs.can_open_tab(),
                "tab {index} must still be below the cap"
            );
            tabs.new_tab(p(&format!("/{index}")));
        }
        assert_eq!(tabs.len(), MAX_TABS - 1);
        assert!(tabs.can_open_tab(), "31 tabs: one slot remains");

        tabs.new_tab(p("/31"));
        assert_eq!(tabs.len(), MAX_TABS);
        assert!(!tabs.can_open_tab(), "32 tabs: the cap is reached");

        // A refused restore must not consume the closed tab. (`close_active`
        // returns the tab that becomes active, not the closed one.)
        tabs.close_active().expect("close a tab");
        assert_eq!(tabs.len(), MAX_TABS - 1);
        tabs.new_tab(p("/31-again"));
        assert_eq!(tabs.len(), MAX_TABS);
        assert_eq!(
            tabs.restore_last_closed(),
            None,
            "restore must refuse at the cap, not bypass it"
        );
        tabs.close_active().expect("make room again");
        assert_eq!(
            tabs.restore_last_closed().map(|tab| tab.cwd),
            Some(p("/31-again")),
            "the earlier refusal must not have consumed the closed tab"
        );
        assert!(!tabs.can_open_tab(), "restoring back to the cap is allowed");
        assert_eq!(tabs.restore_last_closed(), None, "closed list is empty");
    }

    #[test]
    fn from_paths_restores_and_clamps_active() {
        let tabs = Tabs::from_paths(vec![p("/a"), p("/b"), p("/c")], 1).expect("tabs");
        assert_eq!(tabs.len(), 3);
        assert_eq!(tabs.active_index(), 1);
        assert_eq!(tabs.active().cwd, p("/b"));

        let tabs = Tabs::from_paths(vec![p("/a")], 99).expect("tabs");
        assert_eq!(tabs.active_index(), 0);

        assert!(Tabs::from_paths(Vec::new(), 0).is_none());
    }

    #[test]
    fn moving_tabs_reorders_and_keeps_active() {
        let mut tabs = Tabs::new(p("/a"));
        tabs.new_tab(p("/b"));
        tabs.new_tab(p("/c"));
        // active = /c (index 2)
        assert!(tabs.move_active_left());
        let order: Vec<_> = tabs.tabs().iter().map(|tab| tab.cwd.clone()).collect();
        assert_eq!(order, vec![p("/a"), p("/c"), p("/b")]);
        assert_eq!(tabs.active_index(), 1);
        assert_eq!(tabs.active().cwd, p("/c"));

        assert!(tabs.move_active_right());
        let order: Vec<_> = tabs.tabs().iter().map(|tab| tab.cwd.clone()).collect();
        assert_eq!(order, vec![p("/a"), p("/b"), p("/c")]);
        assert_eq!(tabs.active_index(), 2);
        assert!(!tabs.move_active_right());
    }

    #[test]
    fn moving_a_single_tab_is_a_noop() {
        let mut tabs = Tabs::new(p("/a"));
        assert!(!tabs.move_active_left());
        assert!(!tabs.move_active_right());
        assert_eq!(tabs.active_index(), 0);
    }

    #[test]
    fn next_and_prev_wrap_around() {
        let mut tabs = Tabs::new(p("/a"));
        tabs.new_tab(p("/b"));
        tabs.new_tab(p("/c"));
        assert_eq!(tabs.next().cwd, p("/a"));
        assert_eq!(tabs.next().cwd, p("/b"));
        assert_eq!(tabs.prev().cwd, p("/a"));
        assert_eq!(tabs.prev().cwd, p("/c"));
    }
}
