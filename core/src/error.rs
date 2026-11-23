// SPDX-License-Identifier: AGPL-3.0-or-later
//! Common application error shared by the filesystem model and the operation
//! engine.
//!
//! `FsError` (directory scanning) and `OpError` (copy/move/rename/delete) used
//! to be separate types with different `Display` implementations. Both are now
//! aliases of [`AppError`]: callers keep writing `FsError::from_io(..)` or
//! `OpError::Cancelled`, while every failure shares one `Display`, one
//! `message_ja()` and one [`std::error::Error::source()`] implementation.
//!
//! Variants ending in `Cause` retain the underlying failure as a
//! [`BoxedCause`], returned by `source()`. The plain variants are kept so a
//! caller can build an error without a cause and so existing pattern matches
//! keep working; both forms render identically.
//!
//! No new dependency is used: `Display` and `Error` are implemented by hand.

use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

/// Category of a filesystem error, independent of `std::io::ErrorKind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FsErrorKind {
    NotFound,
    PermissionDenied,
    NotADirectory,
    /// A directory was used where a file was expected (Linux `EISDIR`).
    IsADirectory,
    /// Write attempted on a read-only filesystem (Linux `EROFS`).
    ReadOnlyFilesystem,
    /// No space left on the device (Linux `ENOSPC`).
    StorageFull,
    Io,
    Unknown,
}

/// Japanese reason used when an `io::Error` has no more specific category
/// (`ErrorKind::Other`). Shared with `ops::io_error` so filesystem scans and
/// operations render the same kind with the same wording.
pub const UNKNOWN_ERROR_REASON: &str = "エラーが発生しました";

/// An error that carries text only, used when a boxed cause has to be cloned or
/// the original failure cannot be moved into [`BoxedCause::new`].
#[derive(Debug)]
struct TextCause(String);

impl fmt::Display for TextCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for TextCause {}

/// The boxed underlying failure behind [`AppError::source()`].
#[derive(Debug)]
pub struct BoxedCause(Box<dyn Error + Send + Sync>);

impl BoxedCause {
    /// Box a concrete error while keeping its type reachable through
    /// [`Error::source()`].
    pub fn new<E>(error: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        Self(Box::new(error))
    }

    /// Box an error-like value that cannot be moved into [`Self::new`], keeping
    /// its rendered text as the cause.
    pub fn text(message: impl Into<String>) -> Self {
        Self(Box::new(TextCause(message.into())))
    }

    /// View the cause as a standard error reference.
    pub fn as_error(&self) -> &(dyn Error + 'static) {
        self.0.as_ref()
    }

    /// Raw OS error code when the cause is an `io::Error` carrying one.
    ///
    /// Used by [`AppError::raw_os_error`] so every cause-carrying variant can
    /// expose errno through one API, regardless of which variant produced it.
    pub fn raw_os_error(&self) -> Option<i32> {
        self.as_error()
            .downcast_ref::<std::io::Error>()
            .and_then(std::io::Error::raw_os_error)
    }

    /// Cause text appended to a Japanese status message.
    ///
    /// Returns `None` for an empty cause so a clone or a reconstructed error
    /// never renders a stray pair of parentheses.
    pub fn detail_text(&self) -> Option<String> {
        let text = self.0.to_string();
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    }
}

impl Clone for BoxedCause {
    /// A cloned cause cannot clone the boxed trait object. When the cause is an
    /// `io::Error` with an OS error code, rebuild it with
    /// [`std::io::Error::from_raw_os_error`] so callers can still read errno
    /// through [`Self::raw_os_error`]; otherwise keep the rendered text as the
    /// original fallback. The chain and the message survive either way.
    fn clone(&self) -> Self {
        if let Some(code) = self.raw_os_error() {
            return Self::new(std::io::Error::from_raw_os_error(code));
        }
        Self::text(self.0.to_string())
    }
}

impl fmt::Display for BoxedCause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Error for BoxedCause {}

