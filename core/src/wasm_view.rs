// SPDX-License-Identifier: AGPL-3.0-or-later
//! wasm バインディング向け DTO と、その純粋な変換ロジック。
//!
//! `core/src/wasm.rs` は JSON のデコード/`JsValue` 変換だけに留め、
//! ここでは `model::Entry` / `search::SearchResult` との相互変換を担う。
//! wasm-bindgen に依存しないためネイティブの `cargo test` で境界値を検証できる。

use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::{model, search};

/// サムネイル読み込みを避けるためのサイズ上限（8 MiB）。
///
/// `app/src/preview.rs::MAX_IMAGE_BYTES` と同じ値をコア側にも持つ。wasm の
/// in-memory API は JS から任意の `size` を受け取れるため、これを超える行には
/// [`OVERSIZED_KIND`] を付与する。PWA は現状 `kind` を読まないため、
/// 実際に読み込みをスキップするのは `size` を見る側の実装に依存する。
pub const MAX_THUMBNAIL_BYTES: u64 = 8 * 1024 * 1024;

/// `MAX_THUMBNAIL_BYTES` 超過を示す `kind` の値（PWA は現状未使用）。
pub const OVERSIZED_KIND: &str = "oversized";

/// `EntryView.kind` を決める。通常ファイルが上限を超えた場合だけ
/// [`OVERSIZED_KIND`] を返し、ディレクトリ / リンクの種別は変えない。
pub fn entry_kind(entry: &model::Entry) -> String {
    if !entry.is_dir && !entry.is_symlink && entry.size > MAX_THUMBNAIL_BYTES {
        return OVERSIZED_KIND.to_string();
    }
    model::kind_label(entry)
}

/// JS 側から見るファイル一覧の 1 行。
///
/// `model::Entry` は `OsString` / `PathBuf` / `SystemTime` を持つため
/// `Serialize` を直接 deriveできず、JSON 向けの平坦な DTO を用意する。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryView {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: u64,
    pub kind: String,
    /// UNIX epoch からのミリ秒。時刻が読めない / epoch 前 / `i64` 超出なら `null`。
    pub modified_ms: Option<i64>,
}

/// `Entry.modified` を UNIX epoch からのミリ秒へ変換する。
///
/// `None`（時刻不明）、epoch 前、`i64::MAX` を超える未来はすべて `None` とし、
/// JS 側へ不正な数値を渡さない。
pub fn modified_to_ms(modified: Option<SystemTime>) -> Option<i64> {
    let time = modified?;
    let duration = time.duration_since(UNIX_EPOCH).ok()?;
    i64::try_from(duration.as_millis()).ok()
}

impl From<&model::Entry> for EntryView {
    fn from(entry: &model::Entry) -> Self {
        Self {
            name: entry.name_text.clone(),
            path: entry.path.to_string_lossy().into_owned(),
            is_dir: entry.is_dir,
            is_symlink: entry.is_symlink,
            size: entry.size,
            kind: entry_kind(entry),
            modified_ms: modified_to_ms(entry.modified),
        }
    }
}

/// JS 側から見る検索結果の 1 行（一覧 1 行 + 内容検索の行番号）。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHitView {
    pub entry: EntryView,
    pub match_line: Option<u32>,
}

impl From<&search::SearchResult> for SearchHitView {
    fn from(hit: &search::SearchResult) -> Self {
        Self {
            entry: EntryView::from(&hit.entry),
            match_line: hit.match_line,
        }
    }
}

/// JS 側から渡す一覧行（wasm の `std::fs` が使えないための in-memory 入力）。
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryInput {
    pub name: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub is_dir: bool,
    #[serde(default)]
    pub is_symlink: bool,
    #[serde(default)]
    pub size: u64,
    #[serde(default)]
    pub modified_ms: Option<i64>,
}

