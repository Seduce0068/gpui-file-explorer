// SPDX-License-Identifier: AGPL-3.0-or-later
//! Safe filesystem operations used by the file explorer.
//!
//! Safety rules (enforced here, not only in the UI):
//!
//! - **Never overwrite an existing destination.** Callers choose
//!   [`ConflictPolicy::Fail`], [`ConflictPolicy::Skip`] or
//!   [`ConflictPolicy::Rename`]; there is deliberately no "overwrite" policy.
//! - **Never follow symlinks when deleting.** [`remove_entry_safe`] inspects
//!   `symlink_metadata` first, so a symlink to a directory is removed as a
//!   link, not recursively traversed.
//! - **Copy files atomically.** Data is written to a `.partial-*` sibling and
//!   linked/renamed into place only after a complete, flushed copy; partial
//!   files are removed on error or cancellation.
//! - **Moves keep the source on failure.** Cross-device moves copy first and
//!   delete the source only after the copy finished successfully.
//! - **Cancellation leaves no half-written destination.** The caller passes a
//!   [`CancelFlag`]; it is checked between entries and between 64 KiB chunks.
//!
//! This module is intentionally GPUI-free so it can be unit tested headlessly.
//! It is exercised by the tests below and backs the cut/copy/paste UI.

use std::fs;
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// Size of the chunks used when copying file contents.
pub const COPY_CHUNK_SIZE: usize = 64 * 1024;

/// Cooperative cancellation flag shared with the background operation.
#[derive(Debug, Default)]
pub struct CancelFlag(AtomicBool);

impl CancelFlag {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// What to do when the destination name already exists.
///
/// The UI always uses `Rename`; `Fail` and `Skip` are kept as part of the
/// engine API and exercised by the ops unit tests.
#[cfg_attr(not(test), expect(dead_code, reason = "engine API; used in ops tests"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConflictPolicy {
    /// Abort with [`OpError::AlreadyExists`] (safest default).
    Fail,
    /// Leave the existing entry untouched and report it in [`OpStats::skipped`].
    Skip,
    /// Pick a unique name (`name (1).ext`, ...) and copy alongside.
    Rename,
}

/// A failure of a filesystem operation. Messages are user-displayable.
#[derive(Debug)]
pub enum OpError {
    NotFound(PathBuf),
    PermissionDenied(PathBuf),
    AlreadyExists(PathBuf),
    InvalidTarget(String),
    Unsupported(String),
    Cancelled,
    Io { path: PathBuf, message: String },
}

impl std::fmt::Display for OpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(path) => write!(f, "見つかりません: {}", path.display()),
            Self::PermissionDenied(path) => {
                write!(f, "アクセスが拒否されました: {}", path.display())
            }
            Self::AlreadyExists(path) => write!(f, "既に存在します: {}", path.display()),
            Self::InvalidTarget(reason) => write!(f, "操作できません: {reason}"),
            Self::Unsupported(reason) => write!(f, "未対応の操作です: {reason}"),
            Self::Cancelled => write!(f, "操作はキャンセルされました"),
            Self::Io { path, message } => write!(f, "{}: {message}", path.display()),
        }
    }
}

impl std::error::Error for OpError {}

/// Counters and lists describing a completed (or partially completed) operation.
#[derive(Debug, Default, Clone)]
pub struct OpStats {
    /// Number of regular files copied/moved.
    pub files: u64,
    /// Number of directories created.
    pub dirs: u64,
    /// Total bytes copied.
    pub bytes: u64,
    /// Entries skipped because they already existed (policy `Skip`).
    pub skipped: Vec<PathBuf>,
    /// Destination paths that were created (top-level first).
    pub created: Vec<PathBuf>,
}

fn io_error(path: &Path, err: std::io::Error) -> OpError {
    match err.kind() {
        std::io::ErrorKind::NotFound => OpError::NotFound(path.to_path_buf()),
        std::io::ErrorKind::PermissionDenied => OpError::PermissionDenied(path.to_path_buf()),
        std::io::ErrorKind::AlreadyExists => OpError::AlreadyExists(path.to_path_buf()),
        // Cross-device moves are handled by the fallback in `move_entry`; if
        // one reaches here it is a genuine unsupported operation.
        std::io::ErrorKind::CrossesDevices => OpError::Unsupported(format!(
            "異なるデバイス間の操作に失敗しました: {}",
            path.display()
        )),
        std::io::ErrorKind::IsADirectory => OpError::InvalidTarget(format!(
            "ディレクトリに対してファイル操作はできません: {}",
            path.display()
        )),
        _ => OpError::Io {
            path: path.to_path_buf(),
            message: err.to_string(),
        },
    }
}

