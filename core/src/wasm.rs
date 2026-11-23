// SPDX-License-Identifier: AGPL-3.0-or-later
//! wasm32 向け JS バインディング（PWA から一覧・検索を呼ぶための薄い窓口）。
//!
//! このモジュールは `cfg(target_family = "wasm")` でのみコンパイルされ、
//! ネイティブの `cargo test` には一切影響しない。DTO と変換ロジックは
//! `wasm_view` に切り出してあり、ここでは JSON のデコード/`JsValue` 変換と
//! バインディングの配線だけを行う。依存は `wasm-bindgen` と `serde_json` のみ
//! （web_sys / js_sys は使わない）。

use std::path::Path;
use std::sync::atomic::AtomicBool;

use serde::Serialize;
use wasm_bindgen::prelude::*;

use crate::wasm_view::{EntryInput, EntryView, SearchHitView};
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
    let inputs: Vec<EntryInput> =
        serde_json::from_str(json).map_err(|e| JsValue::from_str(&e.to_string()))?;
    let all: Vec<model::Entry> = inputs.into_iter().map(EntryInput::into_entry).collect();
    // `search_by_name` と同じ正規化を使う（trim + 小文字化 + 空クエリは 0 件）。
    let Some(needle) = search::normalized_query(query) else {
        return to_js_json(&Vec::<SearchHitView>::new());
    };
    let hits: Vec<SearchHitView> = all
        .iter()
        .filter(|entry| search::name_matches(&entry.name_text, &needle))
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