/// A failure from any filesystem layer of the application.
#[derive(Clone, Debug)]
pub enum AppError {
    // -- filesystem scan (formerly `FsError`) --
    /// Scan failure with a stable category, without a retained cause.
    Fs {
        kind: FsErrorKind,
        path: PathBuf,
        os_message: Option<String>,
    },
    /// Scan failure retaining the underlying OS error as its cause.
    FsCause {
        kind: FsErrorKind,
        path: PathBuf,
        os_message: Option<String>,
        /// Raw OS error code as reported by `io::Error::raw_os_error`.
        /// Kept separately from `cause` so callers can branch on it without
        /// downcasting the boxed trait object.
        raw_os_error: Option<i32>,
        cause: BoxedCause,
    },

    // -- operations (formerly `OpError`, plain variants kept for API compat) --
    NotFound(PathBuf),
    PermissionDenied(PathBuf),
    AlreadyExists(PathBuf),
    InvalidTarget(String),
    Unsupported(String),
    Cancelled,
    Io {
        path: PathBuf,
        message: String,
    },

    // -- cause-carrying counterparts of the operation variants --
    NotFoundCause(PathBuf, BoxedCause),
    PermissionDeniedCause(PathBuf, BoxedCause),
    AlreadyExistsCause(PathBuf, BoxedCause),
    InvalidTargetCause(String, BoxedCause),
    UnsupportedCause(String, BoxedCause),
    IoCause {
        path: PathBuf,
        message: String,
        cause: BoxedCause,
    },
}

/// Linux errno values for the three kinds that used to be flattened into
/// [`FsErrorKind::Io`]. The mapping is explicit so it cannot drift with the
/// standard library's `ErrorKind` coverage; non-Linux hosts keep the generic
/// classification.
#[cfg(target_os = "linux")]
fn raw_os_error_kind(code: Option<i32>) -> Option<FsErrorKind> {
    match code {
        Some(21) => Some(FsErrorKind::IsADirectory), // EISDIR
        Some(30) => Some(FsErrorKind::ReadOnlyFilesystem), // EROFS
        Some(28) => Some(FsErrorKind::StorageFull),  // ENOSPC
        _ => None,
    }
}

#[cfg(not(target_os = "linux"))]
fn raw_os_error_kind(_code: Option<i32>) -> Option<FsErrorKind> {
    None
}

impl AppError {
    /// Map an `io::Error` to a stable category (legacy `FsError::from_io`).
    ///
    /// Both the category and the raw OS code are retained: `kind()` drives the
    /// Japanese message while [`Self::fs_raw_os_error`] exposes the original
    /// errno for callers that need to distinguish otherwise identical kinds.
    pub fn from_io(path: PathBuf, err: &std::io::Error) -> Self {
        use std::io::ErrorKind;
        let raw = err.raw_os_error();
        let mut kind = match err.kind() {
            ErrorKind::NotFound => FsErrorKind::NotFound,
            ErrorKind::PermissionDenied => FsErrorKind::PermissionDenied,
            ErrorKind::NotADirectory => FsErrorKind::NotADirectory,
            ErrorKind::IsADirectory => FsErrorKind::IsADirectory,
            ErrorKind::ReadOnlyFilesystem => FsErrorKind::ReadOnlyFilesystem,
            ErrorKind::StorageFull => FsErrorKind::StorageFull,
            ErrorKind::Other => FsErrorKind::Unknown,
            _ => FsErrorKind::Io,
        };
        if matches!(kind, FsErrorKind::Io | FsErrorKind::Unknown) {
            if let Some(specific) = raw_os_error_kind(raw) {
                kind = specific;
            }
        }
        // Build `FsCause` directly rather than going through `with_cause`: a
        // plain `Fs` has no raw-code slot, so the code would be lost on the way.
        Self::FsCause {
            kind,
            path,
            os_message: Some(err.to_string()),
            raw_os_error: raw,
            cause: BoxedCause::new(std::io::Error::new(err.kind(), err.to_string())),
        }
    }

    /// OS 固有のエラー番号（Linux の `EISDIR` = 21 等）。
    ///
    /// `from_io` が保持した値のみを返す。`BoxedCause` を downcast せずに
    /// errno で分岐できるようにするための追加 API で、既存 variant の
    /// フィールドや `source()` の形は変更しない。
    pub fn fs_raw_os_error(&self) -> Option<i32> {
        match self {
            Self::FsCause { raw_os_error, .. } => *raw_os_error,
            _ => None,
        }
    }