/// Validate a single file name (not a path): no separators, no `.`/`..`,
/// no NUL byte, and no characters that Windows reserves.
pub fn validate_file_name(name: &str) -> Result<(), OpError> {
    if name.is_empty() {
        return Err(OpError::InvalidTarget("名前が空です".to_string()));
    }
    if name == "." || name == ".." {
        return Err(OpError::InvalidTarget(
            "`.` と `..` は名前として使えません".to_string(),
        ));
    }
    if name.contains('/') || name.contains('\\') {
        return Err(OpError::InvalidTarget(
            "名前にパス区切り文字は使えません".to_string(),
        ));
    }
    if name.contains('\0') {
        return Err(OpError::InvalidTarget(
            "名前に NUL 文字は使えません".to_string(),
        ));
    }
    #[cfg(windows)]
    {
        const RESERVED: [&str; 22] = [
            "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
            "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
        ];
        let stem = name.split('.').next().unwrap_or(name).to_ascii_uppercase();
        if RESERVED.contains(&stem.as_str()) {
            return Err(OpError::InvalidTarget(format!(
                "Windows の予約名は使えません: {name}"
            )));
        }
        if name.contains(['<', '>', ':', '"', '|', '?', '*']) {
            return Err(OpError::InvalidTarget(
                "Windows で使えない文字が含まれています".to_string(),
            ));
        }
    }
    Ok(())
}

/// Return `dir/name` if free, otherwise the first free `name (N).ext`.
///
/// Builds candidate names as raw `OsString`s so non-UTF-8 file names survive
/// the rename-conflict path without lossy conversion.
pub fn unique_path(dir: &Path, name: &std::ffi::OsStr) -> PathBuf {
    let candidate = dir.join(name);
    if !candidate.exists() {
        return candidate;
    }
    let path = Path::new(name);
    let stem = path.file_stem().unwrap_or(name);
    let extension = path.extension();

    let append_counter = |stem: &std::ffi::OsStr, counter: usize| -> std::ffi::OsString {
        #[cfg(unix)]
        {
            use std::os::unix::ffi::{OsStrExt, OsStringExt};
            let mut bytes = stem.as_bytes().to_vec();
            bytes.extend_from_slice(format!(" ({counter})").as_bytes());
            std::ffi::OsString::from_vec(bytes)
        }
        #[cfg(not(unix))]
        {
            let mut text = stem.to_string_lossy().into_owned();
            text.push_str(&format!(" ({counter})"));
            std::ffi::OsString::from(text)
        }
    };

    for counter in 1..=10_000 {
        let mut candidate_name = append_counter(stem, counter);
        if let Some(extension) = extension {
            candidate_name.push(".");
            candidate_name.push(extension);
        }
        let candidate = dir.join(candidate_name);
        if !candidate.exists() {
            return candidate;
        }
    }
    // Practically unreachable; fall back to the original path so the caller
    // fails with AlreadyExists instead of looping forever.
    candidate
}

/// Resolve the destination path for `src` inside `dest_dir` according to
/// `policy`, without touching the filesystem beyond existence checks.
fn resolve_destination(
    src: &Path,
    dest_dir: &Path,
    policy: ConflictPolicy,
) -> Result<Option<PathBuf>, OpError> {
    let name = src
        .file_name()
        .ok_or_else(|| OpError::InvalidTarget("移動元の名前を取得できません".to_string()))?;
    let candidate = dest_dir.join(name);
    if !candidate.exists() {
        return Ok(Some(candidate));
    }
    match policy {
        ConflictPolicy::Fail => Err(OpError::AlreadyExists(candidate)),
        ConflictPolicy::Skip => Ok(None),
        ConflictPolicy::Rename => Ok(Some(unique_path(dest_dir, name))),
    }
}

/// Reject copying/moving a directory into itself or one of its descendants.
fn ensure_not_into_itself(src: &Path, dest_dir: &Path) -> Result<(), OpError> {
    if !src.is_dir() {
        return Ok(());
    }
    let canonical_src = fs::canonicalize(src).map_err(|e| io_error(src, e))?;
    let canonical_dest = fs::canonicalize(dest_dir).map_err(|e| io_error(dest_dir, e))?;
    if canonical_dest == canonical_src || canonical_dest.starts_with(&canonical_src) {
        return Err(OpError::InvalidTarget(
            "ディレクトリを自身または配下へコピー/移動できません".to_string(),
        ));
    }
    Ok(())
}

