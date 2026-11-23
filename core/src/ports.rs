// SPDX-License-Identifier: AGPL-3.0-or-later
//! プラットフォーム境界のポート（トレイトとデータ型のみ）。
//!
//! GPUI も platform crate も依存しない。実装は各アダプタ側に置く:
//! - desktop: `app/src/ports_gpui.rs`
//! - Android: Jetpack Compose アプリ（JNI staticlib）
//! - PWA: Web フロントエンド（wasm コア + Web API）
//!
//! 同期メソッドのブロッキング契約は `trash_view.rs:46-47` を踏襲し、
//! バックグラウンド実行は呼び出し側（UI アダプタ）が担当する。

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

/// ポート呼び出しのエラー。std 由来の IO エラーを保持する。
#[derive(Debug)]
pub struct PortError {
    message: String,
    source: Option<io::Error>,
}

impl PortError {
    /// IO エラーを原因として持つエラーを構築する。
    pub fn io(message: impl Into<String>, source: io::Error) -> Self {
        Self {
            message: message.into(),
            source: Some(source),
        }
    }

    /// メッセージのみのエラーを構築する（未実装ポートの no-op など）。
    pub fn message(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            source: None,
        }
    }

    /// エラー理由。
    pub fn message_text(&self) -> &str {
        &self.message
    }

    /// 原因となった IO エラー（あれば）。
    pub fn io_error(&self) -> Option<&io::Error> {
        self.source.as_ref()
    }
}

impl std::fmt::Display for PortError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for PortError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_ref()
            .map(|e| e as &(dyn std::error::Error + 'static))
    }
}

/// ゴミ箱の一行。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrashEntry {
    /// プラットフォーム固有の ID（非 UTF-8 あり得るため OsString）。
    pub id: OsString,
    /// 表示名。
    pub name: String,
    /// 元の場所。
    pub original_path: PathBuf,
}

/// 復元結果。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RestoreReport {
    /// 復元できた件数。
    pub restored: usize,
    /// 見つからなかった ID の数。
    pub missing: usize,
}

/// ゴミ箱バックエンド。未対応プラットフォームは no-op を返す。
pub trait TrashBackend {
    /// ゴミ箱の一覧。
    fn list(&self) -> Result<Vec<TrashEntry>, PortError>;
    /// ID を指定して復元する。
    fn restore(&self, ids: &[OsString]) -> Result<RestoreReport, PortError>;
    /// 完全削除（P0 では UI に出さない）。
    fn delete(&self, path: &Path) -> Result<(), PortError>;
}

/// ボリューム情報。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VolumeInfo {
    /// 表示名（ドライブ文字 / ラベル）。
    pub name: String,
    /// マウント元（`/proc/mounts` の source 等）。
    pub source: String,
    /// ルートパス。
    pub path: PathBuf,
}

/// ボリューム列挙。
pub trait VolumeSource {
    /// 利用可能なボリューム一覧。
    fn volumes(&self) -> Vec<VolumeInfo>;
}

/// 画像情報（content-based 判定の結果）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageInfo {
    /// 検出されたフォーマット名（例: "png", "jpeg"）。
    pub format: String,
    /// ピクセル幅（不明なら 0）。
    pub width: u32,
    /// ピクセル高さ（不明なら 0）。
    pub height: u32,
}

/// 画像デコーダ（サムネイル判定用）。
pub trait ImageDecoder {
    /// バイト列からフォーマットを判定する。
    fn decode(&self, bytes: &[u8]) -> Result<ImageInfo, PortError>;
}

/// ファイル選択 / reveal / OS 連携。
pub trait OpenRevealPrompt {
    /// ユーザーにファイルを選択させる（ブロッキング）。
    fn pick_paths(&self) -> Result<Vec<PathBuf>, PortError>;
    /// OS のファイルマネージャで場所を表示する。
    fn reveal(&self, path: &Path) -> Result<(), PortError>;
    /// OS の既定アプリで開く。
    fn open_with_system(&self, path: &Path) -> Result<(), PortError>;
}

/// OS クリップボードのファイル一覧（uri-list）。
pub trait Clipboard {
    /// uri-list を読み取る。
    fn read_uri_list(&self) -> Result<Vec<String>, PortError>;
    /// uri-list を書き込む。
    fn write_uri_list(&self, uris: &[String]) -> Result<(), PortError>;
}

/// 監視ハンドル（close で解除）。
pub trait WatchHandle {
    /// 監視を停止する。
    fn close(&mut self);
}

/// ディレクトリ監視。
pub trait WatchSource {
    /// パスの監視を開始する。
    fn watch(&self, path: &Path) -> Result<Box<dyn WatchHandle>, PortError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct NoopTrash;

    impl TrashBackend for NoopTrash {
        fn list(&self) -> Result<Vec<TrashEntry>, PortError> {
            Ok(Vec::new())
        }
        fn restore(&self, _ids: &[OsString]) -> Result<RestoreReport, PortError> {
            Ok(RestoreReport::default())
        }
        fn delete(&self, _path: &Path) -> Result<(), PortError> {
            Err(PortError::message("permanent delete is not available"))
        }
    }

    #[test]
    fn noop_trash_backend_is_usable_without_a_platform() {
        let backend = NoopTrash;
        assert!(backend.list().unwrap().is_empty());
        assert_eq!(backend.restore(&[]).unwrap(), RestoreReport::default());
        assert!(backend.delete(Path::new("/tmp/x")).is_err());
    }

    #[test]
    fn port_error_keeps_message_and_io_source() {
        let err = PortError::io("read failed", io::Error::other("boom"));
        assert_eq!(err.message_text(), "read failed");
        assert!(err.io_error().is_some());
        let plain = PortError::message("unavailable");
        assert!(plain.io_error().is_none());
    }

    #[test]
    fn port_error_source_is_the_held_io_error_or_none() {
        // AR20: `io_error()` is the port accessor; `Error::source()` is the
        // standard error-chain contract used by callers that only see a
        // `dyn Error`. Both must agree on Some/None.
        use std::error::Error;

        let caused = PortError::io("copy failed", io::Error::other("disk full"));
        assert_eq!(
            caused.source().map(ToString::to_string),
            Some("disk full".to_string())
        );
        assert_eq!(
            caused.source().map(ToString::to_string),
            caused.io_error().map(ToString::to_string)
        );

        let plain = PortError::message("not available");
        assert!(plain.source().is_none());
        assert!(plain.io_error().is_none());
    }
}
