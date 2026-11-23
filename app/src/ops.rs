// SPDX-License-Identifier: AGPL-3.0-or-later
//! 旧 `ops` モジュールは `gfe_core::ops` へ移動した（R2-CORE-MOVE）。
//! 既存の `crate::ops::*` 参照を壊さないための薄い re-export。

pub use gfe_core::ops::*;
