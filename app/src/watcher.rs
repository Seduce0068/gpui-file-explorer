// SPDX-License-Identifier: AGPL-3.0-or-later
//! Directory watching: `notify` + `notify-debouncer-full` on a background
//! thread, watching the current directory and its parent non-recursively.
//!
//! The debouncer callback runs on its own thread, so it may only signal a
//! channel; the UI entity is updated by a GPUI foreground task (`Explorer`).

use std::path::PathBuf;
use std::time::Duration;

use notify_debouncer_full::notify::event::{EventKind, ModifyKind, RenameMode};
use notify_debouncer_full::notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, RecommendedCache};
use tracing::error;

type FileEventWatcher = Debouncer<RecommendedWatcher, RecommendedCache>;

/// Normalized filesystem event, independent of `notify`'s event types.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FsEvent {
    Created(Vec<PathBuf>),
    Removed(Vec<PathBuf>),
    Modified(Vec<PathBuf>),
    Renamed {
        from: Option<PathBuf>,
        to: Option<PathBuf>,
    },
    Rescan,
}

/// Convert a debouncer result into UI-facing events. Errors and unknown kinds
/// become `Rescan` so the caller can safely fall back to a full reload.
pub fn normalize_events(result: &DebounceEventResult) -> Vec<FsEvent> {
    let Ok(events) = result else {
        return vec![FsEvent::Rescan];
    };
    let mut out = Vec::new();
    for debounced in events {
        let event = &debounced.event;
        let paths: Vec<PathBuf> = event
            .paths
            .iter()
            .filter(|path| !path.as_os_str().is_empty())
            .cloned()
            .collect();
        match event.kind {
            EventKind::Create(_) if !paths.is_empty() => out.push(FsEvent::Created(paths)),
            EventKind::Remove(_) if !paths.is_empty() => out.push(FsEvent::Removed(paths)),
            EventKind::Modify(ModifyKind::Name(RenameMode::Both)) if paths.len() >= 2 => {
                out.push(FsEvent::Renamed {
                    from: Some(paths[0].clone()),
                    to: Some(paths[1].clone()),
                });
            }
            EventKind::Modify(ModifyKind::Name(RenameMode::From)) if !paths.is_empty() => {
                out.push(FsEvent::Renamed {
                    from: paths.first().cloned(),
                    to: None,
                });
            }
            EventKind::Modify(ModifyKind::Name(RenameMode::To)) if !paths.is_empty() => {
                out.push(FsEvent::Renamed {
                    from: None,
                    to: paths.first().cloned(),
                });
            }
            EventKind::Modify(ModifyKind::Name(_)) => out.push(FsEvent::Rescan),
            EventKind::Modify(_) if !paths.is_empty() => out.push(FsEvent::Modified(paths)),
            EventKind::Access(_) => {}
            _ => out.push(FsEvent::Rescan),
        }
    }
    out
}

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
    /// thread) with normalized events after every debounced batch.
    pub fn new(on_change: impl Fn(Vec<FsEvent>) + Send + 'static) -> Self {
        match new_debouncer(DEBOUNCE, None, move |result: DebounceEventResult| {
            on_change(normalize_events(&result));
        }) {
            Ok(debouncer) => Self {
                debouncer: Some(debouncer),
                watched: Vec::new(),
                last_error: None,
            },
            Err(error) => {
                // Preserve the underlying cause (ENOSPC / EMFILE / permission)
                // instead of a fixed string so the status bar is actionable
                // (second cross-review H-5).
                let reason = format!("ファイル監視を初期化できませんでした: {error}");
                error!(%reason, "watcher initialization failed");
                Self {
                    debouncer: None,
                    watched: Vec::new(),
                    last_error: Some(reason),
                }
            }
        }
    }

    pub fn is_supported(&self) -> bool {
        self.debouncer.is_some()
    }

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
        let mut errors = Vec::new();
        for old in std::mem::take(&mut self.watched) {
            if let Err(err) = debouncer.unwatch(&old) {
                // A stale watch may still fire; report it instead of silently
                // dropping the failure (second cross-review Low finding).
                errors.push(format!("{} の監視解除に失敗: {err}", old.display()));
            }
        }
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
    use super::{normalize_events, DirWatcher, FsEvent};
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

        let (tx, rx) = mpsc::channel::<Vec<FsEvent>>();
        let mut watcher = DirWatcher::new(move |events| {
            let _ = tx.send(events);
        });
        assert!(
            watcher.is_supported(),
            "inotify watcher should initialize on Linux"
        );
        watcher.watch(std::slice::from_ref(&dir));
        assert_eq!(watcher.watched_paths(), std::slice::from_ref(&dir));

        fs::write(dir.join("hello.txt"), b"hi").unwrap();
        let events = rx
            .recv_timeout(Duration::from_secs(5))
            .expect("watcher should report the new file");
        assert!(
            events
                .iter()
                .any(|event| matches!(event, FsEvent::Created(_))),
            "creation should be normalized to FsEvent::Created: {events:?}"
        );

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn watcher_errors_become_rescan() {
        assert_eq!(normalize_events(&Err(vec![])), vec![FsEvent::Rescan]);
    }

    #[test]
    fn normalizes_rename_events() {
        use notify_debouncer_full::notify::event::{Event, EventKind, ModifyKind, RenameMode};
        use notify_debouncer_full::DebouncedEvent;
        use std::time::Instant;

        let from = PathBuf::from("/tmp/from.txt");
        let to = PathBuf::from("/tmp/to.txt");
        let debounced = |kind: EventKind, paths: Vec<PathBuf>| DebouncedEvent {
            event: Event {
                kind,
                paths,
                attrs: Default::default(),
            },
            time: Instant::now(),
        };
        let rename = |mode| EventKind::Modify(ModifyKind::Name(mode));

        assert_eq!(
            normalize_events(&Ok(vec![debounced(
                rename(RenameMode::Both),
                vec![from.clone(), to.clone()]
            )])),
            vec![FsEvent::Renamed {
                from: Some(from.clone()),
                to: Some(to.clone()),
            }]
        );
        assert_eq!(
            normalize_events(&Ok(vec![debounced(
                rename(RenameMode::From),
                vec![from.clone()]
            )])),
            vec![FsEvent::Renamed {
                from: Some(from),
                to: None,
            }]
        );
        assert_eq!(
            normalize_events(&Ok(vec![debounced(
                rename(RenameMode::To),
                vec![to.clone()]
            )])),
            vec![FsEvent::Renamed {
                from: None,
                to: Some(to),
            }]
        );
    }

    #[test]
    fn normalizes_remove_modify_and_access_events() {
        use notify_debouncer_full::notify::event::{
            AccessKind, DataChange, Event, EventKind, MetadataKind, ModifyKind, RemoveKind,
        };
        use notify_debouncer_full::DebouncedEvent;
        use std::time::Instant;

        let path = PathBuf::from("/tmp/changed.txt");
        let debounced = |kind| DebouncedEvent {
            event: Event {
                kind,
                paths: vec![path.clone()],
                attrs: Default::default(),
            },
            time: Instant::now(),
        };

        assert_eq!(
            normalize_events(&Ok(vec![debounced(EventKind::Remove(RemoveKind::Any))])),
            vec![FsEvent::Removed(vec![path.clone()])]
        );
        assert_eq!(
            normalize_events(&Ok(vec![debounced(EventKind::Modify(ModifyKind::Data(
                DataChange::Any
            )))])),
            vec![FsEvent::Modified(vec![path.clone()])]
        );
        assert_eq!(
            normalize_events(&Ok(vec![debounced(EventKind::Modify(
                ModifyKind::Metadata(MetadataKind::Any)
            ))])),
            vec![FsEvent::Modified(vec![path.clone()])]
        );
        assert!(
            normalize_events(&Ok(vec![debounced(EventKind::Access(AccessKind::Any))])).is_empty(),
            "access events must be ignored"
        );
        assert_eq!(
            normalize_events(&Ok(vec![debounced(EventKind::Modify(ModifyKind::Name(
                notify_debouncer_full::notify::event::RenameMode::Any
            )))])),
            vec![FsEvent::Rescan]
        );
        assert_eq!(
            normalize_events(&Ok(vec![debounced(EventKind::Any)])),
            vec![FsEvent::Rescan]
        );
    }

    #[test]
    fn rewatching_same_set_is_a_noop() {
        let dir = scratch("same");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let mut watcher = DirWatcher::new(|_| {});
        watcher.watch(std::slice::from_ref(&dir));
        watcher.watch(std::slice::from_ref(&dir));
        assert_eq!(watcher.watched_paths(), std::slice::from_ref(&dir));
        fs::remove_dir_all(&dir).unwrap();
    }
}
