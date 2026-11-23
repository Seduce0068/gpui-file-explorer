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

    /// 原因となった IO エラーの OS エラー番号（あれば）。
    ///
    /// `AppError::raw_os_error()` と同じ名前・意味に揃え、呼び出し側が
    /// errno で分岐するときに型ごとに別の accessor を覚えなくて済むようにする。
    pub fn raw_os_error(&self) -> Option<i32> {
        self.io_error().and_then(io::Error::raw_os_error)
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

/// 1 回の uri-list 読み取りで受け付けるファイル URI の上限。
pub const MAX_CLIPBOARD_FILE_URIS: usize = 256;

/// `text/uri-list` の各行を検証し、絶対ファイルパスへデコードする。
///
/// std のみの純粋関数。`file` scheme（大文字小文字不問、authority は空または
/// `localhost`）だけを受け付け、パーセントエスケープをデコードし、絶対パス
/// 以外・不正エスケープ・NUL・改行を含む行を捨てる。呼び出し順を保ち、
/// [`MAX_CLIPBOARD_FILE_URIS`] 件を超えた行は無視する。
pub fn parse_file_uri_list(uris: &[String]) -> Vec<PathBuf> {
    let mut paths = Vec::with_capacity(uris.len().min(MAX_CLIPBOARD_FILE_URIS));
    for uri in uris {
        if paths.len() >= MAX_CLIPBOARD_FILE_URIS {
            break;
        }
        if let Some(path) = parse_file_uri(uri) {
            paths.push(path);
        }
    }
    paths
}

/// 1 行の `file://` URI を検証してパスに変換する（不正なら `None`）。
fn parse_file_uri(uri: &str) -> Option<PathBuf> {
    let line = uri.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let rest = line
        .get(..7)
        .filter(|prefix| prefix.eq_ignore_ascii_case("file://"))
        .map(|_| &line[7..])?;
    let (authority, raw_path) = rest.split_once('/').unwrap_or((rest, ""));
    if !(authority.is_empty() || authority.eq_ignore_ascii_case("localhost")) {
        return None;
    }
    if raw_path.is_empty() {
        return None;
    }
    // `split_once('/')` が落とした先頭の '/' を戻し、絶対パスだけを通す。
    let encoded = format!("/{raw_path}");
    let bytes = percent_decode(encoded.as_bytes())?;
    if bytes.is_empty() || bytes.contains(&0) {
        return None;
    }
    let path = PathBuf::from(decoded_path_bytes(bytes)?);
    path.is_absolute().then_some(path)
}

/// パーセントエスケープをバイト列へデコードする（不正な列は拒否）。
fn percent_decode(input: &[u8]) -> Option<Vec<u8>> {
    let mut decoded = Vec::with_capacity(input.len());
    let mut index = 0;
    while index < input.len() {
        match input[index] {
            b'%' => {
                let high = *input.get(index + 1)?;
                let low = *input.get(index + 2)?;
                decoded.push((hex_value(high)? << 4) | hex_value(low)?);
                index += 3;
            }
            b'\r' | b'\n' => return None,
            byte => {
                decoded.push(byte);
                index += 1;
            }
        }
    }
    Some(decoded)
}

/// 16 進数 1 文字を対応する値へ変換する。
fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(unix)]
fn decoded_path_bytes(bytes: Vec<u8>) -> Option<OsString> {
    use std::os::unix::ffi::OsStringExt;
    Some(OsString::from_vec(bytes))
}

#[cfg(not(unix))]
fn decoded_path_bytes(bytes: Vec<u8>) -> Option<OsString> {
    String::from_utf8(bytes).ok().map(OsString::from)
}

/// OS クリップボードのファイル一覧（uri-list）。
///
/// 実装は [`parse_file_uri_list`] と同じ検証を適用し、**検証済みファイル URI
/// のみ**を返す契約とする（`file` scheme・絶対パス・
/// [`MAX_CLIPBOARD_FILE_URIS`] 件まで）。生の uri-list や他 scheme の行を
/// そのまま返してはならない。
pub trait Clipboard {
    /// 検証済みファイル URI を読み取る（最大 [`MAX_CLIPBOARD_FILE_URIS`] 件）。
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

    #[cfg(unix)]
    #[test]
    fn port_error_exposes_raw_os_error() {
        // AV04: ports kept errno behind io_error(); match AppError's one-call
        // accessor so both sides of the boundary can branch on the same code.
        let code = 2; // ENOENT
        let caused = PortError::io("read failed", io::Error::from_raw_os_error(code));
        assert_eq!(caused.raw_os_error(), Some(code));
        let plain = PortError::message("unavailable");
        assert_eq!(plain.raw_os_error(), None);
        // A non-OS io::Error has no errno to expose.
        let text_only = PortError::io("read failed", io::Error::other("boom"));
        assert_eq!(text_only.raw_os_error(), None);
    }

    #[test]
    fn parse_file_uri_list_accepts_only_valid_absolute_file_uris() {
        let uris: Vec<String> = [
            "file:///tmp/a.txt",
            "FILE://localhost/tmp/b%20c.txt",
            "file:///tmp/%E3%81%82.txt",
            "http://example.com/tmp/a.txt",
            "file://remote-host/tmp/a.txt",
            "file:///tmp/%ZZ.txt",
            "file:///tmp/%00.txt",
            "file:///tmp/a\r\nfile:///tmp/b.txt",
            "/tmp/a.txt",
            "file://localhost/",
            "",
            "# comment",
        ]
        .iter()
        .map(ToString::to_string)
        .collect();
        let parsed = parse_file_uri_list(&uris);
        assert_eq!(
            parsed,
            vec![
                PathBuf::from("/tmp/a.txt"),
                PathBuf::from("/tmp/b c.txt"),
                PathBuf::from("/tmp/あ.txt"),
            ]
        );
    }

    #[test]
    fn parse_file_uri_list_caps_the_number_of_entries() {
        let uris: Vec<String> = (0..MAX_CLIPBOARD_FILE_URIS + 16)
            .map(|index| format!("file:///tmp/{index}"))
            .collect();
        let parsed = parse_file_uri_list(&uris);
        assert_eq!(parsed.len(), MAX_CLIPBOARD_FILE_URIS);
        assert_eq!(parsed[0], PathBuf::from("/tmp/0"));
        assert_eq!(
            parsed[MAX_CLIPBOARD_FILE_URIS - 1],
            PathBuf::from(format!("/tmp/{}", MAX_CLIPBOARD_FILE_URIS - 1))
        );
    }

    #[cfg(unix)]
    #[test]
    fn parse_file_uri_list_preserves_non_utf8_percent_encoded_bytes() {
        use std::os::unix::ffi::OsStrExt;

        let uris = vec![String::from("file:///tmp/%FF.bin")];
        let parsed = parse_file_uri_list(&uris);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].as_os_str().as_bytes(), b"/tmp/\xFF.bin");
    }
}
