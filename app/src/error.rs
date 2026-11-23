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
    Io,
    Unknown,
}

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
}

impl Clone for BoxedCause {
    /// A cloned cause cannot clone the boxed trait object, so it keeps the
    /// original message: the chain survives, the concrete type does not.
    fn clone(&self) -> Self {
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

impl AppError {
    /// Map an `io::Error` to a stable category (legacy `FsError::from_io`).
    pub fn from_io(path: PathBuf, err: &std::io::Error) -> Self {
        use std::io::ErrorKind;
        let kind = match err.kind() {
            ErrorKind::NotFound => FsErrorKind::NotFound,
            ErrorKind::PermissionDenied => FsErrorKind::PermissionDenied,
            ErrorKind::NotADirectory => FsErrorKind::NotADirectory,
            ErrorKind::Other => FsErrorKind::Unknown,
            _ => FsErrorKind::Io,
        };
        Self::Fs {
            kind,
            path,
            os_message: Some(err.to_string()),
        }
        .with_cause(
            // `io::Error` is not Clone; reconstruct one with the same kind and
            // message so the cause stays reachable through `source()`.
            BoxedCause::new(std::io::Error::new(err.kind(), err.to_string())),
        )
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
    /// `FsError::message_ja`). For non-filesystem failures the status text is
    /// exactly `Display`, so existing UI wording is preserved.
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
            other => other.to_string(),
        }
    }

    fn fs_message(kind: FsErrorKind, path: &Path, os_message: Option<&str>) -> String {
        let reason = match kind {
            FsErrorKind::NotFound => "見つかりません",
            FsErrorKind::PermissionDenied => "アクセスが拒否されました",
            FsErrorKind::NotADirectory => "ディレクトリではありません",
            FsErrorKind::Io => "読み取れませんでした",
            FsErrorKind::Unknown => "エラーが発生しました",
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
        assert_eq!(cloned.message_ja(), "見つかりません: /tmp/gone");
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
}