/// Copy a single file atomically: write a sibling `.partial-*` file, flush,
/// then link it into place. `dest` must not exist.
fn copy_file_atomic(
    src: &Path,
    dest: &Path,
    cancel: &CancelFlag,
    stats: &mut OpStats,
    progress: &mut dyn FnMut(u64),
) -> Result<(), OpError> {
    #[cfg(unix)]
    let metadata = fs::metadata(src).map_err(|e| io_error(src, e))?;
    let parent = dest
        .parent()
        .ok_or_else(|| OpError::InvalidTarget("コピー先の親がありません".to_string()))?;
    let file_name = dest
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "copy".to_string());
    // O_EXCL reservation with an unpredictable name: `File::create` on a
    // predictable `.partial-*` path follows a pre-planted symlink and can
    // truncate an unrelated file (CWE-59). nanos + PID + file counter keep
    // concurrent copies distinct without colliding with stale temp files.
    let partial = parent.join(format!(
        ".{file_name}.partial-{}-{}-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0),
        std::process::id(),
        stats.files
    ));

    // Report progress relative to the bytes copied by previous files, so the
    // callback receives a monotonically increasing byte count for the whole
    // operation.
    let base_bytes = stats.bytes;
    let result = (|| -> Result<u64, OpError> {
        let mut input = fs::File::open(src).map_err(|e| io_error(src, e))?;
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&partial)
            .map_err(|e| io_error(&partial, e))?;
        let mut buffer = vec![0u8; COPY_CHUNK_SIZE];
        let mut total = 0u64;
        loop {
            if cancel.is_cancelled() {
                return Err(OpError::Cancelled);
            }
            let read = input.read(&mut buffer).map_err(|e| io_error(src, e))?;
            if read == 0 {
                break;
            }
            output
                .write_all(&buffer[..read])
                .map_err(|e| io_error(&partial, e))?;
            total += read as u64;
            progress(base_bytes + total);
        }
        output.flush().map_err(|e| io_error(&partial, e))?;
        // Best-effort durability; a failure here is not fatal for a UI copy.
        let _ = output.sync_all();
        Ok(total)
    })();

    let bytes = match result {
        Ok(bytes) => bytes,
        Err(err) => {
            let _ = fs::remove_file(&partial);
            return Err(err);
        }
    };

    // Reserve `dest` with O_EXCL semantics. A concurrent creator cannot slip
    // in between this check and the final rename, so the "never overwrite"
    // rule holds even under a race. `rename` then atomically replaces only our
    // own empty reservation with the fully copied partial file.
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dest)
    {
        Ok(reservation) => {
            drop(reservation);
            // On Windows `fs::rename` refuses to replace an existing file, so
            // the reservation must be removed first. The O_EXCL check above
            // still prevents clobbering a pre-existing destination; Unix keeps
            // the reservation in place for its atomic replace semantics.
            #[cfg(windows)]
            let _ = fs::remove_file(dest);
            if let Err(error) = fs::rename(&partial, dest) {
                // `dest` is our own reservation at this point; removing it is
                // safe and leaves no half-written destination behind.
                let _ = fs::remove_file(dest);
                let _ = fs::remove_file(&partial);
                return Err(io_error(dest, error));
            }
        }
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {
            let _ = fs::remove_file(&partial);
            return Err(OpError::AlreadyExists(dest.to_path_buf()));
        }
        Err(error) => {
            let _ = fs::remove_file(&partial);
            return Err(io_error(dest, error));
        }
    }

    // Preserve the Unix permission bits (best effort).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = metadata.permissions().mode();
        let _ = fs::set_permissions(dest, fs::Permissions::from_mode(mode));
    }

    stats.files += 1;
    stats.bytes = base_bytes + bytes;
    Ok(())
}

/// Copy a symlink as a symlink (never follow it). On non-Unix platforms this
/// is unsupported for safety: following the link could copy unintended data.
fn copy_symlink(src: &Path, dest: &Path, stats: &mut OpStats) -> Result<(), OpError> {
    #[cfg(unix)]
    {
        let target = fs::read_link(src).map_err(|e| io_error(src, e))?;
        std::os::unix::fs::symlink(&target, dest).map_err(|e| io_error(dest, e))?;
        stats.files += 1;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        let _ = (src, dest, stats);
        Err(OpError::Unsupported(
            "シンボリックリンクのコピーは未対応です".to_string(),
        ))
    }
}

/// Recursively copy `src` to `dest` (which must not exist yet).
fn copy_recursive(
    src: &Path,
    dest: &Path,
    cancel: &CancelFlag,
    stats: &mut OpStats,
    progress: &mut dyn FnMut(u64),
) -> Result<(), OpError> {
    if cancel.is_cancelled() {
        return Err(OpError::Cancelled);
    }
    let metadata = fs::symlink_metadata(src).map_err(|e| io_error(src, e))?;

    if metadata.file_type().is_symlink() {
        return copy_symlink(src, dest, stats);
    }
    if metadata.is_dir() {
        fs::create_dir(dest).map_err(|e| io_error(dest, e))?;
        stats.dirs += 1;
        let mut entries: Vec<PathBuf> = Vec::new();
        let reader = fs::read_dir(src).map_err(|e| io_error(src, e))?;
        for entry in reader {
            let entry = entry.map_err(|e| io_error(src, e))?;
            entries.push(entry.path());
        }
        entries.sort();
        for child in entries {
            let name = child.file_name().ok_or_else(|| {
                OpError::InvalidTarget("子要素の名前を取得できません".to_string())
            })?;
            copy_recursive(&child, &dest.join(name), cancel, stats, progress)?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = metadata.permissions().mode();
            let _ = fs::set_permissions(dest, fs::Permissions::from_mode(mode));
        }
        return Ok(());
    }
    if metadata.is_file() {
        return copy_file_atomic(src, dest, cancel, stats, progress);
    }
    Err(OpError::Unsupported(format!(
        "特殊ファイルはコピーできません: {}",
        src.display()
    )))
}

