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
//! - **Destinations are reserved exclusively before use.** `move_entry` and
//!   `rename_entry` create an `O_EXCL`-style reservation so a concurrent
//!   creator cannot slip in between the existence check and the rename. On
//!   Unix the reservation is replaced atomically by `rename`; on Windows
//!   `rename` refuses to replace it, so it is removed first and a residual
//!   TOCTOU window remains (see `Reservation` below).
//! - **Failure cleanup only removes what we created.** `copy_entry` and the
//!   cross-device fallback of `move_entry` key cleanup on a `dest_created` flag
//!   reported by `copy_recursive`, not on a pre-copy existence snapshot. If an
//!   exclusive create loses a race to a concurrent creator, their entry is
//!   never recursively deleted.
//! - **Partial temp names are unique per operation.** A process-wide nonce
//!   joins the timestamp and PID in every `.partial-*` name, and a temp-name
//!   collision is retried instead of deleting a file we did not create.
//!
//! Residual TOCTOU (deliberately documented, not hidden): `std` exposes no
//! atomic "replace only if still ours" primitive for an entire directory tree
//! or for Windows. A concurrent creator that swaps the destination *after* our
//! copy succeeded but *before* verification/cleanup can therefore still have
//! its entry removed; the fix closes the common check→create race, not every
//! theoretically racy interleaving.
//!
//! This module is intentionally GPUI-free so it can be unit tested headlessly.
//! It is exercised by the tests below and backs the cut/copy/paste UI.

use std::fs;
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::error::UNKNOWN_ERROR_REASON;
use crate::model::{AppError, BoxedCause};

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
///
/// Kept as a type alias: callers still construct `OpError::NotFound(..)` and
/// match on `OpError::Cancelled`, but the type is now the shared [`AppError`],
/// whose `source()` exposes the underlying `io::Error` when one exists.
pub type OpError = AppError;

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
    // Plain variants keep the original status text; `with_cause` swaps in the
    // cause-carrying counterpart so `source()` exposes the original failure.
    let plain = match err.kind() {
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
        // AV11: FsError::from_io maps Other to FsErrorKind::Unknown
        // ("エラーが発生しました"). Splitting it out of the catch-all keeps the
        // two sides symmetric; the original io text is still held by the cause
        // and surfaces in `message_ja()` as "(os text)", just like Fs.
        std::io::ErrorKind::Other => OpError::Io {
            path: path.to_path_buf(),
            message: UNKNOWN_ERROR_REASON.to_string(),
        },
        _ => OpError::Io {
            path: path.to_path_buf(),
            message: err.to_string(),
        },
    };
    plain.with_cause(BoxedCause::new(err))
}

/// Metadata of an entry that must exist, preserving dangling symlinks.
///
/// `Path::exists()` collapses every metadata failure — including `EACCES`
/// from a non-searchable parent directory — into `false`, which made callers
/// report a plain `OpError::NotFound` for an inaccessible source. Going
/// through `symlink_metadata` keeps a dangling symlink present (its own inode
/// is stat-able) and routes real failures through [`io_error`], so
/// `PermissionDenied` keeps its own variant while `NotFound` stays `NotFound`.
fn require_entry(path: &Path) -> Result<fs::Metadata, OpError> {
    fs::symlink_metadata(path).map_err(|e| io_error(path, e))
}

/// Ensure `dir` is a usable destination directory without collapsing metadata
/// failures into a boolean like `Path::is_dir()` does.
///
/// Symlinks to directories stay valid (matching `Path::is_dir()`), while a
/// permission failure is reported through `io_error` as `PermissionDenied`
/// instead of a misleading `InvalidTarget`.
fn require_directory(dir: &Path, label: &str) -> Result<(), OpError> {
    let meta = require_entry(dir)?;
    let is_dir = if meta.file_type().is_symlink() {
        fs::metadata(dir).map_err(|e| io_error(dir, e))?.is_dir()
    } else {
        meta.is_dir()
    };
    if !is_dir {
        return Err(OpError::InvalidTarget(format!(
            "{label}がディレクトリではありません: {}",
            dir.display()
        )));
    }
    Ok(())
}

/// Fold a cleanup failure into the error that triggered the cleanup.
///
/// A destination that is already gone needs no cleanup: treating `NotFound`
/// as success keeps cancellation (which often happens before `dest` is
/// created) reporting the original `OpError::Cancelled`. For every other
/// cleanup failure the returned error renders both the original failure and
/// the cleanup failure, and keeps the original error reachable through
/// `Error::source()` so its category is not lost.
fn with_cleanup_failure(primary: OpError, dest: &Path, cleanup: Result<(), OpError>) -> OpError {
    let cleanup_error = match cleanup {
        Ok(()) => return primary,
        Err(OpError::NotFound(_)) | Err(OpError::NotFoundCause(_, _)) => return primary,
        Err(error) => error,
    };
    let message = format!("{primary} … クリーンアップ失敗: {cleanup_error}");
    let path = match &primary {
        OpError::Io { path, .. } | OpError::IoCause { path, .. } => path.clone(),
        _ => dest.to_path_buf(),
    };
    OpError::Io { path, message }.with_cause(BoxedCause::new(primary))
}

/// Validate a single file name (not a path): no separators, no `.`/`..`,
/// no NUL byte, and no characters that Windows reserves.
pub fn validate_file_name(name: &str) -> Result<(), OpError> {
    if name.is_empty() {
        return Err(OpError::InvalidTarget(
            crate::strings::STATUS_RENAME_INVALID_EMPTY.to_string(),
        ));
    }
    if name == "." || name == ".." {
        return Err(OpError::InvalidTarget(
            crate::strings::STATUS_RENAME_INVALID_DOT.to_string(),
        ));
    }
    if name.contains('/') {
        return Err(OpError::InvalidTarget(
            crate::strings::STATUS_RENAME_INVALID_SEPARATOR.to_string(),
        ));
    }
    if name.contains('\0') {
        return Err(OpError::InvalidTarget(
            crate::strings::STATUS_RENAME_INVALID_NUL.to_string(),
        ));
    }
    #[cfg(windows)]
    {
        if name.contains('\\') {
            return Err(OpError::InvalidTarget(
                crate::strings::STATUS_RENAME_INVALID_SEPARATOR.to_string(),
            ));
        }
        const RESERVED: [&str; 22] = [
            "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
            "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
        ];
        let stem = name.split('.').next().unwrap_or(name).to_ascii_uppercase();
        if RESERVED.contains(&stem.as_str()) {
            return Err(OpError::InvalidTarget(format!(
                "{}{name}",
                crate::strings::STATUS_RENAME_INVALID_RESERVED_PREFIX
            )));
        }
        if name.contains(['<', '>', ':', '"', '|', '?', '*']) {
            return Err(OpError::InvalidTarget(
                crate::strings::STATUS_RENAME_INVALID_CHAR.to_string(),
            ));
        }
    }
    Ok(())
}

/// Numbered candidates tried by [`unique_path`] before falling back.
const UNIQUE_PATH_ATTEMPTS: usize = 10_000;

/// Return `dir/name` if free, otherwise the first free `name (N).ext`.
///
/// Builds candidate names as raw `OsString`s so non-UTF-8 file names survive
/// the rename-conflict path without lossy conversion.
pub fn unique_path(dir: &Path, name: &std::ffi::OsStr) -> PathBuf {
    unique_path_with_limit(dir, name, UNIQUE_PATH_ATTEMPTS)
}