/// JS から渡された UNIX epoch ミリ秒を `SystemTime` へ変換する。
///
/// 負値（epoch 前）と `None` は `None`。`checked_add` により、
/// `i64::MAX` のような極端な値がプラットフォームの表現範囲を超える場合も
/// panic せず `None` になる（wasm32 の JS Date 範囲を含む）。
pub fn ms_to_modified(millis: Option<i64>) -> Option<SystemTime> {
    let millis = millis?;
    if millis < 0 {
        return None;
    }
    UNIX_EPOCH.checked_add(Duration::from_millis(millis as u64))
}

impl EntryInput {
    /// `model::Entry` へ変換する。`path` 省略時は `name` をパスとして使う。
    ///
    /// `size` は JS から任意に渡せるためここでは丸めずに保持し、
    /// [`entry_kind`] / [`EntryView::from`] が [`MAX_THUMBNAIL_BYTES`] を
    /// 超える行を `oversized` として `EntryView.kind` へ保持する（PWA 側は未使用）。
    pub fn into_entry(self) -> model::Entry {
        let name_text = self.name;
        let sort_key = name_text.to_lowercase();
        let path = self.path.unwrap_or_else(|| name_text.clone());
        model::Entry {
            name: std::ffi::OsString::from(name_text.clone()),
            name_text,
            sort_key,
            path: Path::new(&path).to_path_buf(),
            is_dir: self.is_dir,
            is_symlink: self.is_symlink,
            size: self.size,
            modified: ms_to_modified(self.modified_ms),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::path::PathBuf;

    fn entry(name: &str, path: &str, modified: Option<SystemTime>) -> model::Entry {
        model::Entry {
            name: OsString::from(name),
            name_text: name.to_string(),
            sort_key: name.to_lowercase(),
            path: PathBuf::from(path),
            is_dir: false,
            is_symlink: false,
            size: 12,
            modified,
        }
    }

    #[test]
    fn modified_to_ms_boundaries() {
        assert_eq!(modified_to_ms(None), None);
        // epoch 前は duration_since が Err になるため null。
        assert_eq!(
            modified_to_ms(UNIX_EPOCH.checked_sub(Duration::from_secs(1))),
            None
        );
        assert_eq!(modified_to_ms(Some(UNIX_EPOCH)), Some(0));
        assert_eq!(
            modified_to_ms(Some(UNIX_EPOCH + Duration::from_millis(1_234))),
            Some(1_234)
        );
        // i64::MAX を 1 ミリ秒超える未来は i64 へ変換できないため null。
        let beyond_i64 = UNIX_EPOCH + Duration::from_millis(i64::MAX as u64 + 1);
        assert_eq!(modified_to_ms(Some(beyond_i64)), None);
    }

    #[test]
    fn ms_to_modified_boundaries() {
        assert_eq!(ms_to_modified(None), None);
        assert_eq!(ms_to_modified(Some(-1)), None);
        assert_eq!(ms_to_modified(Some(i64::MIN)), None);
        assert_eq!(ms_to_modified(Some(0)), Some(UNIX_EPOCH));
        assert_eq!(
            ms_to_modified(Some(1_234)),
            Some(UNIX_EPOCH + Duration::from_millis(1_234))
        );
        // i64::MAX は panic せず、ネイティブでは表現可能な時刻として扱う。
        assert_eq!(
            ms_to_modified(Some(i64::MAX)),
            Some(UNIX_EPOCH + Duration::from_millis(i64::MAX as u64))
        );
    }

    #[test]
    fn entry_view_uses_same_timestamp_rule() {
        let view = EntryView::from(&entry(
            "README.md",
            "/docs/README.md",
            Some(UNIX_EPOCH + Duration::from_millis(500)),
        ));
        assert_eq!(view.name, "README.md");
        assert_eq!(view.path, "/docs/README.md");
        assert_eq!(view.kind, model::kind_label(&entry("README.md", "", None)));
        assert_eq!(view.modified_ms, Some(500));

        let pre_epoch = EntryView::from(&entry(
            "old.txt",
            "old.txt",
            UNIX_EPOCH.checked_sub(Duration::from_secs(1)),
        ));
        assert_eq!(pre_epoch.modified_ms, None);
    }

    #[test]
    fn entry_input_path_defaults_to_name() {
        let input = EntryInput {
            name: "notes.txt".to_string(),
            path: None,
            is_dir: false,
            is_symlink: false,
            size: 0,
            modified_ms: None,
        };
        let converted = input.into_entry();
        assert_eq!(converted.name_text, "notes.txt");
        assert_eq!(converted.path, PathBuf::from("notes.txt"));
        assert_eq!(converted.modified, None);

        let with_path = EntryInput {
            name: "notes.txt".to_string(),
            path: Some("/demo/notes.txt".to_string()),
            is_dir: false,
            is_symlink: false,
            size: 0,
            modified_ms: Some(-5),
        };
        let converted = with_path.into_entry();
        assert_eq!(converted.path, PathBuf::from("/demo/notes.txt"));
        // 負値は epoch 前として破棄される。
        assert_eq!(converted.modified, None);
    }

    #[test]
    fn oversized_regular_files_get_the_sentinel_kind() {
        let mut big = entry("photo.png", "/demo/photo.png", None);
        big.size = MAX_THUMBNAIL_BYTES;
        assert_eq!(entry_kind(&big), "png", "exactly 8 MiB stays a normal file");
        assert_eq!(EntryView::from(&big).kind, "png");

        big.size = MAX_THUMBNAIL_BYTES + 1;
        assert_eq!(entry_kind(&big), OVERSIZED_KIND);
        assert_eq!(EntryView::from(&big).kind, OVERSIZED_KIND);

        big.size = u64::MAX;
        assert_eq!(entry_kind(&big), OVERSIZED_KIND, "u64::MAX must not wrap");
        assert_eq!(EntryView::from(&big).kind, OVERSIZED_KIND);
    }

    #[test]
    fn oversized_guard_leaves_dirs_links_and_small_files_untouched() {
        let mut dir = entry("folder", "/demo/folder", None);
        dir.is_dir = true;
        dir.size = u64::MAX;
        assert_eq!(entry_kind(&dir), model::kind_label(&dir));

        let mut link = entry("shortcut", "/demo/shortcut", None);
        link.is_symlink = true;
        link.size = u64::MAX;
        assert_eq!(entry_kind(&link), model::kind_label(&link));

        let mut small = entry("notes.txt", "/demo/notes.txt", None);
        small.size = MAX_THUMBNAIL_BYTES - 1;
        assert_eq!(entry_kind(&small), "txt");
    }

    #[test]
    fn oversized_kind_survives_entry_input_round_trip() {
        // JS JSON 経路全体ではなく、EntryInput -> model::Entry -> EntryView.kind
        // の DTO 往復だけを検証する。PWA (pwa/main.js) は現状 kind を読まないため、
        // これは「描画側へ届いた」ことの証明ではない。JS 側の無視は
        // pwa/main.test.js の回帰テストで固定する。
        let input = EntryInput {
            name: "huge.png".to_string(),
            path: None,
            is_dir: false,
            is_symlink: false,
            size: MAX_THUMBNAIL_BYTES + 1,
            modified_ms: None,
        };
        let view = EntryView::from(&input.into_entry());
        assert_eq!(view.size, MAX_THUMBNAIL_BYTES + 1);
        assert_eq!(view.kind, OVERSIZED_KIND);
    }

    #[test]
    fn search_hit_view_wraps_entry_and_line() {
        let hit = search::SearchResult {
            entry: entry(
                "src",
                "/demo/src",
                Some(UNIX_EPOCH + Duration::from_millis(9)),
            ),
            match_line: Some(7),
        };
        let view = SearchHitView::from(&hit);
        assert_eq!(view.entry.name, "src");
        assert_eq!(view.entry.modified_ms, Some(9));
        assert_eq!(view.match_line, Some(7));
    }
}