/// Copy `src` into `dest_dir`, honouring `policy`. Returns counters for the
/// caller's progress/status UI. `on_progress` is called once per copied file
/// with the cumulative byte count.
pub fn copy_entry(
    src: &Path,
    dest_dir: &Path,
    policy: ConflictPolicy,
    cancel: &CancelFlag,
    mut on_progress: impl FnMut(u64),
) -> Result<OpStats, OpError> {
    let progress: &mut dyn FnMut(u64) = &mut on_progress;
    if !src.exists() && !src.is_symlink() {
        return Err(OpError::NotFound(src.to_path_buf()));
    }
    if !dest_dir.is_dir() {
        return Err(OpError::InvalidTarget(format!(
            "コピー先がディレクトリではありません: {}",
            dest_dir.display()
        )));
    }
    ensure_not_into_itself(src, dest_dir)?;

    let Some(dest) = resolve_destination(src, dest_dir, policy)? else {
        let mut stats = OpStats::default();
        stats
            .skipped
            .push(dest_dir.join(src.file_name().unwrap_or_default()));
        return Ok(stats);
    };

    let mut stats = OpStats::default();
    copy_recursive(src, &dest, cancel, &mut stats, progress)?;
    stats.created.insert(0, dest);
    progress(stats.bytes);
    Ok(stats)
}

/// Remove `path` without following symlinks. Directories are emptied
/// recursively first; a symlink is unlinked, never traversed.
pub fn remove_entry_safe(path: &Path) -> Result<(), OpError> {
    let metadata = fs::symlink_metadata(path).map_err(|e| io_error(path, e))?;
    if metadata.file_type().is_symlink() || metadata.is_file() {
        return fs::remove_file(path).map_err(|e| io_error(path, e));
    }
    if metadata.is_dir() {
        let reader = fs::read_dir(path).map_err(|e| io_error(path, e))?;
        for entry in reader {
            let entry = entry.map_err(|e| io_error(path, e))?;
            remove_entry_safe(&entry.path())?;
        }
        return fs::remove_dir(path).map_err(|e| io_error(path, e));
    }
    Err(OpError::Unsupported(format!(
        "特殊ファイルは削除できません: {}",
        path.display()
    )))
}

/// Best-effort total size of `paths`, used for progress display.
///
/// Symlinks are not followed (counted as 0), unreadable entries are skipped,
/// and directories are walked recursively. The function never fails: an
/// incomplete total is still useful for a progress bar.
pub fn total_bytes(paths: &[PathBuf]) -> u64 {
    paths.iter().map(|path| total_bytes_of(path)).sum()
}

/// Aggregate statistics for a directory tree (read-only).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DirStats {
    pub files: u64,
    pub dirs: u64,
    pub bytes: u64,
}

/// Walk `root` and count files/directories/bytes without following symlinks.
///
/// - symlinks are counted as one file with 0 bytes (never followed),
/// - unreadable directories/entries are skipped,
/// - `cancel` is checked per entry; a cancelled walk returns partial stats.
pub fn dir_stats(root: &Path, cancel: &CancelFlag) -> DirStats {
    let mut stats = DirStats::default();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if cancel.is_cancelled() {
            break;
        }
        let Ok(reader) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in reader.flatten() {
            if cancel.is_cancelled() {
                return stats;
            }
            let path = entry.path();
            let Ok(metadata) = fs::symlink_metadata(&path) else {
                continue;
            };
            if metadata.file_type().is_symlink() {
                stats.files += 1;
                continue;
            }
            if metadata.is_dir() {
                stats.dirs += 1;
                stack.push(path);
            } else if metadata.is_file() {
                stats.files += 1;
                stats.bytes += metadata.len();
            }
        }
    }
    stats
}

fn total_bytes_of(path: &Path) -> u64 {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return 0;
    };
    if metadata.file_type().is_symlink() {
        return 0;
    }
    if metadata.is_file() {
        return metadata.len();
    }
    if metadata.is_dir() {
        let Ok(reader) = fs::read_dir(path) else {
            return 0;
        };
        return reader
            .filter_map(|entry| entry.ok())
            .map(|entry| total_bytes_of(&entry.path()))
            .sum();
    }
    0
}

