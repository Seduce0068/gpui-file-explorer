// SPDX-License-Identifier: AGPL-3.0-or-later
//! Tab state for the explorer: each tab owns a directory and its navigation
//! history. Pure state (no GPUI) so switching/closing rules can be tested.

use std::path::PathBuf;

use crate::history::History;

/// One tab: a directory plus its back/forward history.
#[derive(Debug, Clone)]
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
}

impl Tabs {
    pub fn new(cwd: PathBuf) -> Self {
        Self {
            tabs: vec![Tab::new(cwd)],
            active: 0,
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
        Some(Self { tabs, active })
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

    /// Append a new tab and make it active. Returns its index.
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
        self.tabs.remove(self.active);
        if self.active >= self.tabs.len() {
            self.active = self.tabs.len() - 1;
        }
        Some(self.tabs[self.active].clone())
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
        tabs.sync(p("/c"), history);
        assert_eq!(tabs.tabs()[1].cwd, p("/c"));
        assert_eq!(tabs.tabs()[0].cwd, p("/a"));
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
