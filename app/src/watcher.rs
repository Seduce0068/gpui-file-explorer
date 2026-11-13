// SPDX-License-Identifier: AGPL-3.0-or-later
//! Directory watching: `notify` + `notify-debouncer-full` on a background
//! thread, watching the current directory and its parent non-recursively.
//!
//! The debouncer callback runs on its own thread, so it may only signal a
//! channel; the UI entity is updated by a GPUI foreground task (`Explorer`).

use std::path::PathBuf;
use std::time::Duration;

use notify_debouncer_full::notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, RecommendedCache};

type FileEventWatcher = Debouncer<RecommendedWatcher, RecommendedCache>;

/// Debounce window for change events.
pub const DEBOUNCE: Duration = Duration::from_millis(400);

/// Owns the OS watcher for the directories currently displayed.
pub struct DirWatcher {
    debouncer: Option<FileEventWatcher>,
    watched: Vec<PathBuf>,
    last_error: Option<String>,
}

impl DirWatcher {
    /// Create the debounced watcher. `on_change` is called (from the watcher
    /// thread) after every debounced batch of filesystem events.
    pub fn new(on_change: impl Fn() + Send + 'static) -> Self {
        let debouncer = new_debouncer(DEBOUNCE, None, move |result: DebounceEventResult| {
            // We do not need the individual paths: any event in the current
            // directory (or its parent) triggers a refresh of the listing.
            let _ = result;
            on_change();
        })
        .ok();
        let last_error = if debouncer.is_some() {
            None
        } else {
            Some("ファイル監視を初期化できませんでした".to_string())
        };
        Self {
            debouncer,
            watched: Vec::new(),
            last_error,
        }
    }

    pub fn is_supported(&self) -> bool {
        self.debouncer.is_some()
    }

    #[allow(dead_code)]
    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    /// Watched root paths (for the status bar / debugging).
    pub fn watched_paths(&self) -> &[PathBuf] {
        &self.watched
    }

    /// Replace the watched set. Non-recursive watches of the listed
    /// directories; unwatchable paths are skipped (permissions, vanished dirs).
    pub fn watch(&mut self, dirs: &[PathBuf]) {
        // Retry when a previous attempt failed (permission denied, directory
        // temporarily missing, ...): a plain "already watched" check would make
        // that state sticky for the lifetime of the app.
        if self.watched == dirs && self.last_error.is_none() {
            return;
        }
        let Some(debouncer) = &mut self.debouncer else {
            return;
        };
        for old in std::mem::take(&mut self.watched) {
            let _ = debouncer.unwatch(&old);
        }
        let mut errors = Vec::new();
        for dir in dirs {
            match debouncer.watch(dir, RecursiveMode::NonRecursive) {
                Ok(()) => self.watched.push(dir.clone()),
                Err(err) => errors.push(format!("{}: {err}", dir.display())),
            }
        }
        self.last_error = if errors.is_empty() {
            None
        } else {
            Some(errors.join(", "))
        };
    }
}

impl Drop for DirWatcher {
    fn drop(&mut self) {
        // Dropping the debouncer sets its stop flag (notify-debouncer-full
        // `impl Drop`) and the watcher thread exits on its own; `stop(self)`
        // cannot be used from `&mut self` here without consuming the field.
    }
}

#[cfg(test)]
mod tests {
    // Explicit imports only: `use gpui_kit::*` in the crate root would shadow
    // the built-in `#[test]` attribute.
    use super::DirWatcher;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::mpsc;
    use std::time::Duration;

    fn scratch(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("gpui-fe-watch-{tag}-{}", std::process::id()))
    }

    #[test]
    fn watcher_reports_file_creation() {
        let dir = scratch("create");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let (tx, rx) = mpsc::channel::<()>();
        let mut watcher = DirWatcher::new(move || {
            let _ = tx.send(());
        });
        assert!(
            watcher.is_supported(),
            "inotify watcher should initialize on Linux"
        );
        watcher.watch(std::slice::from_ref(&dir));
        assert_eq!(watcher.watched_paths(), std::slice::from_ref(&dir));

        fs::write(dir.join("hello.txt"), b"hi").unwrap();
        rx.recv_timeout(Duration::from_secs(5))
            .expect("watcher should report the new file");

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn rewatching_same_set_is_a_noop() {
        let dir = scratch("same");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let mut watcher = DirWatcher::new(|| {});
        watcher.watch(std::slice::from_ref(&dir));
        watcher.watch(std::slice::from_ref(&dir));
        assert_eq!(watcher.watched_paths(), std::slice::from_ref(&dir));
        fs::remove_dir_all(&dir).unwrap();
    }
}