    /// Raw OS error code of any cause-carrying failure.
    ///
    /// `fs_raw_os_error()` only sees the dedicated slot on `FsCause`; operation
    /// errors instead keep the original `io::Error` inside their
    /// [`BoxedCause`]. This accessor unifies both paths (and `PortError`'s
    /// equivalent) so a caller needs one method to branch on errno. `FsCause`
    /// prefers its slot and falls back to the boxed cause for one built with
    /// `with_cause`; plain variants have no cause and return `None`.
    pub fn raw_os_error(&self) -> Option<i32> {
        match self {
            Self::FsCause {
                raw_os_error,
                cause,
                ..
            } => raw_os_error.or_else(|| cause.raw_os_error()),
            Self::NotFoundCause(_, cause)
            | Self::PermissionDeniedCause(_, cause)
            | Self::AlreadyExistsCause(_, cause)
            | Self::InvalidTargetCause(_, cause)
            | Self::UnsupportedCause(_, cause)
            | Self::IoCause { cause, .. } => cause.raw_os_error(),
            _ => None,
        }
    }

    /// Attach the underlying failure to a plain variant, keeping its rendered
    /// text unchanged. Already-caused variants are returned as-is.
    pub fn with_cause(self, cause: BoxedCause) -> Self {
        match self {
            Self::Fs {
                kind,
                path,
                os_message,
            } => Self::FsCause {
                kind,
                path,
                os_message,
                // A plain `Fs` never carried a raw code; `from_io` builds
                // `FsCause` directly so its code is preserved.
                raw_os_error: None,
                cause,
            },
            Self::NotFound(path) => Self::NotFoundCause(path, cause),
            Self::PermissionDenied(path) => Self::PermissionDeniedCause(path, cause),
            Self::AlreadyExists(path) => Self::AlreadyExistsCause(path, cause),
            Self::InvalidTarget(reason) => Self::InvalidTargetCause(reason, cause),
            Self::Unsupported(reason) => Self::UnsupportedCause(reason, cause),
            Self::Io { path, message } => Self::IoCause {
                path,
                message,
                cause,
            },
            other => other,
        }
    }

    /// Japanese message including the affected path (legacy
    /// `FsError::message_ja`). Failure-scan errors keep their `os_message` in
    /// parentheses; cause-carrying operation errors now do the same with the
    /// text held in their [`BoxedCause`], so the OS reason is visible in the
    /// status/log instead of being stranded in `source()`. `IoCause` already
    /// renders that text as its `message`, so it is not appended twice. Plain
    /// variants still render exactly as `Display`.
    pub fn message_ja(&self) -> String {
        match self {
            Self::Fs {
                kind,
                path,
                os_message,
            }
            | Self::FsCause {
                kind,
                path,
                os_message,
                ..
            } => Self::fs_message(*kind, path, os_message.as_deref()),
            Self::NotFoundCause(path, cause) => {
                Self::with_cause_detail(format!("見つかりません: {}", path.display()), cause, None)
            }
            Self::PermissionDeniedCause(path, cause) => Self::with_cause_detail(
                format!("アクセスが拒否されました: {}", path.display()),
                cause,
                None,
            ),
            Self::AlreadyExistsCause(path, cause) => {
                Self::with_cause_detail(format!("既に存在します: {}", path.display()), cause, None)
            }
            Self::InvalidTargetCause(reason, cause) => {
                Self::with_cause_detail(format!("操作できません: {reason}"), cause, None)
            }
            Self::UnsupportedCause(reason, cause) => {
                Self::with_cause_detail(format!("未対応の操作です: {reason}"), cause, None)
            }
            Self::IoCause {
                path,
                message,
                cause,
            } => Self::with_cause_detail(
                format!("{}: {message}", path.display()),
                cause,
                Some(message),
            ),
            other => other.to_string(),
        }
    }

    fn with_cause_detail(base: String, cause: &BoxedCause, already_shown: Option<&str>) -> String {
        match cause.detail_text() {
            Some(detail) if Some(detail.as_str()) != already_shown => {
                format!("{base} ({detail})")
            }
            _ => base,
        }
    }

