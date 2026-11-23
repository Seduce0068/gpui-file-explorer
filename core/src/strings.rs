// SPDX-License-Identifier: AGPL-3.0-or-later
//! GPUI 非依存のモデル / 操作ロジックが参照する文言の最小サブセット。
//!
//! `app/src/strings.rs` が正規定義（4018 行、source-guard テスト込み）であり、
//! ここには `core::model` / `core::ops` がコンパイル時に必要とする定数だけを
//! 同じ値で保持する。段階移行中は app 側の正規定義を変更しないため、
//! 値の乖離は app 側の既存テスト（`strings.rs` の文言一致テスト）で検出される。
//! 段階3 で app の strings.rs を re-export へ統合するときは、この最小定義を
//! 正規定義へ置き換えること。

/// 相対時刻の単語（`model::human_age` で使用）。
pub const AGE_FUTURE: &str = "未来";
pub const AGE_JUST_NOW: &str = "たった今";
pub const AGE_MINUTES_SUFFIX: &str = "分前";
pub const AGE_HOURS_SUFFIX: &str = "時間前";
pub const AGE_DAYS_SUFFIX: &str = "日前";
pub const AGE_YEARS_SUFFIX: &str = "年前";

/// 種別ラベル（`model::kind_label` で使用）。
pub const KIND_FOLDER: &str = "フォルダ";
pub const KIND_LINK: &str = "リンク";
pub const KIND_NONE: &str = "—";

/// 未知値の表示（`model::human_age` / `human_datetime` で使用）。
pub const VALUE_UNKNOWN: &str = "-";

/// 名前変更の拒否文言（`ops::validate_rename` で使用）。
pub const STATUS_RENAME_INVALID_EMPTY: &str = "名前が空です";
pub const STATUS_RENAME_INVALID_DOT: &str = "`.` と `..` は名前として使えません";
pub const STATUS_RENAME_INVALID_SEPARATOR: &str = "名前にパス区切り文字は使えません";
pub const STATUS_RENAME_INVALID_NUL: &str = "名前に NUL 文字は使えません";
pub const STATUS_RENAME_INVALID_RESERVED_PREFIX: &str = "Windows の予約名は使えません: ";
pub const STATUS_RENAME_INVALID_CHAR: &str = "Windows で使えない文字が含まれています";
