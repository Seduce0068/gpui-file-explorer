// SPDX-License-Identifier: AGPL-3.0-or-later
//! 旧 `search` モジュールは `gfe_core::search` へ移動した（R2-CORE-MOVE）。
//! 既存の `crate::search::*` 参照を壊さないための薄い re-export。

pub use gfe_core::search::*;
