// SPDX-License-Identifier: AGPL-3.0-or-later
//! 旧 `model` モジュールは `gfe_core::model` へ移動した（R2-CORE-MOVE）。
//! 既存の `crate::model::*` / `crate::model::error::*` 参照を壊さないための
//! 薄い re-export。`error` サブモジュールも glob に含まれる。

pub use gfe_core::model::*;