/// Move `src` into `dest_dir`. Uses `rename` when possible; on `EXDEV` falls
/// back to copy-then-delete, keeping the source intact if the copy fails.
pub fn move_entry(
    src: &Path,
    dest_dir: &Path,
    policy: ConflictPolicy,
    cancel: &CancelFlag,
    mut on_progress: impl FnMut(u64),
) -> Result<OpStats, OpError> {
    if !src.exists() && !src.is_symlink() {
        return Err(OpError::NotFound(src.to_path_buf()));
    }
    if !dest_dir.is_dir() {
        return Err(OpError::InvalidTarget(format!(
            "移動先がディレクトリではありません: {}",
            dest_dir.display()
        )));
    }
    ensure_not_into_itself(src, dest_dir)?;

    // Same-directory move is a no-op.
    if src.parent() == Some(dest_dir) {
        return Ok(OpStats::default());
    }

    let Some(dest) = resolve_destination(src, dest_dir, policy)? else {
        let mut stats = OpStats::default();
        stats
            .skipped
            .push(dest_dir.join(src.file_name().unwrap_or_default()));
        return Ok(stats);
    };

    // Reserve `dest` with O_EXCL before renaming so a concurrent creator
    // cannot slip in between the existence check and the rename (F6-02),
    // mirroring the copy path. On Windows the reservation must be removed
    // first because rename refuses to replace an existing file.
    let reservation = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&dest);
    match reservation {
        Ok(file) => {
            drop(file);
            #[cfg(windows)]
            let _ = fs::remove_file(&dest);
            match fs::rename(src, &dest) {
                Ok(()) => {
                    let mut stats = OpStats::default();
                    stats.created.push(dest);
                    on_progress(0);
                    Ok(stats)
                }
                Err(error) if error.kind() == ErrorKind::CrossesDevices => {
                    // Cross-device link: the reservation was removed above, so
                    // copy to the resolved destination, then delete the source
                    // only after a fully successful copy.
                    let _ = fs::remove_file(&dest);
                    let mut stats = OpStats::default();
                    let progress: &mut dyn FnMut(u64) = &mut on_progress;
                    if let Err(copy_err) = copy_recursive(src, &dest, cancel, &mut stats, progress)
                    {
                        let _ = remove_entry_safe(&dest);
                        return Err(copy_err);
                    }
                    on_progress(stats.bytes);
                    if cancel.is_cancelled() {
                        let _ = remove_entry_safe(&dest);
                        return Err(OpError::Cancelled);
                    }
                    if let Err(remove_err) = remove_entry_safe(src) {
                        return Err(OpError::Io {
                            path: src.to_path_buf(),
                            message: format!(
                                "コピーは完了しましたが元を削除できませんでした: {remove_err}"
                            ),
                        });
                    }
                    stats.created.insert(0, dest);
                    Ok(stats)
                }
                Err(error) => {
                    let _ = fs::remove_file(&dest);
                    Err(io_error(&dest, error))
                }
            }
        }
        Err(error) if error.kind() == ErrorKind::AlreadyExists => Err(OpError::AlreadyExists(dest)),
        Err(error) => Err(io_error(&dest, error)),
    }
}