    fn fs_message(kind: FsErrorKind, path: &Path, os_message: Option<&str>) -> String {
        let reason = match kind {
            FsErrorKind::NotFound => "見つかりません",
            FsErrorKind::PermissionDenied => "アクセスが拒否されました",
            FsErrorKind::NotADirectory => "ディレクトリではありません",
            FsErrorKind::IsADirectory => "ディレクトリです",
            FsErrorKind::ReadOnlyFilesystem => "読み取り専用のファイルシステムです",
            FsErrorKind::StorageFull => "空き領域がありません",
            FsErrorKind::Io => "読み取れませんでした",
            FsErrorKind::Unknown => UNKNOWN_ERROR_REASON,
        };
        match os_message {
            Some(message) if !message.is_empty() => {
                format!("{}: {reason} ({message})", path.display())
            }
            _ => format!("{}: {reason}", path.display()),
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fs { .. } | Self::FsCause { .. } => f.write_str(&self.message_ja()),
            Self::NotFound(path) | Self::NotFoundCause(path, _) => {
                write!(f, "見つかりません: {}", path.display())
            }
            Self::PermissionDenied(path) | Self::PermissionDeniedCause(path, _) => {
                write!(f, "アクセスが拒否されました: {}", path.display())
            }
            Self::AlreadyExists(path) | Self::AlreadyExistsCause(path, _) => {
                write!(f, "既に存在します: {}", path.display())
            }
            Self::InvalidTarget(reason) | Self::InvalidTargetCause(reason, _) => {
                write!(f, "操作できません: {reason}")
            }
            Self::Unsupported(reason) | Self::UnsupportedCause(reason, _) => {
                write!(f, "未対応の操作です: {reason}")
            }
            Self::Cancelled => write!(f, "操作はキャンセルされました"),
            Self::Io { path, message } | Self::IoCause { path, message, .. } => {
                write!(f, "{}: {message}", path.display())
            }
        }
    }
}

impl Error for AppError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::FsCause { cause, .. }
            | Self::NotFoundCause(_, cause)
            | Self::PermissionDeniedCause(_, cause)
            | Self::AlreadyExistsCause(_, cause)
            | Self::InvalidTargetCause(_, cause)
            | Self::UnsupportedCause(_, cause)
            | Self::IoCause { cause, .. } => Some(cause.as_error()),
            _ => None,
        }
    }
}

/// Test-only accessors used to assert the filesystem metadata without tying
/// tests to a single variant shape.
#[cfg(test)]
impl AppError {
    pub(crate) fn fs_kind(&self) -> Option<FsErrorKind> {
        match self {
            Self::Fs { kind, .. } | Self::FsCause { kind, .. } => Some(*kind),
            _ => None,
        }
    }

    pub(crate) fn fs_path(&self) -> Option<&Path> {
        match self {
            Self::Fs { path, .. } | Self::FsCause { path, .. } => Some(path),
            _ => None,
        }
    }