/// [`unique_path`] with an explicit attempt bound.
///
/// The bound is a parameter (rather than a `cfg(test)` constant) so tests can
/// drive the exhaustion fallback with a handful of files while production
/// still tries 10,000 candidates. On exhaustion the occupied original path is
/// returned, making the caller fail with `AlreadyExists` instead of looping
/// forever.
fn unique_path_with_limit(dir: &Path, name: &std::ffi::OsStr, attempts: usize) -> PathBuf {
    let candidate = dir.join(name);
    // A dangling symlink reports `exists() == false` but is still an occupied
    // name; skip it just like `rename_entry` does.
    if !candidate.exists() && !candidate.is_symlink() {
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

    for counter in 1..=attempts {
        let mut candidate_name = append_counter(stem, counter);
        if let Some(extension) = extension {
            candidate_name.push(".");
            candidate_name.push(extension);
        }
        let candidate = dir.join(candidate_name);
        if !candidate.exists() && !candidate.is_symlink() {
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
    // Mirror `rename_entry`: a dangling symlink is an existing entry even
    // though `exists()` follows the link and reports false for it.
    if !candidate.exists() && !candidate.is_symlink() {
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

/// Process-wide, monotonically increasing nonce for `.partial-*` temp names.
///
/// Two concurrent copies in this process share the same PID and both start with
/// `stats.files == 0`. On a coarse clock the timestamp can be identical too, so
/// without this nonce they would pick the same temp path: one `create_new`
/// would fail with `AlreadyExists` and its error cleanup would delete the other
/// operation's in-progress partial. The nonce makes the name unique within the
/// process; the timestamp + PID keep it distinct from stale files of earlier
/// runs. Wrapping after `u64::MAX` is harmless because the clock component has
/// advanced by then.
static PARTIAL_NONCE: AtomicU64 = AtomicU64::new(0);

fn next_partial_nonce() -> u64 {
    PARTIAL_NONCE.fetch_add(1, Ordering::Relaxed)
}

/// Build a NAME_MAX-safe partial-file name. The destination name is deliberately
/// not embedded here (see [`copy_file_atomic`]); `files` is the per-operation
/// counter and `nonce` is the process-wide unique value from
/// [`next_partial_nonce`].
fn partial_temp_name(nanos: u128, pid: u32, files: u64, nonce: u64) -> String {
    format!(".partial-{nanos}-{pid}-{files}-{nonce}")
}

/// Reserve a fresh `.partial-*` sibling with `create_new`.
///
/// The candidate-name generator is injected so tests can force deterministic
/// `AlreadyExists` collisions; production passes [`partial_temp_name`]. A
/// colliding name is retried with a fresh candidate and is *never* deleted:
/// only the file this function successfully opens is returned, so another
/// operation's in-progress partial cannot be removed by a retry.
fn reserve_partial(
    parent: &Path,
    dest: &Path,
    mut make_name: impl FnMut() -> String,
) -> Result<(PathBuf, fs::File), OpError> {
    // Bound the retry loop so a directory pre-planted with many colliding names
    // cannot spin forever; the production nonce changes on every attempt, so
    // reaching the cap is only plausible under deliberate interference.
    const PARTIAL_CREATE_ATTEMPTS: usize = 32;
    let mut attempts = 0usize;
    loop {
        let candidate = parent.join(make_name());
        match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => return Ok((candidate, file)),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                attempts += 1;
                if attempts >= PARTIAL_CREATE_ATTEMPTS {
                    return Err(OpError::Io {
                        path: dest.to_path_buf(),
                        message: "一時ファイル名を確保できませんでした".to_string(),
                    });
                }
            }
            Err(error) => return Err(io_error(dest, error)),
        }
    }
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
    // O_EXCL reservation with an unpredictable name: `File::create` on a
    // predictable `.partial-*` path follows a pre-planted symlink and can
    // truncate an unrelated file (CWE-59). nanos + PID + per-operation counter
    // + process-wide nonce keep concurrent copies distinct without colliding
    // with stale temp files. The destination name is deliberately *not*
    // embedded: a legal name close to NAME_MAX would otherwise push the temp
    // name past the limit and make the copy fail with ENAMETOOLONG.
    //
    // A `create_new` collision on the temp name is retried with a fresh nonce
    // instead of being reported as a destination conflict. Crucially, the error
    // path below only removes a partial we successfully opened ourselves, so a
    // retry never deletes another operation's in-progress file.
    // Report progress relative to the bytes copied by previous files, so the
    // callback receives a monotonically increasing byte count for the whole
    // operation.
    let base_bytes = stats.bytes;

    // Open the source before reserving a temp name so a missing/unreadable
    // source leaves no partial behind at all.
    let mut input = fs::File::open(src).map_err(|e| io_error(src, e))?;

    // The O_EXCL reservation loop lives in `reserve_partial`; the injected
    // generator keeps the timestamp / PID / counter / nonce scheme unchanged.
    let files = stats.files;
    let (partial, mut output) = reserve_partial(parent, dest, || {
        partial_temp_name(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0),
            std::process::id(),
            files,
            next_partial_nonce(),
        )
    })?;

    let result = (|| -> Result<u64, OpError> {
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
                .map_err(|e| io_error(dest, e))?;
            total += read as u64;
            progress(base_bytes + total);
        }
        output.flush().map_err(|e| io_error(dest, e))?;
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
///
/// `dest_created` is flipped to `true` exactly when this call successfully
/// creates its `dest`. The flag is shared by the whole recursion: recursion is
/// top-down, so a successful creation at any depth implies the root `dest` was
/// created by this operation first, making one monotone flag sufficient to key
/// the caller's failure cleanup. When the root creation loses a race to a
/// concurrent creator it fails *before* the flag is set, so cleanup leaves that
/// foreign entry untouched (see [`copy_entry`] and `move_entry`'s EXDEV path).
fn copy_recursive(
    src: &Path,
    dest: &Path,
    cancel: &CancelFlag,
    stats: &mut OpStats,
    progress: &mut dyn FnMut(u64),
    dest_created: &mut bool,
) -> Result<(), OpError> {
    if cancel.is_cancelled() {
        return Err(OpError::Cancelled);
    }
    let metadata = fs::symlink_metadata(src).map_err(|e| io_error(src, e))?;

    if metadata.file_type().is_symlink() {
        copy_symlink(src, dest, stats)?;
        *dest_created = true;
        return Ok(());
    }
    if metadata.is_dir() {
        fs::create_dir(dest).map_err(|e| io_error(dest, e))?;
        *dest_created = true;
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
            copy_recursive(
                &child,
                &dest.join(name),
                cancel,
                stats,
                progress,
                dest_created,
            )?;
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
        copy_file_atomic(src, dest, cancel, stats, progress)?;
        *dest_created = true;
        return Ok(());
    }
    Err(OpError::Unsupported(format!(
        "特殊ファイルはコピーできません: {}",
        src.display()
    )))
}

/// What a successful [`verify_transfer`] checked.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VerifyStats {
    /// Number of regular files and symlinks verified.
    pub files: u64,
    /// Number of directories verified (including the root).
    pub dirs: u64,
    /// Total byte length of the verified regular files.
    pub bytes: u64,
}

fn verify_mismatch(path: &Path, what: &str) -> OpError {
    OpError::InvalidTarget(format!(
        "コピー先の{what}がコピー元と一致しません: {}",
        path.display()
    ))
}

fn verify_entry(
    src: &Path,
    dest: &Path,
    stats: &mut VerifyStats,
    cancel: &CancelFlag,
) -> Result<(), OpError> {
    if cancel.is_cancelled() {
        return Err(OpError::Cancelled);
    }
    let src_meta = fs::symlink_metadata(src).map_err(|e| io_error(src, e))?;
    let dest_meta = fs::symlink_metadata(dest).map_err(|e| match e.kind() {
        ErrorKind::NotFound => {
            OpError::InvalidTarget(format!("コピー先が存在しません: {}", dest.display()))
        }
        _ => io_error(dest, e),
    })?;

    let src_link = src_meta.file_type().is_symlink();
    let dest_link = dest_meta.file_type().is_symlink();
    if src_link || dest_link {
        if !src_link || !dest_link {
            return Err(verify_mismatch(dest, "種類"));
        }
        let src_target = fs::read_link(src).map_err(|e| io_error(src, e))?;
        let dest_target = fs::read_link(dest).map_err(|e| io_error(dest, e))?;
        if src_target != dest_target {
            return Err(verify_mismatch(dest, "リンク先"));
        }
        stats.files += 1;
        return Ok(());
    }

    if src_meta.is_dir() || dest_meta.is_dir() {
        if !src_meta.is_dir() || !dest_meta.is_dir() {
            return Err(verify_mismatch(dest, "種類"));
        }
        let mut src_names: Vec<std::ffi::OsString> = Vec::new();
        for entry in fs::read_dir(src).map_err(|e| io_error(src, e))? {
            src_names.push(entry.map_err(|e| io_error(src, e))?.file_name());
        }
        let mut dest_names: Vec<std::ffi::OsString> = Vec::new();
        for entry in fs::read_dir(dest).map_err(|e| io_error(dest, e))? {
            dest_names.push(entry.map_err(|e| io_error(dest, e))?.file_name());
        }
        src_names.sort();
        dest_names.sort();
        if src_names != dest_names {
            return Err(verify_mismatch(dest, "子要素"));
        }
        for name in src_names {
            verify_entry(&src.join(&name), &dest.join(&name), stats, cancel)?;
        }
        stats.dirs += 1;
        return Ok(());
    }

    if src_meta.is_file() && dest_meta.is_file() {
        if src_meta.len() != dest_meta.len() {
            return Err(OpError::InvalidTarget(format!(
                "コピー先のサイズが一致しません: {} ({} != {})",
                dest.display(),
                dest_meta.len(),
                src_meta.len()
            )));
        }
        stats.files += 1;
        stats.bytes += src_meta.len();
        return Ok(());
    }

    Err(verify_mismatch(dest, "種類"))
}

/// Verify a completed transfer: `dest` must exist and match `src` in type,
/// symlink target, directory children and regular-file byte length.
///
/// Read-only and symlink-safe (never follows links). Called by [`copy_entry`]
/// and the cross-device fallback of [`move_entry`] after the write finishes;
/// a failed verification removes the destination so no truncated copy is left
/// behind.
pub fn verify_transfer(
    src: &Path,
    dest: &Path,
    cancel: &CancelFlag,
) -> Result<VerifyStats, OpError> {
    let mut stats = VerifyStats::default();
    verify_entry(src, dest, &mut stats, cancel)?;
    Ok(stats)
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
    // `src.exists()` would hide EACCES behind a `false`; `require_entry`
    // preserves the OS error category while still accepting dangling symlinks.
    require_entry(src)?;
    require_directory(dest_dir, "コピー先")?;
    ensure_not_into_itself(src, dest_dir)?;

    let Some(dest) = resolve_destination(src, dest_dir, policy)? else {
        let mut stats = OpStats::default();
        stats
            .skipped
            .push(dest_dir.join(src.file_name().unwrap_or_default()));
        return Ok(stats);
    };

    let mut stats = OpStats::default();
    // Ownership-aware cleanup: only a destination this operation actually
    // created may be removed on failure. A pre-copy `dest_existed` snapshot is
    // not enough — a concurrent creator can plant `dest` *after* that check and
    // before our exclusive create, and an `AlreadyExists` error must then leave
    // the foreign entry alone. `copy_recursive` reports creation through this
    // flag instead. (A pre-existing dangling symlink is likewise never created
    // by us, so it is also preserved.)
    let mut dest_created = false;
    if let Err(error) = copy_recursive(src, &dest, cancel, &mut stats, progress, &mut dest_created)
    {
        // `copy_recursive` may already have created `dest` (or part of its
        // tree) before failing, for example on cancellation. Clean it up just
        // like the verification-failure path below and the cross-device
        // fallback in `move_entry`, so no half-written destination remains.
        // The flag guarantees we only ever delete our own entry.
        let cleanup = if dest_created {
            remove_entry_safe(&dest)
        } else {
            Ok(())
        };
        return Err(with_cleanup_failure(error, &dest, cleanup));
    }
    if let Err(error) = verify_transfer(src, &dest, cancel) {
        // `copy_recursive` succeeded, so `dest_created` is true and the entry is
        // ours; never leave a truncated or structurally different copy behind.
        // Residual TOCTOU: a concurrent creator that swaps `dest` between the
        // copy and this cleanup could still lose its entry — `std` exposes no
        // atomic "replace only if still ours" primitive for the whole tree.
        let cleanup = remove_entry_safe(&dest);
        return Err(with_cleanup_failure(error, &dest, cleanup));
    }
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
        // Collect the child paths first and finish with `read_dir` before
        // recursing: holding a directory handle open while deleting its
        // children prevents removing the directory itself on Windows, and
        // mirrors `copy_recursive` / `verify_entry` above.
        let mut children: Vec<PathBuf> = Vec::new();
        let reader = fs::read_dir(path).map_err(|e| io_error(path, e))?;
        for entry in reader {
            let entry = entry.map_err(|e| io_error(path, e))?;
            children.push(entry.path());
        }
        children.sort();
        for child in children {
            remove_entry_safe(&child)?;
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

/// An exclusive reservation of a destination path, created before `fs::rename`
/// so a concurrent creator cannot slip in between the existence check and the
/// rename.
///
/// On Unix the reservation stays in place and `rename` replaces it atomically:
/// an empty directory for directory sources (POSIX permits renaming a directory
/// onto an empty directory), a regular file for everything else — including
/// symlinks, which are not directory inodes even when they point at one.
///
/// On Windows `fs::rename` refuses to replace an existing entry, so the
/// reservation is released just before the rename and `owned` is cleared.
/// Failure cleanup then leaves `path` alone because it may belong to a
/// concurrent creator. This remove-then-rename sequence keeps only the
/// check→reserve race closed; a residual TOCTOU window remains because `std`
/// exposes no atomic replace-if-absent operation on Windows. It is deliberately
/// preferred over a two-step (source → temp → destination) rename, which would
/// move the source before the collision is known and change failure semantics.
#[derive(Debug)]
struct Reservation {
    path: PathBuf,
    /// True only while the entry at `path` is still the one we created and may
    /// therefore be safely removed by failure cleanup.
    owned: bool,
    /// True when the reservation is a directory (Unix directory sources).
    is_dir: bool,
}

impl Reservation {
    /// Reserve `path` exclusively. `src_is_dir` must describe the source's own
    /// file type (from `symlink_metadata`), not a symlink's target.
    fn create(path: &Path, src_is_dir: bool) -> std::io::Result<Self> {
        #[cfg(unix)]
        {
            if src_is_dir {
                fs::create_dir(path)?;
                return Ok(Self {
                    path: path.to_path_buf(),
                    owned: true,
                    is_dir: true,
                });
            }
        }
        let _ = src_is_dir; // not read on non-Unix platforms
        let file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        drop(file);
        Ok(Self {
            path: path.to_path_buf(),
            owned: true,
            is_dir: false,
        })
    }

    /// Remove our reservation before a rename on platforms that cannot replace
    /// an existing entry (Windows). Afterwards `owned` is false, so cleanup
    /// will never delete an entry a concurrent creator may have placed there.
    #[cfg(any(windows, test))]
    fn release_for_rename(&mut self) {
        if self.owned {
            let _ = fs::remove_file(&self.path);
            self.owned = false;
        }
    }

    /// Best-effort removal of the reservation we still own. A no-op once
    /// `owned` is false, so a concurrent creator's entry is never deleted.
    fn cleanup(&mut self) {
        if !self.owned {
            return;
        }
        let removed = if self.is_dir {
            fs::remove_dir(&self.path)
        } else {
            fs::remove_file(&self.path)
        };
        if removed.is_ok() {
            self.owned = false;
        }
    }
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
    // `src.exists()` would hide EACCES behind a `false`; `require_entry`
    // preserves the OS error category while still accepting dangling symlinks.
    require_entry(src)?;
    require_directory(dest_dir, "移動先")?;
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

    // Use the source's own file type for the reservation: a symlink to a
    // directory is still a symlink inode, and `rename` cannot replace a
    // directory with a symlink.
    let src_is_dir = fs::symlink_metadata(src)
        .map(|meta| meta.file_type().is_dir())
        .unwrap_or(false);
    let mut reservation = match Reservation::create(&dest, src_is_dir) {
        Ok(reservation) => reservation,
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {
            return Err(OpError::AlreadyExists(dest));
        }
        Err(error) => return Err(io_error(&dest, error)),
    };
    // Only Windows needs the pre-rename removal. `release_for_rename` clears
    // `owned`, so a later failure cannot delete a concurrent creator's entry.
    #[cfg(windows)]
    reservation.release_for_rename();
    match fs::rename(src, &dest) {
        Ok(()) => {
            let mut stats = OpStats::default();
            stats.created.push(dest);
            on_progress(0);
            Ok(stats)
        }
        Err(error) if error.kind() == ErrorKind::CrossesDevices => {
            // `copy_recursive` needs `dest` to be absent. Remove our own
            // reservation first (a directory for Unix directory sources), then
            // fall back to copy-then-delete, keeping the source intact until the
            // copy finished successfully. The copy path reserves `dest` itself.
            reservation.cleanup();
            let mut stats = OpStats::default();
            let progress: &mut dyn FnMut(u64) = &mut on_progress;
            // Once our reservation is released there is a window in which a
            // concurrent creator can claim `dest`. Track whether the fallback
            // copy actually created it so an `AlreadyExists` failure never
            // turns our cleanup into a recursive delete of foreign data.
            let mut dest_created = false;
            if let Err(copy_err) =
                copy_recursive(src, &dest, cancel, &mut stats, progress, &mut dest_created)
            {
                let cleanup = if dest_created {
                    remove_entry_safe(&dest)
                } else {
                    Ok(())
                };
                return Err(with_cleanup_failure(copy_err, &dest, cleanup));
            }
            // The copy succeeded, so `dest` is ours; the verify/cancel paths
            // below clean it up. Residual TOCTOU: a concurrent creator that
            // swaps `dest` after our copy could still lose its entry, because
            // `std` offers no atomic "replace only if still ours" for a tree.
            if let Err(verify_err) = verify_transfer(src, &dest, cancel) {
                let cleanup = remove_entry_safe(&dest);
                return Err(with_cleanup_failure(verify_err, &dest, cleanup));
            }
            on_progress(stats.bytes);
            if cancel.is_cancelled() {
                let cleanup = remove_entry_safe(&dest);
                return Err(with_cleanup_failure(OpError::Cancelled, &dest, cleanup));
            }
            if let Err(remove_err) = remove_entry_safe(src) {
                // AY01: the copy is complete but the source survived. Keeping
                // `dest` as well leaves two copies of the same data and makes
                // the UI treat the move as a partial failure: it retains the
                // clipboard, so every Ctrl+V retry resolves a fresh
                // `name (1)`, `name (2)`, … destination. Roll back only the
                // `dest` this fallback created and keep `src` as the single
                // surviving copy, exactly like the copy/verify/cancel paths
                // above. `with_cleanup_failure` preserves the original error
                // (and reports a cleanup failure when one occurs).
                let primary = OpError::Io {
                    path: src.to_path_buf(),
                    message: format!(
                        "コピーは完了しましたが元を削除できませんでした: {remove_err}"
                    ),
                };
                let cleanup = remove_entry_safe(&dest);
                return Err(with_cleanup_failure(primary, &dest, cleanup));
            }
            stats.created.insert(0, dest);
            Ok(stats)
        }
        Err(error) => {
            // Remove only the reservation we still own; after a Windows
            // release this is a no-op and a concurrent creator's file survives.
            reservation.cleanup();
            Err(io_error(&dest, error))
        }
    }
}

/// Rename `src` to `new_name` in its current directory. Refuses to overwrite
/// an existing entry. Case-only renames go through a temporary name so they
/// work on case-insensitive filesystems too.
pub fn rename_entry(src: &Path, new_name: &str) -> Result<PathBuf, OpError> {
    validate_file_name(new_name)?;
    // `src.exists()` would hide EACCES behind a `false`; `require_entry`
    // preserves the OS error category while still accepting dangling symlinks.
    require_entry(src)?;
    let parent = src
        .parent()
        .ok_or_else(|| OpError::InvalidTarget("親ディレクトリがありません".to_string()))?;
    let target = parent.join(new_name);
    if target == src {
        return Ok(target);
    }
    // `Path::exists()` follows the link and reports false for a dangling
    // symlink; `is_symlink()` uses `symlink_metadata`, so a broken symlink is
    // still recognised as an existing entry that must never be overwritten.
    if target.exists() || target.is_symlink() {
        // Case-only rename on a case-insensitive filesystem: the target is the
        // same file. Route through a temporary name.
        // `canonicalize` fails for a dangling symlink, so fall back to inode
        // identity on Unix; Windows keeps the canonicalize path (its
        // case-insensitive FS is covered there and symlink metadata differs).
        #[cfg(unix)]
        let same_file = {
            use std::os::unix::fs::MetadataExt;
            match (fs::symlink_metadata(src), fs::symlink_metadata(&target)) {
                (Ok(a), Ok(b)) => a.dev() == b.dev() && a.ino() == b.ino(),
                _ => false,
            }
        };
        #[cfg(not(unix))]
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
        if let Err(error) = fs::rename(&temp, &target) {
            // The first rename already removed `src`. Roll back to that path
            // so a failed second rename never leaves the entry only at the
            // hidden `.rename-*` temp name. If rollback also fails, surface the
            // temp location instead of silently losing track of the file.
            if let Err(rollback_error) = fs::rename(&temp, src) {
                return Err(OpError::Io {
                    message: format!(
                        "リネームに失敗し、元への復元もできませんでした: {error}; 一時ファイル: {}; 復元エラー: {rollback_error}",
                        temp.display()
                    ),
                    path: temp,
                });
            }
            return Err(io_error(&target, error));
        }
        return Ok(target);
    }
    // Reserve `target` with exclusive semantics before the final rename so a
    // concurrent creator cannot slip in between the existence check above and
    // the rename (the same guard `copy_entry` / `move_entry` use). Unix uses an
    // empty-directory reservation for directory sources so the rename replaces
    // it atomically instead of hitting ENOTDIR. The case-only branch above
    // deliberately has no reservation: its target already exists.
    let src_is_dir = fs::symlink_metadata(src)
        .map(|meta| meta.file_type().is_dir())
        .unwrap_or(false);
    let mut reservation = match Reservation::create(&target, src_is_dir) {
        Ok(reservation) => reservation,
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {
            return Err(OpError::AlreadyExists(target));
        }
        Err(error) => return Err(io_error(&target, error)),
    };
    // Windows cannot rename onto the reservation; releasing it clears `owned`,
    // so the failure path below cannot delete a concurrent creator's entry.
    #[cfg(windows)]
    reservation.release_for_rename();
    match fs::rename(src, &target) {
        Ok(()) => Ok(target),
        Err(error) => {
            // Remove only the reservation we still own. On Windows the
            // reservation is already released and this is a no-op; an
            // unconditional `remove_file(&target)` could delete a file placed
            // here by a concurrent creator after the release.
            reservation.cleanup();
            Err(io_error(src, error))
        }
    }
}

/// Minimal batch rename: apply [`rename_entry`] to every `(source, new_name)`
/// pair in order and return the number of successful renames.
///
/// The operation stops at the first error. Earlier renames are **not** rolled
/// back, so callers should validate names up front (for example with
/// [`validate_file_name`]) and surface the partially-completed count.
pub fn batch_rename(renames: &[(PathBuf, String)]) -> Result<usize, OpError> {
    let mut done = 0usize;
    for (src, new_name) in renames {
        rename_entry(src, new_name)?;
        done += 1;
    }
    Ok(done)
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
    fn partial_names_are_unique_within_one_process() {
        // Two concurrent copies share PID and start with files == 0. Even with
        // an identical (coarse) timestamp, the process-wide nonce must make the
        // temp names differ so neither operation deletes the other's partial.
        let nanos = 1_700_000_000_000_000_000u128;
        let pid = std::process::id();
        let first_nonce = next_partial_nonce();
        let second_nonce = next_partial_nonce();
        assert_ne!(first_nonce, second_nonce, "nonce must be monotonic");

        let first = partial_temp_name(nanos, pid, 0, first_nonce);
        let second = partial_temp_name(nanos, pid, 0, second_nonce);
        assert_ne!(
            first, second,
            "identical clock/PID must still yield unique names"
        );
        assert!(first.starts_with(".partial-"));
        assert!(second.starts_with(".partial-"));
        // NAME_MAX-safe: destination name is deliberately not embedded.
        assert!(first.len() < 255 && second.len() < 255);
    }

    #[test]
    fn partial_reservation_retries_collisions_then_succeeds() {
        let dir = scratch("partial-retry");
        let dest = dir.join("dest.txt");
        // 31 collisions stay below the 32-attempt cap; the next name is free.
        let collisions: Vec<String> = (0..31).map(|i| format!("planted-{i}")).collect();
        for name in &collisions {
            write(&dir.join(name), format!("foreign-{name}").as_bytes());
        }
        let mut generator = collisions
            .iter()
            .cloned()
            .chain(std::iter::once("fresh".to_string()));
        let (partial, _file) =
            reserve_partial(&dir, &dest, || generator.next().expect("name sequence"))
                .expect("collisions below the cap must be retried");
        assert_eq!(partial, dir.join("fresh"));
        for name in &collisions {
            assert_eq!(
                fs::read(dir.join(name)).unwrap(),
                format!("foreign-{name}").as_bytes(),
                "a retry must not delete a foreign partial: {name}"
            );
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn partial_reservation_exhaustion_errors_without_deleting_collisions() {
        let dir = scratch("partial-exhaust");
        let dest = dir.join("dest.txt");
        let collisions: Vec<String> = (0..32).map(|i| format!("planted-{i}")).collect();
        for name in &collisions {
            write(&dir.join(name), format!("foreign-{name}").as_bytes());
        }
        let mut generator = collisions.iter().cloned();
        let err = reserve_partial(&dir, &dest, || generator.next().expect("name sequence"))
            .expect_err("32 collisions must exhaust the retry budget");
        match &err {
            OpError::Io { path, message } => {
                assert_eq!(path, &dest, "the error must reference the destination");
                assert!(
                    message.contains("一時ファイル名を確保できませんでした"),
                    "unexpected message: {message}"
                );
            }
            other => panic!("unexpected error variant: {other:?}"),
        }
        for name in &collisions {
            assert_eq!(
                fs::read(dir.join(name)).unwrap(),
                format!("foreign-{name}").as_bytes(),
                "exhaustion must not delete a foreign partial: {name}"
            );
        }
        assert!(!dest.exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn partial_reservation_does_not_delete_a_partial_created_during_retry() {
        let dir = scratch("partial-concurrent");
        let dest = dir.join("dest.txt");
        let occupied = dir.join("occupied");
        let foreign = dir.join("foreign-partial");
        write(&occupied, b"foreign-occupied");
        let mut call = 0;
        let (partial, _file) = reserve_partial(&dir, &dest, || {
            call += 1;
            match call {
                1 => "occupied".to_string(),
                2 => {
                    // Another operation reserves its own partial between our
                    // attempts; that in-progress file must survive.
                    write(&foreign, b"in-progress");
                    "foreign-partial".to_string()
                }
                _ => "fresh-partial".to_string(),
            }
        })
        .expect("a fresh name after two collisions must succeed");
        assert_eq!(partial, dir.join("fresh-partial"));
        assert_eq!(fs::read(&foreign).unwrap(), b"in-progress");
        assert_eq!(fs::read(&occupied).unwrap(), b"foreign-occupied");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn concurrent_copies_keep_their_own_partials() {
        // Distinct destination names avoid a destination-level race, isolating
        // the temp-name race: both operations have the same PID and files == 0.
        let dir = scratch("concurrent-partials");
        let a_src = dir.join("a-src/a.txt");
        let b_src = dir.join("b-src/b.txt");
        write(&a_src, b"aaa");
        write(&b_src, b"bbb");
        // One shared destination directory: both partials are created as
        // siblings there, so a name collision would make one operation delete
        // the other's in-progress file. The destination file names differ, so
        // there is no destination-level race to confuse the result.
        let dest_dir = dir.join("dest");
        fs::create_dir(&dest_dir).unwrap();

        let a_dest = dest_dir.clone();
        let b_dest = dest_dir.clone();
        let a = std::thread::spawn(move || {
            copy_entry(
                &a_src,
                &a_dest,
                ConflictPolicy::Fail,
                &CancelFlag::new(),
                |_| {},
            )
        });
        let b = std::thread::spawn(move || {
            copy_entry(
                &b_src,
                &b_dest,
                ConflictPolicy::Fail,
                &CancelFlag::new(),
                |_| {},
            )
        });
        assert_eq!(a.join().unwrap().unwrap().files, 1);
        assert_eq!(b.join().unwrap().unwrap().files, 1);
        assert_eq!(fs::read(dest_dir.join("a.txt")).unwrap(), b"aaa");
        assert_eq!(fs::read(dest_dir.join("b.txt")).unwrap(), b"bbb");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn validate_file_name_rejects_paths_and_dots() {
        assert!(validate_file_name("ok.txt").is_ok());
        assert!(validate_file_name("").is_err());
        assert!(validate_file_name(".").is_err());
        assert!(validate_file_name("..").is_err());
        assert!(validate_file_name("a/b").is_err());
        #[cfg(windows)]
        {
            assert!(validate_file_name("a\\b").is_err());
        }
        assert!(validate_file_name("a\0b").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn validate_file_name_accepts_backslash_on_unix() {
        assert!(validate_file_name("a\\b").is_ok());
    }

    #[cfg(windows)]
    #[test]
    fn validate_file_name_rejects_windows_reserved_names() {
        // Windows treats the stem before the first dot as a device name, so a
        // legal-looking extension must not smuggle a reserved name through.
        for name in [
            "CON", "CON.txt", "PRN", "AUX", "NUL", "COM1", "COM9", "LPT1", "LPT9",
            "com1.log", // comparison is case-insensitive
        ] {
            assert!(
                validate_file_name(name).is_err(),
                "reserved Windows name must be rejected: {name}"
            );
        }
        // Names that merely start with a reserved word are ordinary file names.
        for name in ["CONSOLE.txt", "NULLED.txt", "COM10.txt", "LPT0.txt"] {
            assert!(
                validate_file_name(name).is_ok(),
                "non-reserved name must be accepted: {name}"
            );
        }
    }

    #[cfg(windows)]
    #[test]
    fn validate_file_name_rejects_windows_forbidden_chars() {
        for name in ["a<b", "a>b", "a:b", "a\"b", "a|b", "a?b", "a*b"] {
            assert!(
                validate_file_name(name).is_err(),
                "forbidden Windows character must be rejected: {name}"
            );
        }
    }

    #[test]
    fn with_cleanup_failure_not_found_keeps_original_error() {
        let dest = Path::new("/tmp/dest.txt");
        let err = with_cleanup_failure(
            OpError::Cancelled,
            dest,
            Err(OpError::NotFound(dest.to_path_buf())),
        );
        assert!(
            matches!(err, OpError::Cancelled),
            "a missing destination needs no cleanup: {err:?}"
        );

        // The cause-carrying counterpart takes the same branch.
        let err = with_cleanup_failure(
            OpError::Cancelled,
            dest,
            Err(OpError::NotFoundCause(
                dest.to_path_buf(),
                BoxedCause::text("missing"),
            )),
        );
        assert!(
            matches!(err, OpError::Cancelled),
            "NotFoundCause must be treated like NotFound: {err:?}"
        );
    }

    #[test]
    fn with_cleanup_failure_other_error_composes_message_and_keeps_source() {
        let dest = Path::new("/tmp/dest.txt");
        let err = with_cleanup_failure(
            OpError::Cancelled,
            dest,
            Err(OpError::PermissionDenied(dest.to_path_buf())),
        );
        let text = err.to_string();
        assert!(
            text.contains("操作はキャンセルされました")
                && text.contains("クリーンアップ失敗")
                && text.contains("アクセスが拒否されました"),
            "composed text must report both failures: {text}"
        );
        assert_eq!(
            std::error::Error::source(&err).map(ToString::to_string),
            Some("操作はキャンセルされました".to_string()),
            "the primary error must stay reachable through source()"
        );

        // An `Io`/`IoCause` primary adopts its ORIGINAL path, not the cleanup
        // destination: the error must point at the failing source operation.
        let original = Path::new("/tmp/original.txt");
        for primary in [
            OpError::Io {
                path: original.to_path_buf(),
                message: "読み取り失敗".to_string(),
            },
            OpError::IoCause {
                path: original.to_path_buf(),
                message: "読み取り失敗".to_string(),
                cause: BoxedCause::text("io failure"),
            },
        ] {
            let err = with_cleanup_failure(
                primary,
                dest,
                Err(OpError::PermissionDenied(dest.to_path_buf())),
            );
            // `Io` gets promoted to the cause-carrying `IoCause` by
            // `with_cleanup_failure`, so accept both shapes.
            let path = match &err {
                OpError::Io { path, .. } | OpError::IoCause { path, .. } => path,
                other => panic!("composed error must be Io/IoCause: {other:?}"),
            };
            assert_eq!(
                path,
                &original.to_path_buf(),
                "the primary Io path must win over the cleanup dest"
            );
        }
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

    #[cfg(unix)]
    #[test]
    fn copy_fail_keeps_preexisting_dangling_symlink() {
        let dir = scratch("copy-dangling-fail");
        let src = dir.join("a.txt");
        write(&src, b"new");
        let dest_dir = dir.join("dest");
        fs::create_dir(&dest_dir).unwrap();
        let missing = Path::new("missing");
        std::os::unix::fs::symlink(missing, dest_dir.join("a.txt")).unwrap();

        // A dangling symlink reports `exists() == false` but is still an
        // existing destination entry that must never be overwritten.
        assert!(!dest_dir.join("a.txt").exists());
        assert!(dest_dir.join("a.txt").is_symlink());

        let err = copy_entry(
            &src,
            &dest_dir,
            ConflictPolicy::Fail,
            &CancelFlag::new(),
            |_| {},
        )
        .unwrap_err();
        assert!(matches!(err, OpError::AlreadyExists(_)));
        assert_eq!(fs::read(&src).unwrap(), b"new", "source must be unchanged");

        // The user's pre-existing symlink must survive the error cleanup,
        // still pointing at the same (missing) target.
        let link = dest_dir.join("a.txt");
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink(),
            "cleanup must not delete a pre-existing dangling symlink"
        );
        assert_eq!(fs::read_link(&link).unwrap(), missing);

        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn copy_reports_cleanup_failure_alongside_copy_error() {
        use std::os::unix::fs::PermissionsExt;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let dir = scratch("copy-cleanup-failure");
        let src = dir.join("tree");
        fs::create_dir(&src).unwrap();
        write(&src.join("a.bin"), b"a");
        write(&src.join("b.bin"), &[b'b'; COPY_CHUNK_SIZE + 1]);
        let dest_dir = dir.join("dest");
        fs::create_dir(&dest_dir).unwrap();
        let dest_tree = dest_dir.join("tree");

        // Make the freshly created destination read-only while the second
        // file is being copied. Its final reservation then fails with
        // PermissionDenied, and the cleanup that follows cannot remove the
        // first file (nor the directory) because the parent is read-only.
        let progress_calls = AtomicUsize::new(0);
        let err = copy_entry(
            &src,
            &dest_dir,
            ConflictPolicy::Fail,
            &CancelFlag::new(),
            |_| {
                if progress_calls.fetch_add(1, Ordering::SeqCst) == 1 {
                    fs::set_permissions(&dest_tree, fs::Permissions::from_mode(0o555)).unwrap();
                }
            },
        )
        .unwrap_err();

        // Restore permissions before removing the temporary tree.
        let _ = fs::set_permissions(&dest_tree, fs::Permissions::from_mode(0o755));
        let _ = fs::set_permissions(&dest_dir, fs::Permissions::from_mode(0o755));

        let text = err.to_string();
        assert!(
            text.contains("クリーンアップ失敗"),
            "cleanup failure must be reported: {text}"
        );
        assert!(
            text.contains("アクセスが拒否されました"),
            "the original copy failure must be reported too: {text}"
        );
        assert!(
            std::error::Error::source(&err).is_some(),
            "the original failure must remain in the source chain"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn copy_rename_keeps_preexisting_dangling_symlink() {
        let dir = scratch("copy-dangling-rename");
        let src = dir.join("a.txt");
        write(&src, b"new");
        let dest_dir = dir.join("dest");
        fs::create_dir(&dest_dir).unwrap();
        let missing = Path::new("missing");
        std::os::unix::fs::symlink(missing, dest_dir.join("a.txt")).unwrap();

        copy_entry(
            &src,
            &dest_dir,
            ConflictPolicy::Rename,
            &CancelFlag::new(),
            |_| {},
        )
        .unwrap();

        // A fresh unique name is chosen instead of clobbering the symlink.
        let link = dest_dir.join("a.txt");
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink(),
            "the pre-existing dangling symlink must survive a rename copy"
        );
        assert_eq!(fs::read_link(&link).unwrap(), missing);
        assert_eq!(fs::read(dest_dir.join("a (1).txt")).unwrap(), b"new");

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
    fn cancellation_before_directory_copy_leaves_no_destination() {
        let dir = scratch("cancel-dir-before");
        let src = dir.join("tree");
        write(&src.join("a.txt"), b"a");
        write(&src.join("b.txt"), b"b");
        let dest_dir = dir.join("dest");
        fs::create_dir(&dest_dir).unwrap();

        // Already cancelled: `copy_recursive` rejects the tree at its first
        // check, before it can create the destination directory.
        let cancel = CancelFlag::new();
        cancel.cancel();
        let err = copy_entry(&src, &dest_dir, ConflictPolicy::Fail, &cancel, |_| {}).unwrap_err();
        assert!(matches!(err, OpError::Cancelled));
        assert!(fs::symlink_metadata(dest_dir.join("tree")).is_err());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cancellation_mid_directory_copy_removes_destination() {
        let dir = scratch("cancel-dir-mid");
        let src = dir.join("tree");
        write(&src.join("a.txt"), b"a");
        write(&src.join("b.txt"), b"b");
        let dest_dir = dir.join("dest");
        fs::create_dir(&dest_dir).unwrap();

        // Cancel from the progress callback after the first file was copied,
        // so `copy_recursive` has already created the destination tree when
        // it observes the cancellation and returns `Err`.
        let cancel = CancelFlag::new();
        let err = copy_entry(&src, &dest_dir, ConflictPolicy::Fail, &cancel, |_| {
            cancel.cancel()
        })
        .unwrap_err();
        assert!(matches!(err, OpError::Cancelled));
        assert!(
            fs::symlink_metadata(dest_dir.join("tree")).is_err(),
            "a cancelled directory copy must not leave a half-written destination behind"
        );

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
    fn move_directory_into_another_directory() {
        let dir = scratch("move-dir");
        let src = dir.join("folder");
        fs::create_dir(&src).unwrap();
        write(&src.join("child.txt"), b"content");
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
        assert_eq!(
            fs::read(dest_dir.join("folder/child.txt")).unwrap(),
            b"content"
        );
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

    #[cfg(unix)]
    #[test]
    fn case_only_rename_recognises_dangling_symlink_by_inode() {
        use std::os::unix::fs::{symlink, MetadataExt};

        let dir = scratch("case-only-dangling");
        let target_path = dir.join("missing-target");
        let src = dir.join("Link");
        symlink(&target_path, &src).unwrap();

        // A second dirent with a DIFFERENT name sharing the SAME inode. On a
        // case-insensitive FS `Link`/`link` would already be one dirent; on
        // Linux the hard link is what lets the case-only branch be reached.
        // `src != target` also bypasses the early `target == src` return, so
        // only the inode comparison (canonicalize fails for dangling links)
        // can make this rename succeed.
        let target = dir.join("link");
        fs::hard_link(&src, &target).unwrap();
        let src_meta = fs::symlink_metadata(&src).unwrap();
        let target_meta = fs::symlink_metadata(&target).unwrap();
        assert_eq!(
            (src_meta.dev(), src_meta.ino()),
            (target_meta.dev(), target_meta.ino()),
            "fixture must share one inode across two distinct names"
        );
        assert!(target.is_symlink() && !target.exists());

        let renamed = rename_entry(&src, "link").unwrap();
        assert_eq!(renamed, target);
        assert!(
            target.is_symlink(),
            "the dangling symlink must survive the rename"
        );
        assert_eq!(
            fs::read_link(&target).unwrap(),
            target_path,
            "the symlink target must be preserved"
        );
        assert!(
            !src.is_symlink() && !src.exists(),
            "the source dirent must have moved to the new name"
        );
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
    fn reservation_cleanup_only_removes_what_it_owns() {
        let dir = scratch("reservation-owned");
        let path = dir.join("dest");
        let mut reservation = Reservation::create(&path, false).unwrap();
        assert!(path.is_file(), "regular-file reservation expected");

        // Simulate Windows: the reservation is released before the rename, so
        // it is no longer ours to clean up.
        reservation.release_for_rename();
        assert!(!path.exists());

        // A concurrent creator claims the now-free path. Failure cleanup must
        // not delete that entry.
        write(&path, b"concurrent");
        reservation.cleanup();
        assert_eq!(
            fs::read(&path).unwrap(),
            b"concurrent",
            "cleanup must not remove a concurrent creator's file"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn reservation_for_directory_is_an_empty_directory() {
        let dir = scratch("reservation-dir");
        let path = dir.join("dest");
        let mut reservation = Reservation::create(&path, true).unwrap();
        assert!(reservation.is_dir);
        assert!(
            path.is_dir(),
            "directory sources need a directory reservation"
        );
        assert_eq!(fs::read_dir(&path).unwrap().count(), 0);

        reservation.cleanup();
        assert!(!path.exists(), "cleanup removes the reservation directory");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rename_entry_renames_directory_to_fresh_name() {
        let dir = scratch("rename-dir");
        let src = dir.join("old");
        fs::create_dir(&src).unwrap();
        write(&src.join("child.txt"), b"child");

        let renamed = rename_entry(&src, "new").unwrap();
        assert_eq!(renamed, dir.join("new"));
        assert!(renamed.is_dir());
        assert_eq!(fs::read(renamed.join("child.txt")).unwrap(), b"child");
        assert!(!src.exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn rename_entry_renames_symlink_to_directory() {
        let dir = scratch("rename-link-dir");
        let folder = dir.join("folder");
        fs::create_dir(&folder).unwrap();
        let src = dir.join("link");
        std::os::unix::fs::symlink("folder", &src).unwrap();

        // `src.is_dir()` would follow the symlink and wrongly pick a directory
        // reservation; `symlink_metadata` keeps it a regular-file reservation.
        let renamed = rename_entry(&src, "link2").unwrap();
        assert_eq!(fs::read_link(&renamed).unwrap(), Path::new("folder"));
        assert!(folder.is_dir());
        assert!(!src.exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn rename_entry_allows_broken_symlink_and_preserves_target() {
        let dir = scratch("rename-broken-link");
        let src = dir.join("link.txt");
        let old_target = Path::new("nonexistent-target.txt");
        std::os::unix::fs::symlink(old_target, &src).unwrap();

        // A broken symlink does not satisfy `Path::exists()` but must still be
        // treated as an existing entry, matching `copy_entry`/`move_entry`.
        assert!(!src.exists());
        assert!(src.is_symlink());

        let renamed = rename_entry(&src, "renamed-link.txt").unwrap();
        assert_eq!(renamed, dir.join("renamed-link.txt"));
        assert!(
            fs::symlink_metadata(&src).is_err(),
            "old link path must be gone"
        );

        let metadata = fs::symlink_metadata(&renamed).unwrap();
        assert!(metadata.file_type().is_symlink());
        assert_eq!(fs::read_link(&renamed).unwrap(), old_target);

        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn rename_entry_refuses_to_overwrite_broken_symlink_destination() {
        let dir = scratch("rename-broken-link-dest");
        let src = dir.join("a.txt");
        write(&src, b"a");
        let dangling_target = Path::new("nonexistent-target.txt");
        std::os::unix::fs::symlink(dangling_target, dir.join("b.txt")).unwrap();

        // A dangling symlink is an existing entry even though `exists()` is
        // false; renaming onto it must fail instead of silently replacing it.
        assert!(!dir.join("b.txt").exists());
        assert!(dir.join("b.txt").is_symlink());

        let err = rename_entry(&src, "b.txt").unwrap_err();
        assert!(matches!(err, OpError::AlreadyExists(_)));
        assert_eq!(fs::read_link(dir.join("b.txt")).unwrap(), dangling_target);
        assert_eq!(fs::read(&src).unwrap(), b"a", "source must be unchanged");

        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn rename_entry_rolls_back_temp_when_second_rename_fails() {
        let dir = scratch("rename-rollback");
        // `src` is a symlink to a nonempty directory and the target is that
        // directory, so both paths canonicalize identically and rename_entry
        // takes the case-only branch. The first rename moves the symlink to
        // `.rename-*`; the second rename (temp symlink -> nonempty directory)
        // then fails, exercising the rollback path.
        let folder = dir.join("folder");
        fs::create_dir(&folder).unwrap();
        write(&folder.join("child.txt"), b"child");
        let src = dir.join("link");
        std::os::unix::fs::symlink("folder", &src).unwrap();

        assert!(rename_entry(&src, "folder").is_err());
        assert!(
            fs::symlink_metadata(&src).is_ok(),
            "original src must be restored after the failed second rename"
        );
        assert_eq!(fs::read_link(&src).unwrap(), Path::new("folder"));
        assert_eq!(fs::read(folder.join("child.txt")).unwrap(), b"child");

        let leftovers: Vec<PathBuf> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with(".rename-"))
            })
            .collect();
        assert!(
            leftovers.is_empty(),
            "no .rename-* temp file may be left behind: {leftovers:?}"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn verify_transfer_accepts_matching_tree() {
        let dir = scratch("verify-ok");
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
        assert_eq!(stats.files, 3);
        #[cfg(not(unix))]
        assert_eq!(stats.files, 2);

        let verified = verify_transfer(&src, &dest_dir.join("tree"), &CancelFlag::new()).unwrap();
        #[cfg(unix)]
        assert_eq!(verified.files, 3);
        #[cfg(not(unix))]
        assert_eq!(verified.files, 2);
        assert_eq!(verified.dirs, 2);
        assert_eq!(verified.bytes, 9); // "nested" (6) + "top" (3); symlinks count 0
        assert!(verify_transfer(&src, &dest_dir.join("other"), &CancelFlag::new()).is_err());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn verify_transfer_detects_missing_destination() {
        let dir = scratch("verify-missing");
        let src = dir.join("a.txt");
        write(&src, b"data");

        let err = verify_transfer(&src, &dir.join("gone.txt"), &CancelFlag::new()).unwrap_err();
        assert!(
            matches!(err, OpError::InvalidTarget(ref message) if message.contains("存在しません")),
            "unexpected error: {err:?}"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn verify_transfer_detects_size_mismatch() {
        let dir = scratch("verify-size");
        let src = dir.join("a.bin");
        let dest = dir.join("b.bin");
        write(&src, b"12345");
        write(&dest, b"123");

        let err = verify_transfer(&src, &dest, &CancelFlag::new()).unwrap_err();
        assert!(
            matches!(err, OpError::InvalidTarget(ref message) if message.contains("サイズ")),
            "unexpected error: {err:?}"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn verify_transfer_detects_child_mismatch() {
        let dir = scratch("verify-children");
        let src = dir.join("src");
        let dest = dir.join("dest");
        write(&src.join("a.txt"), b"a");
        write(&dest.join("b.txt"), b"a");

        let err = verify_transfer(&src, &dest, &CancelFlag::new()).unwrap_err();
        assert!(
            matches!(err, OpError::InvalidTarget(ref message) if message.contains("子要素")),
            "unexpected error: {err:?}"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn verify_transfer_detects_symlink_target_mismatch() {
        // Both sides are symlinks, so verification must compare read_link()
        // targets instead of following either link.
        let dir = scratch("verify-link-target");
        let src = dir.join("src");
        let dest = dir.join("dest");
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&dest).unwrap();
        std::os::unix::fs::symlink("target-a", src.join("link")).unwrap();
        std::os::unix::fs::symlink("target-b", dest.join("link")).unwrap();

        let err = verify_transfer(&src, &dest, &CancelFlag::new()).unwrap_err();
        assert!(
            matches!(err, OpError::InvalidTarget(ref message) if message.contains("リンク先")),
            "unexpected error: {err:?}"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn verify_transfer_detects_kind_mismatch() {
        let dir = scratch("verify-kind");
        let src_dir = dir.join("src-dir");
        let dest_file = dir.join("dest-file");
        fs::create_dir(&src_dir).unwrap();
        write(&dest_file, b"x");
        let err = verify_transfer(&src_dir, &dest_file, &CancelFlag::new()).unwrap_err();
        assert!(
            matches!(err, OpError::InvalidTarget(ref message) if message.contains("種類")),
            "unexpected error: {err:?}"
        );

        #[cfg(unix)]
        {
            // A symlink on one side and a regular file on the other is also a
            // type mismatch; symlink_metadata must not follow the link.
            let src_link = dir.join("src-link");
            let dest_plain = dir.join("dest-plain");
            std::os::unix::fs::symlink("nowhere", &src_link).unwrap();
            write(&dest_plain, b"x");
            let err = verify_transfer(&src_link, &dest_plain, &CancelFlag::new()).unwrap_err();
            assert!(
                matches!(err, OpError::InvalidTarget(ref message) if message.contains("種類")),
                "unexpected error: {err:?}"
            );
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn verify_transfer_honours_a_cancelled_flag_before_touching_paths() {
        // Cancellation is checked before symlink_metadata, so even paths that
        // do not exist must report Cancelled rather than an I/O error.
        let dir = scratch("verify-cancel");
        let cancel = CancelFlag::new();
        cancel.cancel();
        for pair in [
            (dir.join("a.txt"), dir.join("b.txt")),
            (dir.join("missing-src"), dir.join("missing-dest")),
        ] {
            let err = verify_transfer(&pair.0, &pair.1, &cancel).unwrap_err();
            assert!(
                matches!(err, OpError::Cancelled),
                "unexpected error: {err:?}"
            );
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn batch_rename_renames_all_entries() {
        let dir = scratch("batch-rename");
        write(&dir.join("a.txt"), b"a");
        write(&dir.join("b.txt"), b"b");

        let done = batch_rename(&[
            (dir.join("a.txt"), "one.txt".to_string()),
            (dir.join("b.txt"), "two.txt".to_string()),
        ])
        .unwrap();
        assert_eq!(done, 2);
        assert!(!dir.join("a.txt").exists());
        assert!(!dir.join("b.txt").exists());
        assert_eq!(fs::read(dir.join("one.txt")).unwrap(), b"a");
        assert_eq!(fs::read(dir.join("two.txt")).unwrap(), b"b");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn batch_rename_stops_at_first_error_and_keeps_earlier_renames() {
        let dir = scratch("batch-rename-partial");
        write(&dir.join("a.txt"), b"a");
        write(&dir.join("b.txt"), b"b");
        write(&dir.join("taken.txt"), b"taken");

        let err = batch_rename(&[
            (dir.join("a.txt"), "one.txt".to_string()),
            (dir.join("b.txt"), "taken.txt".to_string()),
        ])
        .unwrap_err();
        assert!(matches!(err, OpError::AlreadyExists(_)));
        assert_eq!(fs::read(dir.join("one.txt")).unwrap(), b"a");
        assert!(dir.join("b.txt").exists());

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

    /// Destination on a second filesystem for the EXDEV fallback tests.
    /// `/dev/shm` is a tmpfs on Linux. Return `None` with a clear message when
    /// it is missing/unwritable or shares a device with `/tmp`, so the tests
    /// stay meaningful in restricted containers.
    #[cfg(target_os = "linux")]
    fn exdev_shm_dest_dir(tag: &str) -> Option<PathBuf> {
        use std::os::unix::fs::MetadataExt;

        let root = Path::new("/dev/shm");
        if !root.is_dir() {
            eprintln!("skip: /dev/shm is not a directory; EXDEV fallback not exercised");
            return None;
        }
        let dir = root.join(format!(
            "gpui-fe-exdev-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        if fs::create_dir(&dir).is_err() {
            eprintln!("skip: /dev/shm is not writable; EXDEV fallback not exercised");
            return None;
        }
        let device_of = |path: &Path| fs::metadata(path).ok().map(|meta| meta.dev());
        if device_of(&dir) == device_of(&std::env::temp_dir()) {
            eprintln!("skip: /tmp and /dev/shm share a device; EXDEV fallback not exercised");
            let _ = fs::remove_dir(&dir);
            return None;
        }
        Some(dir)
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn move_entry_crosses_devices_via_copy_fallback() {
        let Some(dest_dir) = exdev_shm_dest_dir("move-ok") else {
            return;
        };
        let src_dir = scratch("exdev-src-ok");
        let src = src_dir.join("payload.txt");
        write(&src, b"cross-device payload");

        let stats = move_entry(
            &src,
            &dest_dir,
            ConflictPolicy::Fail,
            &CancelFlag::new(),
            |_| {},
        )
        .expect("cross-device move should succeed via copy-then-delete");

        assert_eq!(stats.files, 1);
        assert!(
            !src.exists(),
            "source must be deleted after a successful cross-device move"
        );
        assert_eq!(
            fs::read(dest_dir.join("payload.txt")).unwrap(),
            b"cross-device payload"
        );

        let _ = fs::remove_dir_all(&src_dir);
        let _ = fs::remove_dir_all(&dest_dir);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn move_entry_crosses_devices_copy_failure_keeps_source() {
        let Some(dest_dir) = exdev_shm_dest_dir("move-fail") else {
            return;
        };
        let src_dir = scratch("exdev-src-fail");
        let src = src_dir.join("payload.txt");
        write(&src, b"do not lose me");

        // Cancel before the fallback copy starts so `copy_recursive` fails.
        // The source is only deleted after a complete copy and verification,
        // so it must survive this failure.
        let cancel = CancelFlag::new();
        cancel.cancel();
        let err = move_entry(&src, &dest_dir, ConflictPolicy::Fail, &cancel, |_| {}).unwrap_err();
        assert!(matches!(err, OpError::Cancelled));
        assert_eq!(fs::read(&src).unwrap(), b"do not lose me");
        assert!(
            fs::read_dir(&dest_dir).unwrap().next().is_none(),
            "a failed fallback copy must not leave a destination behind"
        );

        let _ = fs::remove_dir_all(&src_dir);
        let _ = fs::remove_dir_all(&dest_dir);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn move_entry_crosses_devices_source_removal_failure_rolls_back_dest() {
        use std::os::unix::fs::PermissionsExt;

        let Some(dest_dir) = exdev_shm_dest_dir("move-rollback") else {
            return;
        };
        let src_dir = scratch("exdev-src-rollback");
        let src = src_dir.join("payload.txt");
        write(&src, b"keep the source only");

        // Removing a file needs write permission on its parent directory.
        // A read-only source directory forces exactly the post-copy
        // `remove_entry_safe(src)` failure this regression targets.
        struct RestoreMode(PathBuf, u32);
        impl Drop for RestoreMode {
            fn drop(&mut self) {
                let _ = fs::set_permissions(&self.0, fs::Permissions::from_mode(self.1));
            }
        }
        let original_mode = fs::metadata(&src_dir).unwrap().permissions().mode();
        fs::set_permissions(&src_dir, fs::Permissions::from_mode(0o555)).unwrap();
        let restore_mode = RestoreMode(src_dir.clone(), original_mode);

        let err = move_entry(
            &src,
            &dest_dir,
            ConflictPolicy::Fail,
            &CancelFlag::new(),
            |_| {},
        )
        .expect_err("source removal must fail while its parent is read-only");
        assert!(
            err.to_string().contains("元を削除できませんでした"),
            "the original error must survive the rollback: {err}"
        );
        assert_eq!(fs::read(&src).unwrap(), b"keep the source only");
        assert!(
            fs::read_dir(&dest_dir).unwrap().next().is_none(),
            "a failed source removal must not leave the copied destination behind"
        );

        // Retrying after the source is writable again must reuse the same
        // destination name: `payload (1).txt`, `(2)` … must not accumulate.
        drop(restore_mode);
        let stats = move_entry(
            &src,
            &dest_dir,
            ConflictPolicy::Fail,
            &CancelFlag::new(),
            |_| {},
        )
        .expect("retry should succeed once the source can be removed");
        assert_eq!(stats.created, vec![dest_dir.join("payload.txt")]);
        let names: Vec<_> = fs::read_dir(&dest_dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(
            names.len(),
            1,
            "retry must not leave a duplicate: {names:?}"
        );
        assert_eq!(names[0], "payload.txt");

        let _ = fs::remove_dir_all(&src_dir);
        let _ = fs::remove_dir_all(&dest_dir);
    }

    #[cfg(unix)]
    #[test]
    fn source_in_no_search_permission_dir_reports_permission_denied() {
        use std::os::unix::fs::PermissionsExt;

        // Restore mode on drop so a failed assertion never leaves an
        // un-removable scratch directory behind.
        struct RestoreMode(PathBuf, u32);
        impl Drop for RestoreMode {
            fn drop(&mut self) {
                let _ = fs::set_permissions(&self.0, fs::Permissions::from_mode(self.1));
            }
        }

        let dir = scratch("eacces-source");
        let locked = dir.join("locked");
        fs::create_dir(&locked).unwrap();
        let src = locked.join("secret.txt");
        write(&src, b"secret");
        let dest_dir = dir.join("dest");
        fs::create_dir(&dest_dir).unwrap();

        let original_mode = fs::metadata(&locked).unwrap().permissions().mode();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
        let _restore = RestoreMode(locked.clone(), original_mode);

        // `Path::exists()` used to turn the EACCES into `false`, so all three
        // entry points reported `NotFound`. Each must now keep PermissionDenied.
        let copied = copy_entry(
            &src,
            &dest_dir,
            ConflictPolicy::Fail,
            &CancelFlag::new(),
            |_| {},
        );
        assert!(
            matches!(
                copied,
                Err(OpError::PermissionDenied(_) | OpError::PermissionDeniedCause(_, _))
            ),
            "copy_entry must report PermissionDenied, got {copied:?}"
        );

        let moved = move_entry(
            &src,
            &dest_dir,
            ConflictPolicy::Fail,
            &CancelFlag::new(),
            |_| {},
        );
        assert!(
            matches!(
                moved,
                Err(OpError::PermissionDenied(_) | OpError::PermissionDeniedCause(_, _))
            ),
            "move_entry must report PermissionDenied, got {moved:?}"
        );

        let renamed = rename_entry(&src, "renamed.txt");
        assert!(
            matches!(
                renamed,
                Err(OpError::PermissionDenied(_) | OpError::PermissionDeniedCause(_, _))
            ),
            "rename_entry must report PermissionDenied, got {renamed:?}"
        );

        drop(_restore);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn copy_file_with_name_near_name_max_succeeds() {
        let dir = scratch("long-name");
        // NAME_MAX is 255 on the target filesystems; 240 bytes is legal for
        // the destination but used to overflow `.{dest}.partial-*` (280 bytes).
        let name = format!("{}.txt", "a".repeat(236));
        assert_eq!(name.len(), 240);
        let src = dir.join(&name);
        write(&src, b"payload");
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
        assert_eq!(stats.bytes, 7);
        assert_eq!(fs::read(dest_dir.join(&name)).unwrap(), b"payload");
        let partials: Vec<_> = fs::read_dir(&dest_dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().starts_with(".partial-"))
            .collect();
        assert!(partials.is_empty(), "temp file left behind: {partials:?}");

        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn require_directory_follows_a_dest_symlink_and_reports_a_dangling_one() {
        use std::error::Error as _;
        use std::os::unix::fs::symlink;

        let dir = scratch("dest-symlink");
        let real = dir.join("real-dest");
        fs::create_dir(&real).unwrap();
        let link = dir.join("link-dest");
        symlink(&real, &link).unwrap();

        // `require_directory` sees a symlink via `symlink_metadata`, then calls
        // `fs::metadata` to resolve it. A link to a real directory is a valid
        // destination for both operations.
        let copy_src = dir.join("copy.txt");
        write(&copy_src, b"copied");
        let copied = copy_entry(
            &copy_src,
            &link,
            ConflictPolicy::Fail,
            &CancelFlag::new(),
            |_| {},
        )
        .expect("copy into a symlinked directory must succeed");
        assert_eq!(copied.files, 1);
        assert_eq!(fs::read(real.join("copy.txt")).unwrap(), b"copied");

        let move_src = dir.join("move.txt");
        write(&move_src, b"moved");
        let moved = move_entry(
            &move_src,
            &link,
            ConflictPolicy::Fail,
            &CancelFlag::new(),
            |_| {},
        )
        .expect("move into a symlinked directory must succeed");
        assert_eq!(moved.created, vec![link.join("move.txt")]);
        assert!(!move_src.exists());
        assert_eq!(fs::read(real.join("move.txt")).unwrap(), b"moved");

        // A dangling link's own inode is stat-able, but the following
        // `fs::metadata` fails with ENOENT. That failure must go through
        // `io_error` and keep its `io::Error` cause instead of collapsing into
        // a plain InvalidTarget (the status bar relies on the category).
        let dangling = dir.join("dangling-dest");
        symlink(dir.join("no-such-dir"), &dangling).unwrap();
        let copy_src = dir.join("copy-dangling.txt");
        write(&copy_src, b"copy");
        let err = copy_entry(
            &copy_src,
            &dangling,
            ConflictPolicy::Fail,
            &CancelFlag::new(),
            |_| {},
        )
        .expect_err("a dangling dest_dir symlink must be rejected");
        assert!(
            matches!(err, OpError::NotFoundCause(_, _)),
            "expected an io_error-derived NotFound, got {err:?}"
        );
        let io = err
            .source()
            .and_then(|cause| cause.downcast_ref::<std::io::Error>())
            .expect("io_error() must preserve the io::Error cause");
        assert_eq!(io.kind(), std::io::ErrorKind::NotFound);
        assert!(copy_src.exists(), "a failed copy must keep the source");

        let move_src = dir.join("move-dangling.txt");
        write(&move_src, b"move");
        let err = move_entry(
            &move_src,
            &dangling,
            ConflictPolicy::Fail,
            &CancelFlag::new(),
            |_| {},
        )
        .expect_err("a dangling dest_dir symlink must be rejected by move too");
        assert!(
            matches!(err, OpError::NotFoundCause(_, _)),
            "expected an io_error-derived NotFound, got {err:?}"
        );
        let io = err
            .source()
            .and_then(|cause| cause.downcast_ref::<std::io::Error>())
            .expect("io_error() must preserve the io::Error cause");
        assert_eq!(io.kind(), std::io::ErrorKind::NotFound);
        assert!(move_src.exists(), "a failed move must keep the source");

        let _ = fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod dir_stats_tests {
    use super::{dir_stats, io_error, unique_path, unique_path_with_limit, CancelFlag, OpError};
    use crate::error::UNKNOWN_ERROR_REASON;
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
        assert!(matches!(
            not_found,
            OpError::NotFound(_) | OpError::NotFoundCause(_, _)
        ));
        assert!(std::error::Error::source(&not_found).is_some());

        let denied = io_error(
            path,
            std::io::Error::from(std::io::ErrorKind::PermissionDenied),
        );
        assert!(matches!(
            denied,
            OpError::PermissionDenied(_) | OpError::PermissionDeniedCause(_, _)
        ));

        let exists = io_error(
            path,
            std::io::Error::from(std::io::ErrorKind::AlreadyExists),
        );
        assert!(matches!(
            exists,
            OpError::AlreadyExists(_) | OpError::AlreadyExistsCause(_, _)
        ));

        let cross = io_error(
            path,
            std::io::Error::from(std::io::ErrorKind::CrossesDevices),
        );
        assert!(matches!(
            cross,
            OpError::Unsupported(_) | OpError::UnsupportedCause(_, _)
        ));

        let is_dir = io_error(path, std::io::Error::from(std::io::ErrorKind::IsADirectory));
        assert!(matches!(
            is_dir,
            OpError::InvalidTarget(_) | OpError::InvalidTargetCause(_, _)
        ));

        let other = io_error(path, std::io::Error::other("boom"));
        assert!(matches!(
            other,
            OpError::Io { .. } | OpError::IoCause { .. }
        ));
        assert_eq!(
            std::error::Error::source(&other).map(ToString::to_string),
            Some("boom".to_string())
        );
        // AV11: Other must read like FsError::from_io's Unknown kind, with the
        // original reason kept in the cause rather than dropped.
        assert!(
            other.to_string().contains(UNKNOWN_ERROR_REASON),
            "Display: {other}"
        );
        assert_eq!(
            other.message_ja(),
            format!("/tmp/example: {UNKNOWN_ERROR_REASON} (boom)")
        );

        // Other ErrorKinds keep the concrete io text as the status message.
        let invalid = io_error(path, std::io::Error::from(std::io::ErrorKind::InvalidInput));
        assert!(invalid.message_ja().starts_with("/tmp/example: "));
        assert!(!invalid.message_ja().contains(UNKNOWN_ERROR_REASON));
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

    #[test]
    fn unique_path_exhaustion_returns_the_occupied_original_path() {
        // Contract of the practically-unreachable fallback: after every
        // numbered candidate is taken, `unique_path` returns the occupied
        // original path so the caller fails with AlreadyExists. An explicit
        // small bound exercises the real fallback without 10,001 files.
        let dir = scratch("unique-exhaustion");
        let name = std::ffi::OsStr::new("taken.txt");
        write(&dir.join("taken.txt"), 1);
        write(&dir.join("taken (1).txt"), 1);
        write(&dir.join("taken (2).txt"), 1);

        let exhausted = unique_path_with_limit(&dir, name, 2);
        assert_eq!(
            exhausted,
            dir.join("taken.txt"),
            "exhaustion must fall back to the original occupied path"
        );
        assert!(exhausted.exists());

        // Respecting the bound: one more slot stays free at limit 3, and the
        // public entry point still uses the full 10,000-candidate ladder.
        let available = unique_path_with_limit(&dir, name, 3);
        assert_eq!(available, dir.join("taken (3).txt"));
        assert!(!available.exists());
        assert_eq!(
            unique_path(&dir, name),
            dir.join("taken (3).txt"),
            "the public helper must use the production bound"
        );

        let _ = fs::remove_dir_all(&dir);
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
