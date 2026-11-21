// SPDX-License-Identifier: AGPL-3.0-or-later
//! Directory watching: `notify` + `notify-debouncer-full` on a background
//! thread, watching the current directory and its parent non-recursively.
//!
//! The debouncer callback runs on its own thread, so it may only signal a
//! channel; the UI entity is updated by a GPUI foreground task (`Explorer`).

use std::path::{Path, PathBuf};
use std::time::Duration;

use notify_debouncer_full::notify::event::{EventKind, ModifyKind, RenameMode};
use notify_debouncer_full::notify::{
    Config, Error, EventHandler, RecommendedWatcher, RecursiveMode, Result as NotifyResult,
    Watcher, WatcherKind,
};
use notify_debouncer_full::{new_debouncer_opt, DebounceEventResult, Debouncer, RecommendedCache};
use tracing::error;

type FileEventWatcher = Debouncer<SafeWatcher, RecommendedCache>;

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

/// Collect every path reported as removed in a batch of events. Used by the
/// search view to drop vanished results without a full re-search.
pub fn removed_paths(events: &[FsEvent]) -> Vec<PathBuf> {
    let mut removed = Vec::new();
    for event in events {
        if let FsEvent::Removed(paths) = event {
            removed.extend(paths.iter().cloned());
        }
    }
    removed
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

/// Fail-soft adapter around the platform watcher.
///
/// `new_debouncer_opt` spawns its worker thread *before* calling `T::new(...)?`.
/// When the OS watcher fails to initialize (ENOSPC / EMFILE / permission), that
/// `?` returns early and the orphaned thread loops forever on a stop flag no
/// surviving guard can set (upstream notify-debouncer-full 0.7.0 bug). Returning
/// `Ok` even on failure keeps the debouncer guard alive, so the existing `Drop`
/// stops the thread normally.
struct SafeWatcher {
    inner: Option<RecommendedWatcher>,
    init_error: Option<String>,
}

impl SafeWatcher {
    /// Rebuild a `notify::Error` from the stored cause. `notify::Error` is not
    /// `Clone`, so the failure message is kept as a `String`.
    fn init_failure(&self) -> Error {
        Error::generic(
            self.init_error
                .as_deref()
                .unwrap_or("OS watcher initialization failed"),
        )
    }
}

impl Watcher for SafeWatcher {
    fn new<F: EventHandler>(handler: F, config: Config) -> NotifyResult<Self> {
        match RecommendedWatcher::new(handler, config) {
            Ok(inner) => Ok(Self {
                inner: Some(inner),
                init_error: None,
            }),
            Err(error) => {
                let reason = error.to_string();
                error!(%reason, "OS watcher initialization failed inside debouncer");
                // Never propagate the error: the debouncer thread already exists
                // at this point, and returning `Err` would leak it forever.
                Ok(Self {
                    inner: None,
                    init_error: Some(reason),
                })
            }
        }
    }

    fn watch(&mut self, path: &Path, recursive_mode: RecursiveMode) -> NotifyResult<()> {
        match &mut self.inner {
            Some(inner) => inner.watch(path, recursive_mode),
            None => Err(self.init_failure()),
        }
    }

    fn unwatch(&mut self, path: &Path) -> NotifyResult<()> {
        match &mut self.inner {
            Some(inner) => inner.unwatch(path),
            None => Err(self.init_failure()),
        }
    }

    fn configure(&mut self, option: Config) -> NotifyResult<bool> {
        match &mut self.inner {
            Some(inner) => inner.configure(option),
            None => Err(self.init_failure()),
        }
    }

    fn kind() -> WatcherKind {
        RecommendedWatcher::kind()
    }
}

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
        // The debouncer spawns its worker thread before constructing the OS
        // watcher. Fail here, while no thread exists yet, for the common case
        // of an unavailable watcher; `SafeWatcher` covers the TOCTOU remainder.
        if let Err(error) = RecommendedWatcher::new(|_| {}, Config::default()) {
            let reason = format!("ファイル監視を初期化できませんでした: {error}");
            error!(%reason, "watcher initialization failed");
            return Self {
                debouncer: None,
                watched: Vec::new(),
                last_error: Some(reason),
            };
        }

        match new_debouncer_opt::<_, SafeWatcher, _>(
            DEBOUNCE,
            None,
            move |result: DebounceEventResult| {
                on_change(normalize_events(&result));
            },
            RecommendedCache::new(),
            Config::default(),
        ) {
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
    fn safe_watcher_without_inner_reports_init_failure() {
        use notify_debouncer_full::notify::{RecursiveMode, Watcher};

        use super::SafeWatcher;

        let dir = scratch("safe-watcher");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        // Simulate the debouncer having kept its thread alive while the real OS
        // watcher failed, without needing to force an actual kernel failure.
        let mut watcher = SafeWatcher {
            inner: None,
            init_error: Some("injected init failure".to_string()),
        };

        let err = watcher
            .watch(&dir, RecursiveMode::NonRecursive)
            .expect_err("fail-soft watcher must report the stored init error");
        assert!(
            err.to_string().contains("injected init failure"),
            "stored cause should be preserved: {err}"
        );
        assert!(watcher.unwatch(&dir).is_err());
        assert!(watcher.configure(Default::default()).is_err());

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn safe_watcher_without_init_error_uses_default_message() {
        use notify_debouncer_full::notify::{RecursiveMode, Watcher};

        use super::SafeWatcher;

        let dir = scratch("safe-watcher-default");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        // `new` always stores a cause on failure, so a missing reason is only
        // reachable by direct construction; `init_failure` must still fall back
        // to a meaningful message on every delegating method.
        let mut watcher = SafeWatcher {
            inner: None,
            init_error: None,
        };

        let err = watcher
            .watch(&dir, RecursiveMode::NonRecursive)
            .expect_err("watch must fail without an inner watcher");
        assert!(
            err.to_string().contains("OS watcher initialization failed"),
            "default init failure message missing: {err}"
        );

        let err = watcher
            .unwatch(&dir)
            .expect_err("unwatch must fail without an inner watcher");
        assert!(
            err.to_string().contains("OS watcher initialization failed"),
            "default init failure message missing: {err}"
        );

        let err = watcher
            .configure(Default::default())
            .expect_err("configure must fail without an inner watcher");
        assert!(
            err.to_string().contains("OS watcher initialization failed"),
            "default init failure message missing: {err}"
        );

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn rename_both_with_single_path_falls_back_to_rescan() {
        use notify_debouncer_full::notify::event::{Event, EventKind, ModifyKind, RenameMode};
        use notify_debouncer_full::DebouncedEvent;
        use std::time::Instant;

        // `RenameMode::Both` needs two paths (from + to). With only one usable
        // path no rename pair can be formed, so the safe full-reload signal wins.
        let debounced = DebouncedEvent {
            event: Event {
                kind: EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
                paths: vec![PathBuf::from("/tmp/only-one.txt")],
                attrs: Default::default(),
            },
            time: Instant::now(),
        };
        assert_eq!(
            normalize_events(&Ok(vec![debounced])),
            vec![FsEvent::Rescan]
        );
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
    fn empty_paths_after_filtering_fall_back_to_rescan() {
        use notify_debouncer_full::notify::event::{
            CreateKind, DataChange, Event, EventKind, ModifyKind, RemoveKind, RenameMode,
        };
        use notify_debouncer_full::DebouncedEvent;
        use std::time::Instant;

        let debounced = |kind: EventKind, paths: Vec<PathBuf>| DebouncedEvent {
            event: Event {
                kind,
                paths,
                attrs: Default::default(),
            },
            time: Instant::now(),
        };

        // Both a genuinely empty path list and a list holding only an empty
        // PathBuf are filtered out before the kind is matched (L10).
        for paths in [Vec::new(), vec![PathBuf::new()]] {
            let cases = [
                ("create", EventKind::Create(CreateKind::Any)),
                ("remove", EventKind::Remove(RemoveKind::Any)),
                (
                    "rename-both",
                    EventKind::Modify(ModifyKind::Name(RenameMode::Both)),
                ),
                (
                    "modify-data",
                    EventKind::Modify(ModifyKind::Data(DataChange::Any)),
                ),
            ];
            for (label, kind) in cases {
                assert_eq!(
                    normalize_events(&Ok(vec![debounced(kind, paths.clone())])),
                    vec![FsEvent::Rescan],
                    "{label} with no usable paths must normalize to Rescan"
                );
            }
        }
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

    #[test]
    fn multiple_old_unwatch_failures_are_joined_and_cleared_by_rewatch() {
        // Test seam without touching production code: this child module can
        // reach `DirWatcher`'s private `debouncer`. Dropping the OS-level
        // watches behind the manager's back leaves `watched` stale on purpose,
        // so the next `watch()` drives the real `debouncer.unwatch()` failure
        // branch (inotify returns `WatchNotFound`) for BOTH old paths. No
        // test-only constructor or injected closure is required.
        let dir = scratch("unwatch-failures");
        let old_a = dir.join("a");
        let old_b = dir.join("b");
        let next = dir.join("next");
        let after = dir.join("after");
        for path in [&old_a, &old_b, &next, &after] {
            fs::create_dir_all(path).unwrap();
        }

        let mut watcher = DirWatcher::new(|_| {});
        if !watcher.is_supported() {
            // No OS watcher on this host: there is no underlying watch to
            // invalidate, so the failure-aggregation scenario cannot be set up.
            let _ = fs::remove_dir_all(&dir);
            return;
        }
        watcher.watch(&[old_a.clone(), old_b.clone()]);
        assert!(watcher.last_error().is_none(), "seed watches must succeed");
        assert_eq!(watcher.watched_paths(), &[old_a.clone(), old_b.clone()]);

        {
            let debouncer = watcher.debouncer.as_mut().expect("supported watcher");
            debouncer.unwatch(&old_a).expect("seed watch a exists");
            debouncer.unwatch(&old_b).expect("seed watch b exists");
        }

        watcher.watch(std::slice::from_ref(&next));
        let error = watcher
            .last_error()
            .expect("stale watches must surface unwatch failures");
        assert!(
            error.contains(&old_a.display().to_string()),
            "first unwatch failure missing: {error}"
        );
        assert!(
            error.contains(&old_b.display().to_string()),
            "second unwatch failure missing: {error}"
        );
        assert!(
            error.contains("の監視解除に失敗"),
            "unwatch failures must be labelled: {error}"
        );
        assert!(
            error.contains(", "),
            "multiple failures must be joined: {error}"
        );
        assert_eq!(watcher.watched_paths(), std::slice::from_ref(&next));

        // The stale paths are no longer in `watched`, so a later rewatch only
        // performs healthy unwatch/watch calls and must clear `last_error`.
        watcher.watch(std::slice::from_ref(&after));
        assert!(
            watcher.last_error().is_none(),
            "successful rewatch must clear the aggregated error: {:?}",
            watcher.last_error()
        );
        assert_eq!(watcher.watched_paths(), std::slice::from_ref(&after));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn removed_paths_collects_only_removals() {
        use super::removed_paths;
        let events = vec![
            FsEvent::Created(vec![PathBuf::from("/a")]),
            FsEvent::Removed(vec![PathBuf::from("/b"), PathBuf::from("/c")]),
            FsEvent::Modified(vec![PathBuf::from("/d")]),
            FsEvent::Removed(vec![PathBuf::from("/e")]),
            FsEvent::Rescan,
        ];
        assert_eq!(
            removed_paths(&events),
            vec![
                PathBuf::from("/b"),
                PathBuf::from("/c"),
                PathBuf::from("/e")
            ]
        );
        assert!(removed_paths(&[]).is_empty());
    }
}