    pub(crate) fn fs_os_message(&self) -> Option<&str> {
        match self {
            Self::Fs { os_message, .. } | Self::FsCause { os_message, .. } => os_message.as_deref(),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AppError, BoxedCause, FsErrorKind};
    use std::error::Error;
    use std::io::ErrorKind;
    use std::path::{Path, PathBuf};

    #[test]
    fn fs_error_retains_io_source() {
        let io = std::io::Error::new(ErrorKind::PermissionDenied, "boom");
        let error = AppError::from_io(PathBuf::from("/tmp/x"), &io);
        assert_eq!(error.fs_kind(), Some(FsErrorKind::PermissionDenied));
        assert_eq!(error.fs_path(), Some(Path::new("/tmp/x")));
        assert_eq!(error.fs_os_message(), Some("boom"));
        let source = error.source().expect("io cause must be retained");
        assert_eq!(source.to_string(), "boom");
        assert!(error.message_ja().contains("アクセスが拒否されました"));
    }

    #[test]
    fn plain_and_cause_variants_share_display() {
        let path = PathBuf::from("/tmp/target.txt");
        let plain = AppError::AlreadyExists(path.clone());
        let caused = AppError::AlreadyExistsCause(path, BoxedCause::text("detail"));
        assert_eq!(plain.to_string(), caused.to_string());
        assert!(plain.source().is_none());
        assert_eq!(
            caused.source().map(ToString::to_string),
            Some("detail".to_string())
        );
    }

    #[test]
    fn remaining_cause_variants_expose_their_held_message() {
        // `source()` has seven cause-carrying arms. The tests above pin
        // FsCause / NotFoundCause / AlreadyExistsCause; this one covers the
        // other four so no arm can silently stop returning its cause.
        let cases: Vec<(AppError, &str)> = vec![
            (
                AppError::PermissionDeniedCause(
                    PathBuf::from("/tmp/locked"),
                    BoxedCause::text("permission detail"),
                ),
                "permission detail",
            ),
            (
                AppError::InvalidTargetCause(
                    "target is a symlink".to_string(),
                    BoxedCause::text("invalid target detail"),
                ),
                "invalid target detail",
            ),
            (
                AppError::UnsupportedCause(
                    "reflink".to_string(),
                    BoxedCause::text("unsupported detail"),
                ),
                "unsupported detail",
            ),
            (
                AppError::IoCause {
                    path: PathBuf::from("/tmp/io"),
                    message: "read failed".to_string(),
                    cause: BoxedCause::text("io detail"),
                },
                "io detail",
            ),
        ];
        for (error, expected) in cases {
            assert_eq!(
                error.source().map(ToString::to_string),
                Some(expected.to_string()),
                "cause of {error}"
            );
        }
    }

    #[test]
    fn cloning_a_caused_error_preserves_source_and_message_ja() {
        // FsCause: `from_io` attaches the OS error as a `BoxedCause`; cloning a
        // boxed trait object falls back to its rendered text, so source() text
        // and the Japanese message must survive the clone unchanged.
        let io = std::io::Error::new(ErrorKind::NotFound, "missing file");
        let original = AppError::from_io(PathBuf::from("/tmp/missing"), &io);
        let cloned = original.clone();
        assert!(matches!(cloned, AppError::FsCause { .. }));
        assert_eq!(cloned.source().expect("cause").to_string(), "missing file");
        assert_eq!(
            cloned.source().map(ToString::to_string),
            original.source().map(ToString::to_string)
        );
        assert_eq!(cloned.message_ja(), original.message_ja());
        assert!(cloned.message_ja().contains("見つかりません"));

        // NotFoundCause: the same guarantee for an operation error carrying a
        // cause, whose message_ja() is the plain Display text.
        let not_found = AppError::NotFoundCause(
            PathBuf::from("/tmp/gone"),
            BoxedCause::text("underlying detail"),
        );
        let cloned = not_found.clone();
        assert_eq!(
            cloned.source().expect("cause").to_string(),
            "underlying detail"
        );
        assert_eq!(
            cloned.source().map(ToString::to_string),
            not_found.source().map(ToString::to_string)
        );
        assert_eq!(cloned.message_ja(), not_found.message_ja());
        // AV10: a cause-carrying operation error now surfaces its OS/detail
        // text in the Japanese message, mirroring `FsCause`'s "(os text)".
        assert_eq!(
            cloned.message_ja(),
            "見つかりません: /tmp/gone (underlying detail)"
        );
    }

    #[test]
    fn with_cause_is_noop_for_already_caused_variants() {
        // FsCause already carries its source; a second with_cause must keep the
        // original cause (the `other => other` arm), not replace it.
        let io = std::io::Error::new(ErrorKind::NotFound, "original fs cause");
        let fs_cause = AppError::from_io(PathBuf::from("/tmp/x"), &io);
        let before = fs_cause.source().map(ToString::to_string);
        let after = fs_cause
            .with_cause(BoxedCause::text("replacement"))
            .source()
            .map(ToString::to_string);
        assert_eq!(after, before);
        assert_eq!(after, Some("original fs cause".to_string()));

        // The same no-op guarantee holds for a NotFoundCause operation error.
        let not_found = AppError::NotFoundCause(
            PathBuf::from("/tmp/gone"),
            BoxedCause::text("original detail"),
        );
        let before = not_found.source().map(ToString::to_string);
        let after = not_found
            .with_cause(BoxedCause::text("replacement"))
            .source()
            .map(ToString::to_string);
        assert_eq!(after, before);
        assert_eq!(after, Some("original detail".to_string()));
    }

    #[test]
    fn with_cause_is_noop_for_cancelled() {
        // `Cancelled` has no cause-carrying counterpart: the `other => other`
        // arm must leave it untouched (and keep source() empty), so a supplied
        // cause cannot turn a cancellation into a different error.
        let cancelled = AppError::Cancelled.with_cause(BoxedCause::text("ignored"));
        assert!(matches!(cancelled, AppError::Cancelled));
        assert!(cancelled.source().is_none());
        assert_eq!(cancelled.to_string(), "操作はキャンセルされました");
    }

    #[test]
    fn fs_message_omits_parenthetical_for_empty_os_message() {
        // `io::Error::other("")` carries an empty OS message: from_io stores
        // Some(""), and fs_message must fall through to the no-parenthesis arm.
        let error = AppError::from_io(PathBuf::from("/tmp/empty-os"), &std::io::Error::other(""));
        assert_eq!(error.fs_os_message(), Some(""));
        assert_eq!(error.fs_kind(), Some(FsErrorKind::Unknown));
        assert_eq!(
            error.message_ja(),
            "/tmp/empty-os: エラーが発生しました",
            "an empty OS message must not produce an empty '(…)' suffix"
        );

        // The same arm handles a genuinely absent OS message.
        let no_message = AppError::Fs {
            kind: FsErrorKind::Unknown,
            path: PathBuf::from("/tmp/no-os"),
            os_message: None,
        };
        assert_eq!(no_message.message_ja(), "/tmp/no-os: エラーが発生しました");
    }

    #[test]
    fn from_io_maps_not_a_directory_to_its_own_category_and_message() {
        let io = std::io::Error::new(ErrorKind::NotADirectory, "x");
        let error = AppError::from_io(PathBuf::from("/tmp/not-a-dir"), &io);
        assert_eq!(error.fs_kind(), Some(FsErrorKind::NotADirectory));
        assert!(error.message_ja().contains("ディレクトリではありません"));
    }

    #[test]
    fn from_io_maps_other_kinds_to_io_and_message() {
        let io = std::io::Error::new(ErrorKind::InvalidInput, "x");
        let error = AppError::from_io(PathBuf::from("/tmp/invalid-input"), &io);
        assert_eq!(error.fs_kind(), Some(FsErrorKind::Io));
        assert!(error.message_ja().contains("読み取れませんでした"));
    }

    #[cfg(unix)]
    #[test]
    fn from_io_keeps_raw_os_error() {
        // AR07: the reconstructed cause only keeps kind+text, so without a
        // dedicated slot the original errno was lost. ENOENT = 2 on Unix.
        let io = std::io::Error::from_raw_os_error(2);
        let error = AppError::from_io(PathBuf::from("/tmp/missing"), &io);
        assert_eq!(error.fs_raw_os_error(), Some(2));
        assert_eq!(error.fs_kind(), Some(FsErrorKind::NotFound));

        // Errors built without `from_io` have no OS code to expose.
        let plain = AppError::NotFound(PathBuf::from("/tmp/gone"));
        assert_eq!(plain.fs_raw_os_error(), None);
        let fs_plain = AppError::Fs {
            kind: FsErrorKind::Io,
            path: PathBuf::from("/tmp/x"),
            os_message: None,
        };
        assert_eq!(fs_plain.fs_raw_os_error(), None);
        assert_eq!(
            fs_plain
                .with_cause(BoxedCause::text("detail"))
                .fs_raw_os_error(),
            None
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn from_io_maps_is_a_directory_read_only_and_storage_full() {
        // AR08: these errno values used to be flattened into `Io` ("読み取れませんでした").
        for (code, expected, fragment) in [
            (21, FsErrorKind::IsADirectory, "ディレクトリです"),
            (
                30,
                FsErrorKind::ReadOnlyFilesystem,
                "読み取り専用のファイルシステムです",
            ),
            (28, FsErrorKind::StorageFull, "空き領域がありません"),
        ] {
            let io = std::io::Error::from_raw_os_error(code);
            let error = AppError::from_io(PathBuf::from("/tmp/x"), &io);
            assert_eq!(error.fs_kind(), Some(expected), "errno {code}");
            assert_eq!(error.fs_raw_os_error(), Some(code));
            assert!(
                error.message_ja().contains(fragment),
                "errno {code}: {}",
                error.message_ja()
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn all_cause_variants_expose_raw_os_error() {
        // AV04: one accessor must work for every cause-carrying variant, not
        // just FsCause's dedicated slot.
        let code = 2; // ENOENT
        let io = || std::io::Error::from_raw_os_error(code);
        let cases: Vec<AppError> = vec![
            AppError::NotFoundCause(PathBuf::from("/tmp/x"), BoxedCause::new(io())),
            AppError::PermissionDeniedCause(PathBuf::from("/tmp/x"), BoxedCause::new(io())),
            AppError::AlreadyExistsCause(PathBuf::from("/tmp/x"), BoxedCause::new(io())),
            AppError::InvalidTargetCause("理由".to_string(), BoxedCause::new(io())),
            AppError::UnsupportedCause("理由".to_string(), BoxedCause::new(io())),
            AppError::IoCause {
                path: PathBuf::from("/tmp/x"),
                message: io().to_string(),
                cause: BoxedCause::new(io()),
            },
        ];
        for error in cases {
            assert_eq!(error.raw_os_error(), Some(code), "variant {error:?}");
        }

        // FsCause keeps the code in its slot even though its boxed cause is
        // reconstructed from kind+text.
        let fs = AppError::from_io(PathBuf::from("/tmp/x"), &io());
        assert_eq!(fs.raw_os_error(), Some(code));
        assert_eq!(fs.raw_os_error(), fs.fs_raw_os_error());

        // Plain variants have no cause and therefore no errno.
        assert_eq!(AppError::Cancelled.raw_os_error(), None);
        assert_eq!(
            AppError::NotFound(PathBuf::from("/tmp/x")).raw_os_error(),
            None
        );
    }

    #[cfg(unix)]
    #[test]
    fn fs_cause_slot_wins_over_the_boxed_errno() {
        // AW18: `FsCause` has two errno sources. The dedicated slot must win;
        // the boxed cause is only the fallback for a slot-less value.
        let boxed_code = 13; // EACCES
        let with_slot = AppError::FsCause {
            kind: FsErrorKind::Io,
            path: PathBuf::from("/tmp/slot"),
            os_message: None,
            raw_os_error: Some(2), // ENOENT, deliberately different
            cause: BoxedCause::new(std::io::Error::from_raw_os_error(boxed_code)),
        };
        assert_eq!(with_slot.raw_os_error(), Some(2));
        assert_eq!(with_slot.fs_raw_os_error(), Some(2));

        let slotless = AppError::FsCause {
            kind: FsErrorKind::Io,
            path: PathBuf::from("/tmp/no-slot"),
            os_message: None,
            raw_os_error: None,
            cause: BoxedCause::new(std::io::Error::from_raw_os_error(boxed_code)),
        };
        assert_eq!(slotless.raw_os_error(), Some(boxed_code));
        assert_eq!(slotless.fs_raw_os_error(), None);
    }

    #[cfg(unix)]
    #[test]
    fn cloned_boxed_cause_keeps_raw_os_error() {
        // AV09: cloning a cause used to collapse it to text and lose errno.
        let io = std::io::Error::from_raw_os_error(2);
        let original = AppError::IoCause {
            path: PathBuf::from("/tmp/clone"),
            message: io.to_string(),
            cause: BoxedCause::new(io),
        };
        let cloned = original.clone();
        assert_eq!(original.raw_os_error(), Some(2));
        assert_eq!(cloned.raw_os_error(), Some(2), "clone must keep errno");
        // AV10: the message survives the clone; IoCause already renders the OS
        // text as its `message`, so it is not appended a second time.
        assert_eq!(cloned.message_ja(), original.message_ja());
        assert!(cloned.message_ja().starts_with("/tmp/clone: "));

        // Text-only causes keep the original rendered-text fallback.
        let text = AppError::NotFoundCause(
            PathBuf::from("/tmp/gone"),
            BoxedCause::text("underlying detail"),
        );
        let cloned = text.clone();
        assert_eq!(cloned.raw_os_error(), None);
        assert_eq!(
            cloned.source().map(ToString::to_string),
            Some("underlying detail".to_string())
        );
    }
}