/// Rename `src` to `new_name` in its current directory. Refuses to overwrite
/// an existing entry. Case-only renames go through a temporary name so they
/// work on case-insensitive filesystems too.
pub fn rename_entry(src: &Path, new_name: &str) -> Result<PathBuf, OpError> {
    validate_file_name(new_name)?;
    if !src.exists() {
        return Err(OpError::NotFound(src.to_path_buf()));
    }
    let parent = src
        .parent()
        .ok_or_else(|| OpError::InvalidTarget("親ディレクトリがありません".to_string()))?;
    let target = parent.join(new_name);
    if target == src {
        return Ok(target);
    }
    if target.exists() {
        // Case-only rename on a case-insensitive filesystem: the target is the
        // same file. Route through a temporary name.
        let same_file = fs::canonicalize(src)
            .ok()
            .zip(fs::canonicalize(&target).ok())
            .is_some_and(|(a, b)| a == b);
        if !same_file {
            return Err(OpError::AlreadyExists(target));
        }
        // Short, unpredictable intermediate name: embedding `new_name` can
        // exceed NAME_MAX, and a predictable path risks collision with a
        // pre-planted file on a case-insensitive filesystem.
        let temp = parent.join(format!(
            ".rename-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0)
        ));
        fs::rename(src, &temp).map_err(|e| io_error(src, e))?;
        fs::rename(&temp, &target).map_err(|e| io_error(&target, e))?;
        return Ok(target);
    }
    fs::rename(src, &target).map_err(|e| io_error(src, e))?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "gpui-fe-ops-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(path: &Path, contents: &[u8]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    #[test]
    fn validate_file_name_rejects_paths_and_dots() {
        assert!(validate_file_name("ok.txt").is_ok());
        assert!(validate_file_name("").is_err());
        assert!(validate_file_name(".").is_err());
        assert!(validate_file_name("..").is_err());
        assert!(validate_file_name("a/b").is_err());
        assert!(validate_file_name("a\\b").is_err());
        assert!(validate_file_name("a\0b").is_err());
    }

    #[test]
    fn copy_file_into_directory() {
        let dir = scratch("copy-file");
        let src = dir.join("a.txt");
        write(&src, b"hello");
        let dest_dir = dir.join("dest");
        fs::create_dir(&dest_dir).unwrap();

        let stats = copy_entry(
            &src,
            &dest_dir,
            ConflictPolicy::Fail,
            &CancelFlag::new(),
            |_| {},
        )
        .unwrap();
        assert_eq!(stats.files, 1);
        assert_eq!(stats.bytes, 5);
        assert_eq!(fs::read(dest_dir.join("a.txt")).unwrap(), b"hello");
        assert!(src.exists(), "copy must keep the source");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn copy_refuses_to_overwrite_by_default() {
        let dir = scratch("copy-conflict");
        let src = dir.join("a.txt");
        write(&src, b"new");
        let dest_dir = dir.join("dest");
        write(&dest_dir.join("a.txt"), b"old");

        let err = copy_entry(
            &src,
            &dest_dir,
            ConflictPolicy::Fail,
            &CancelFlag::new(),
            |_| {},
        )
        .unwrap_err();
        assert!(matches!(err, OpError::AlreadyExists(_)));
        assert_eq!(fs::read(dest_dir.join("a.txt")).unwrap(), b"old");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn copy_skip_and_rename_policies() {
        let dir = scratch("copy-policies");
        let src = dir.join("a.txt");
        write(&src, b"new");
        let dest_dir = dir.join("dest");
        write(&dest_dir.join("a.txt"), b"old");

        let skipped = copy_entry(
            &src,
            &dest_dir,
            ConflictPolicy::Skip,
            &CancelFlag::new(),
            |_| {},
        )
        .unwrap();
        assert_eq!(skipped.skipped.len(), 1);
        assert_eq!(fs::read(dest_dir.join("a.txt")).unwrap(), b"old");

        copy_entry(
            &src,
            &dest_dir,
            ConflictPolicy::Rename,
            &CancelFlag::new(),
            |_| {},
        )
        .unwrap();
        assert_eq!(fs::read(dest_dir.join("a (1).txt")).unwrap(), b"new");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn copy_directory_recursively_and_keep_symlink() {
        let dir = scratch("copy-dir");
        let src = dir.join("tree");
        write(&src.join("sub/nested.txt"), b"nested");
        write(&src.join("top.txt"), b"top");
        #[cfg(unix)]
        std::os::unix::fs::symlink("top.txt", src.join("link.txt")).unwrap();
        let dest_dir = dir.join("dest");
        fs::create_dir(&dest_dir).unwrap();

        let stats = copy_entry(
            &src,
            &dest_dir,
            ConflictPolicy::Fail,
            &CancelFlag::new(),
            |_| {},
        )
        .unwrap();
        #[cfg(unix)]
        assert_eq!(stats.files, 3); // top, nested, symlink
        #[cfg(not(unix))]
        assert_eq!(stats.files, 2); // symlink creation is Unix-only in this test
        assert_eq!(stats.dirs, 2);
        assert_eq!(fs::read(dest_dir.join("tree/top.txt")).unwrap(), b"top");
        assert_eq!(
            fs::read(dest_dir.join("tree/sub/nested.txt")).unwrap(),
            b"nested"
        );
        #[cfg(unix)]
        {
            let link = dest_dir.join("tree/link.txt");
            assert!(fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink());
            assert_eq!(fs::read_link(&link).unwrap(), Path::new("top.txt"));
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn copy_reports_monotonic_byte_progress() {
        let dir = scratch("progress");
        let src = dir.join("big.bin");
        let size = (COPY_CHUNK_SIZE * 3 + 7) as u64;
        write(&src, &vec![1u8; size as usize]);
        let dest_dir = dir.join("dest");
        fs::create_dir(&dest_dir).unwrap();

        let mut samples: Vec<u64> = Vec::new();
        let stats = copy_entry(
            &src,
            &dest_dir,
            ConflictPolicy::Fail,
            &CancelFlag::new(),
            |bytes| samples.push(bytes),
        )
        .unwrap();

        assert!(!samples.is_empty(), "progress must be reported per chunk");
        assert!(
            samples.windows(2).all(|pair| pair[0] <= pair[1]),
            "progress must be monotonic: {samples:?}"
        );
        assert_eq!(samples.last().copied(), Some(stats.bytes));
        assert_eq!(stats.bytes, size);
        assert_eq!(
            fs::read(dest_dir.join("big.bin")).unwrap().len() as u64,
            size
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn total_bytes_counts_files_and_directories() {
        let dir = scratch("total");
        write(&dir.join("a.bin"), &[0u8; 10]);
        write(&dir.join("sub/b.bin"), &[0u8; 20]);
        assert_eq!(total_bytes(&[dir.join("a.bin")]), 10);
        assert_eq!(total_bytes(std::slice::from_ref(&dir)), 30);
        assert_eq!(total_bytes(&[dir.join("missing")]), 0);

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(dir.join("a.bin"), dir.join("link.bin")).unwrap();
            // The symlink itself must not be followed/counted.
            assert_eq!(total_bytes(&[dir.join("link.bin")]), 0);
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn copy_into_itself_is_rejected() {
        let dir = scratch("copy-self");
        let src = dir.join("tree");
        fs::create_dir_all(src.join("sub")).unwrap();
        write(&src.join("sub/a.txt"), b"a");

        let err = copy_entry(
            &src,
            &src.join("sub"),
            ConflictPolicy::Fail,
            &CancelFlag::new(),
            |_| {},
        )
        .unwrap_err();
        assert!(matches!(err, OpError::InvalidTarget(_)));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cancellation_leaves_no_partial_file() {
        let dir = scratch("cancel");
        let src = dir.join("big.bin");
        write(&src, &vec![7u8; COPY_CHUNK_SIZE * 4]);
        let dest_dir = dir.join("dest");
        fs::create_dir(&dest_dir).unwrap();

        let cancel = CancelFlag::new();
        cancel.cancel();
        let err = copy_entry(&src, &dest_dir, ConflictPolicy::Fail, &cancel, |_| {}).unwrap_err();
        assert!(matches!(err, OpError::Cancelled));
        assert!(!dest_dir.join("big.bin").exists());
        let leftovers: Vec<_> = fs::read_dir(&dest_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains(".partial-"))
            .collect();
        assert!(leftovers.is_empty(), "partial files must be cleaned up");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn move_within_same_filesystem() {
        let dir = scratch("move");
        let src = dir.join("a.txt");
        write(&src, b"move me");
        let dest_dir = dir.join("dest");
        fs::create_dir(&dest_dir).unwrap();

        let stats = move_entry(
            &src,
            &dest_dir,
            ConflictPolicy::Fail,
            &CancelFlag::new(),
            |_| {},
        )
        .unwrap();
        assert_eq!(stats.created.len(), 1);
        assert!(!src.exists());
        assert_eq!(fs::read(dest_dir.join("a.txt")).unwrap(), b"move me");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn move_into_same_directory_is_a_noop() {
        let dir = scratch("move-noop");
        let src = dir.join("a.txt");
        write(&src, b"x");
        let stats =
            move_entry(&src, &dir, ConflictPolicy::Fail, &CancelFlag::new(), |_| {}).unwrap();
        assert_eq!(stats.files, 0);
        assert!(src.exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rename_entry_refuses_overwrite() {
        let dir = scratch("rename");
        let src = dir.join("a.txt");
        write(&src, b"a");
        write(&dir.join("b.txt"), b"b");

        let err = rename_entry(&src, "b.txt").unwrap_err();
        assert!(matches!(err, OpError::AlreadyExists(_)));
        assert_eq!(fs::read(dir.join("b.txt")).unwrap(), b"b");

        let renamed = rename_entry(&src, "c.txt").unwrap();
        assert_eq!(renamed, dir.join("c.txt"));
        assert_eq!(fs::read(renamed).unwrap(), b"a");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn remove_entry_safe_does_not_follow_symlinks() {
        let dir = scratch("remove-symlink");
        let outside = dir.join("outside");
        write(&outside.join("keep.txt"), b"keep");
        let tree = dir.join("tree");
        fs::create_dir_all(&tree).unwrap();
        write(&tree.join("own.txt"), b"own");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, tree.join("link-outside")).unwrap();

        remove_entry_safe(&tree).unwrap();
        assert!(!tree.exists());
        #[cfg(unix)]
        assert!(
            outside.join("keep.txt").exists(),
            "symlink target must survive"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn exdev_maps_to_crosses_devices() {
        // EXDEV is 18 on Linux. The move fallback relies on std mapping it to
        // ErrorKind::CrossesDevices instead of a hard-coded raw errno.
        let err = std::io::Error::from_raw_os_error(18);
        assert_eq!(err.kind(), std::io::ErrorKind::CrossesDevices);
    }
}

#[cfg(test)]
mod dir_stats_tests {
    #[cfg(unix)]
    use super::unique_path;
    use super::{dir_stats, io_error, CancelFlag, OpError};
    use std::fs;
    use std::path::{Path, PathBuf};

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "gpui-fe-dirstats-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(path: &PathBuf, bytes: usize) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, vec![b'x'; bytes]).unwrap();
    }

    #[test]
    fn counts_files_dirs_and_bytes() {
        let dir = scratch("counts");
        write(&dir.join("a.txt"), 5);
        write(&dir.join("sub/b.txt"), 7);
        write(&dir.join("sub/deeper/c.txt"), 11);

        let stats = dir_stats(&dir, &CancelFlag::new());
        assert_eq!(stats.files, 3);
        assert_eq!(stats.dirs, 2);
        assert_eq!(stats.bytes, 23);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn symlinks_are_counted_but_not_followed() {
        let dir = scratch("symlink");
        write(&dir.join("real.txt"), 4);
        write(&dir.join("outside/secret.txt"), 9);
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(dir.join("real.txt"), dir.join("link.txt")).unwrap();
            std::os::unix::fs::symlink(dir.join("outside"), dir.join("link-dir")).unwrap();
        }

        let stats = dir_stats(&dir, &CancelFlag::new());
        // real.txt + outside/secret.txt + 2 symlinks (Unix only);
        // link-dir is not followed.
        #[cfg(unix)]
        assert_eq!(stats.files, 4);
        #[cfg(not(unix))]
        assert_eq!(stats.files, 2);
        assert_eq!(stats.dirs, 1); // outside (direct child), not link-dir
        assert_eq!(stats.bytes, 13);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cancellation_returns_early() {
        let dir = scratch("cancel");
        write(&dir.join("a.txt"), 1);
        let cancel = CancelFlag::new();
        cancel.cancel();
        let stats = dir_stats(&dir, &cancel);
        assert_eq!(stats, super::DirStats::default());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn io_error_maps_common_kinds() {
        let path = Path::new("/tmp/example");
        let not_found = io_error(path, std::io::Error::from(std::io::ErrorKind::NotFound));
        assert!(matches!(not_found, OpError::NotFound(_)));

        let denied = io_error(
            path,
            std::io::Error::from(std::io::ErrorKind::PermissionDenied),
        );
        assert!(matches!(denied, OpError::PermissionDenied(_)));

        let exists = io_error(
            path,
            std::io::Error::from(std::io::ErrorKind::AlreadyExists),
        );
        assert!(matches!(exists, OpError::AlreadyExists(_)));

        let cross = io_error(
            path,
            std::io::Error::from(std::io::ErrorKind::CrossesDevices),
        );
        assert!(matches!(cross, OpError::Unsupported(_)));

        let is_dir = io_error(path, std::io::Error::from(std::io::ErrorKind::IsADirectory));
        assert!(matches!(is_dir, OpError::InvalidTarget(_)));

        let other = io_error(path, std::io::Error::other("boom"));
        assert!(matches!(other, OpError::Io { .. }));
    }

    #[test]
    fn op_error_display_is_user_readable() {
        let path = Path::new("/tmp/target.txt");
        let cases: Vec<(OpError, &str)> = vec![
            (OpError::NotFound(path.to_path_buf()), "見つかりません"),
            (
                OpError::PermissionDenied(path.to_path_buf()),
                "アクセスが拒否されました",
            ),
            (OpError::AlreadyExists(path.to_path_buf()), "既に存在します"),
            (
                OpError::InvalidTarget("理由".to_string()),
                "操作できません: 理由",
            ),
            (
                OpError::Unsupported("理由".to_string()),
                "未対応の操作です: 理由",
            ),
            (OpError::Cancelled, "操作はキャンセルされました"),
            (
                OpError::Io {
                    path: path.to_path_buf(),
                    message: "詳細".to_string(),
                },
                "/tmp/target.txt: 詳細",
            ),
        ];
        for (error, expected) in cases {
            let text = error.to_string();
            assert!(text.contains(expected), "expected {expected:?} in {text:?}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn unique_path_preserves_non_utf8_names() {
        use std::os::unix::ffi::{OsStrExt, OsStringExt};

        let dir = scratch("non-utf8-unique");
        let name = std::ffi::OsString::from_vec(vec![b'x', 0xff, b'.', b't', b'x', b't']);
        fs::write(dir.join(&name), b"first").unwrap();

        let unique = unique_path(&dir, &name);
        let bytes = unique.file_name().unwrap().as_bytes();
        assert_eq!(
            bytes, b"x\xff (1).txt",
            "raw bytes must survive conflict rename"
        );
        assert!(!unique.exists());
        fs::write(&unique, b"second").unwrap();
        let second = unique_path(&dir, &name);
        assert_eq!(second.file_name().unwrap().as_bytes(), b"x\xff (2).txt");

        let _ = fs::remove_dir_all(&dir);
    }
}
