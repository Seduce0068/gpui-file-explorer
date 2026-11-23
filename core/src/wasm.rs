// SPDX-License-Identifier: AGPL-3.0-or-later
//! wasm32 向け JS バインディング（PWA から一覧・検索を呼ぶための薄い窓口）。
//!
//! このモジュールは `cfg(target_family = "wasm")` でのみコンパイルされ、
//! ネイティブの `cargo test` には一切影響しない。既存の `std::fs` ベース
//! `model::read_entries` / `search::search_by_name` をそのまま呼び、結果を
//! `serde_json` で JSON 文字列にして `JsValue` で返す。
//! 依存は `wasm-bindgen` と `serde_json` のみ（web_sys / js_sys は使わない）。

use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use crate::{model, search};

/// `console.error` を呼ぶ最小の bridge（`web_sys` を使わない）。
#[wasm_bindgen(inline_js = r#"
export function gfe_console_error(message) {
    console.error(message);
}
"#)]
extern "C" {
    #[wasm_bindgen(js_name = gfe_console_error)]
    fn console_error(message: &str);
}

/// panic の内容をブラウザの `console.error` に流す。
///
/// PWA 側で一度呼んでおくと、wasm 側の panic が `RuntimeError: unreachable`
/// だけで消えるのを防げる。フック内で文字列化しかしないため、
/// 追加の依存やスレッド状態は不要。
#[wasm_bindgen]
pub fn set_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        console_error(&info.to_string());
    }));
}

/// JS 側から見るファイル一覧の 1 行。
///
/// `model::Entry` は `OsString` / `PathBuf` / `SystemTime` を持つため
/// `Serialize` を直接deriveできず、JSON 向けの平坦な DTO を用意する。
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct EntryView {
    name: String,
    path: String,
    is_dir: bool,
    is_symlink: bool,
    size: u64,
    kind: String,
    /// UNIX epoch からのミリ秒。時刻が読めない / epoch 前なら `null`。
    modified_ms: Option<i64>,
}

impl From<&model::Entry> for EntryView {
    fn from(entry: &model::Entry) -> Self {
        Self {
            name: entry.name_text.clone(),
            path: entry.path.to_string_lossy().into_owned(),
            is_dir: entry.is_dir,
            is_symlink: entry.is_symlink,
            size: entry.size,
            kind: model::kind_label(entry),
            modified_ms: entry
                .modified
                .and_then(|time: SystemTime| time.duration_since(UNIX_EPOCH).ok())
                .and_then(|duration| i64::try_from(duration.as_millis()).ok()),
        }
    }
}

/// JS 側から見る検索結果の 1 行（一覧 1 行 + 内容検索の行番号）。
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SearchHitView {
    entry: EntryView,
    match_line: Option<u32>,
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
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EntryInput {
    name: String,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    is_dir: bool,
    #[serde(default)]
    is_symlink: bool,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    modified_ms: Option<i64>,
}

impl EntryInput {
    fn into_entry(self) -> model::Entry {
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
            modified: self.modified_ms.and_then(|millis| {
                if millis >= 0 {
                    UNIX_EPOCH.checked_add(std::time::Duration::from_millis(millis as u64))
                } else {
                    None
                }
            }),
        }
    }
}

/// JS から渡した JSON 配列をコアの並び順ロジックで整列し、JSON で返す。
#[wasm_bindgen(js_name = listEntriesFromJson)]
pub fn list_entries_from_json(json: &str) -> Result<JsValue, JsValue> {
    let inputs: Vec<EntryInput> =
        serde_json::from_str(json).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let all: Vec<model::Entry> = inputs.into_iter().map(EntryInput::into_entry).collect();
    let view = model::visible_entries(&all, true, model::SortOrder::default());
    let rendered: Vec<EntryView> = view.iter().map(EntryView::from).collect();
    to_js_json(&rendered)
}

/// JS から渡した JSON 配列に対してコアの名前検索を実行する。
#[wasm_bindgen(js_name = searchInJson)]
pub fn search_in_json(query: &str, json: &str) -> Result<JsValue, JsValue> {
    let needle = query.to_lowercase();
    let inputs: Vec<EntryInput> =
        serde_json::from_str(json).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let all: Vec<model::Entry> = inputs.into_iter().map(EntryInput::into_entry).collect();
    let hits: Vec<SearchHitView> = all
        .iter()
        .filter(|entry| entry.name_text.to_lowercase().contains(&needle))
        .map(|entry| SearchHitView {
            entry: entry.into(),
            match_line: None,
        })
        .collect();
    to_js_json(&hits)
}

/// `serde_json` で JSON 文字列にして `JsValue` へ渡す共通処理。
fn to_js_json<T: Serialize>(value: &T) -> Result<JsValue, JsValue> {
    let json =
        serde_json::to_string(value).map_err(|error| JsValue::from_str(&error.to_string()))?;
    Ok(JsValue::from_str(&json))
}

/// PWA から呼ぶコア API のハンドル（状態を持たない薄いラッパー）。
#[wasm_bindgen]
pub struct CoreViewer {}

#[wasm_bindgen]
impl CoreViewer {
    /// JS 側からは `CoreViewer.new()` で生成する。
    pub fn new() -> CoreViewer {
        CoreViewer {}
    }

    /// `dir` を走査し、一覧を JSON 配列の `JsValue` で返す。
    ///
    /// 各要素は
    /// `{ name, path, isDir, isSymlink, size, kind, modifiedMs }`（camelCase）。
    /// `std::fs` は wasm32 で実質的に失敗するため、その場合は
    /// `AppError::message_ja()` を文字列の `JsValue` として返す。
    pub fn list_entries(&self, dir: &str) -> Result<JsValue, JsValue> {
        let entries = model::read_entries(Path::new(dir))
            .map_err(|error| JsValue::from_str(&error.message_ja()))?;
        let views: Vec<EntryView> = entries.iter().map(EntryView::from).collect();
        to_js_json(&views)
    }

    /// `dir` 配下を再帰検索し、名前が `query` を含む結果を JSON 配列で返す。
    ///
    /// 各要素は `{ entry: {...}, matchLine: null }`（名前検索なので行番号は常に null）。
    pub fn search(&self, query: &str, dir: &str) -> Result<JsValue, JsValue> {
        let hits = search::search_by_name(
            Path::new(dir),
            query,
            search::SearchOptions::default(),
            &AtomicBool::new(false),
        );
        let views: Vec<SearchHitView> = hits.iter().map(SearchHitView::from).collect();
        to_js_json(&views)
    }
}

impl Default for CoreViewer {
    fn default() -> Self {
        Self::new()
    }
}
